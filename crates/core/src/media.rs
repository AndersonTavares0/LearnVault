//! Safe media resolution.
//!
//! The core accepts a `library_id` and `media_id`, never a client-supplied path. It
//! revalidates the relative path stored in the database, confines it to the library root,
//! and returns an already-open read-only file with metadata from that descriptor. Returning
//! the descriptor prevents adapters from reopening a path that may have changed.
//!
//! Unix opens every path component relative to its parent directory descriptor and refuses
//! symlinks. Windows opens files relative to a directory capability, preventing an
//! intermediate junction from redirecting access outside the root. Hard links to files
//! outside the root are indistinguishable from ordinary files. The library is never written
//! by this code, though another process may still change
//! its contents.
//!
//! The platform adapter is responsible for pairing the supplied root with the `library_id`.
//! This module confines the file to the root it receives but cannot verify that association.

use std::fs::{File, Metadata};
use std::io;
use std::path::{Path, PathBuf};

use rusqlite::OptionalExtension;

use crate::db::Database;
use crate::error::{Error, Result};
use crate::paths::resolve_within;
use crate::paths::{KeyMode, relative_path_key};
use crate::scanner::{MediaKind, classify, mtime_seconds};

/// Referência persistida usada pelo adaptador HTTP para abrir uma mídia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaReference {
    /// Biblioteca que contém o arquivo.
    pub library_id: String,
    /// Identidade estável do arquivo.
    pub media_id: String,
    /// Caminho relativo persistido, nunca um caminho absoluto.
    pub relative_path: String,
    size: u64,
    mtime: i64,
}

impl MediaReference {
    /// Tamanho em bytes observado na última varredura.
    pub fn size(&self) -> u64 {
        self.size
    }
}

/// Resolve um caminho relativo indexado sem acessar o sistema de arquivos.
///
/// A função só retorna mídias disponíveis e recusa componentes que poderiam transformar a
/// entrada HTTP em um caminho fora da biblioteca. A abertura e a validação final ficam em
/// [`open_media`].
pub fn find_media_by_path(
    db: &Database,
    library_id: &str,
    path: &str,
) -> Result<Option<MediaReference>> {
    let candidate = std::path::Path::new(path);
    if path.is_empty() || candidate.is_absolute() {
        return Ok(None);
    }
    let mut parts = Vec::new();
    for component in candidate.components() {
        match component {
            std::path::Component::Normal(value) => parts.push(value.to_string_lossy().into_owned()),
            _ => return Ok(None),
        }
    }
    if parts.is_empty() {
        return Ok(None);
    }
    let normalized = parts.join("/");
    let key = match relative_path_key(&normalized, KeyMode::for_host()) {
        Ok(key) => key,
        // A chave lexical pode recusar um caminho que a varredura de componentes do host
        // não reconheceu como travessia (por exemplo `..\x` no Unix, onde `\` é um
        // caractere comum). Para o cliente isso continua sendo ausência de mídia, nunca
        // corrupção do servidor — e o resultado não pode depender da plataforma.
        Err(Error::InvalidRelativePath { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    db.connection()
        .query_row(
            "SELECT library_id, media_id, relative_path, size, mtime
             FROM media
            WHERE media.library_id = ?1 AND media.relative_path_key = ?2 AND media.availability = 'available'
              AND NOT EXISTS (SELECT 1 FROM media_removal r WHERE r.library_id = media.library_id AND r.media_id = media.media_id)
          LIMIT 1",
            rusqlite::params![library_id, key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?
        .map(|(library_id, media_id, relative_path, raw_size, mtime)| {
            u64::try_from(raw_size)
                .map(|size| MediaReference {
                    library_id,
                    relative_path,
                    size,
                    mtime,
                    media_id: media_id.clone(),
                })
                .map_err(|_| Error::MediaCorruptSize { media_id })
        })
        .transpose()
}

/// Descobre a biblioteca que contém uma mídia pelo identificador estável.
///
/// O desktop entrega mídia por `media_id` opaco sem conhecer a raiz: esta consulta liga a
/// identidade à biblioteca registrada, e a abertura em [`open_media`] revalida a raiz
/// correspondente. Não acessa o sistema de arquivos.
pub fn find_media_library_id(db: &Database, media_id: &str) -> Result<Option<String>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT library_id FROM media WHERE media_id = ?1",
            [media_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?)
}

/// Mídia resolvida e já aberta para leitura.
#[derive(Debug)]
pub struct OpenMedia {
    /// Identificador da mídia.
    pub media_id: String,
    /// Arquivo aberto em modo somente leitura, posicionado no início.
    pub file: File,
    /// Tamanho em bytes, lido do descritor aberto.
    pub size: u64,
    /// Modificação em segundos desde a época Unix, lida do descritor aberto.
    pub mtime: i64,
    /// Classificação derivada do nome do arquivo.
    pub kind: MediaKind,
    /// Nome do arquivo (apenas o componente final, nunca um caminho).
    ///
    /// É a única informação de localização exposta a um adaptador, e serve para decisões
    /// de tipo de conteúdo (por exemplo a allowlist de MIME do protocolo de mídia do
    /// desktop). O caminho completo nunca cruza a fronteira do núcleo.
    pub name: String,
    /// Verdadeiro quando o arquivo difere do que a última varredura registrou.
    pub changed: bool,
}

/// Abre uma mídia da biblioteca para leitura.
///
/// A mídia é identificada por `library_id` e `media_id`. A raiz vem da integração de
/// plataforma, já resolvida; esta função apenas confirma que o arquivo está sob ela.
///
/// Devolve erro tipado quando o identificador é desconhecido, quando a mídia foi removida
/// do catálogo, quando a linha está marcada como ausente, quando o arquivo desapareceu do
/// disco, quando o caminho armazenado é inválido ou escapa da biblioteca, quando o
/// resultado não é um arquivo comum, quando o componente final é um link simbólico ou
/// ponto de reparse, quando a abertura falha ou quando o arquivo aberto não corresponde ao
/// caminho validado.
pub fn open_media(
    db: &Database,
    library_id: &str,
    media_id: &str,
    root: &Path,
) -> Result<OpenMedia> {
    let canonical_root = canonical_root(root)?;
    let stored = media_row(db, library_id, media_id)?;

    // Uma mídia removida saiu do catálogo por decisão explícita: ela não é entregue.
    if is_removed(db, library_id, media_id)? {
        return Err(Error::MediaRemoved {
            media_id: media_id.to_owned(),
        });
    }

    if stored.availability != AVAILABLE {
        return Err(Error::MediaUnavailable {
            media_id: media_id.to_owned(),
        });
    }

    // O caminho vem do banco e não é confiado: a validação lexical é repetida aqui, e um
    // caminho que escaparia da raiz é recusado antes de qualquer acesso ao disco.
    let candidate = resolve_within(&canonical_root, &stored.relative_path)?;

    let on_path = std::fs::symlink_metadata(&candidate).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::MediaMissingOnDisk {
            media_id: media_id.to_owned(),
        },
        _ => Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        },
    })?;

    // `symlink_metadata` não segue o link, então o tipo do próprio componente final é
    // observável. Links não são seguidos na indexação, portanto não são mídia conhecida.
    if on_path.file_type().is_symlink() || is_reparse_point(&on_path) {
        return Err(Error::MediaIsSymlink {
            media_id: media_id.to_owned(),
        });
    }

    // Classifica o tipo antes de abrir: no Windows, abrir um diretório falha com
    // PermissionDenied e seria reportado como `MediaUnreadable`, enquanto no Unix a
    // abertura funciona e a recusa viria só do `metadata.is_file()` pós-abertura. Usar o
    // tipo já observado no caminho mantém `MediaNotAFile` idêntico nas duas plataformas e
    // preserva `MediaUnreadable` para falhas reais de abertura de um arquivo comum.
    if !on_path.is_file() {
        return Err(Error::MediaNotAFile {
            media_id: media_id.to_owned(),
        });
    }

    let resolved = std::fs::canonicalize(&candidate).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::MediaMissingOnDisk {
            media_id: media_id.to_owned(),
        },
        _ => Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        },
    })?;

    // A canonicalização resolve links intermediários; a comparação por componente garante
    // que o destino real continua dentro da biblioteca.
    if !is_inside(&resolved, &canonical_root) {
        return Err(Error::MediaOutsideLibrary {
            media_id: media_id.to_owned(),
        });
    }

    let file = open_readonly(&canonical_root, &stored.relative_path, &resolved, media_id)?;

    // Os metadados e a classificação vêm do descritor aberto, nunca do caminho.
    let metadata = file.metadata().map_err(|source| Error::MediaUnreadable {
        media_id: media_id.to_owned(),
        source,
    })?;

    if !metadata.is_file() {
        return Err(Error::MediaNotAFile {
            media_id: media_id.to_owned(),
        });
    }

    verify_identity(&resolved, &metadata, media_id)?;

    let size = metadata.len();
    let mtime = mtime_seconds(&metadata);
    let changed = size != stored.size || mtime != stored.mtime;
    let name = file_name(&resolved);
    let kind = classify(&name, size);

    Ok(OpenMedia {
        media_id: media_id.to_owned(),
        file,
        size,
        mtime,
        kind,
        name,
        changed,
    })
}

/// Valida a disponibilidade e o estado de uma referência de mídia no catálogo.
///
/// Não acessa o sistema de arquivos nem abre descritores em disco.
pub fn validate_media_reference(db: &Database, reference: &MediaReference) -> Result<()> {
    let media_id = &reference.media_id;
    let stored = media_row(db, &reference.library_id, media_id)?;
    if stored.availability != AVAILABLE {
        return Err(Error::MediaUnavailable {
            media_id: media_id.to_owned(),
        });
    }
    if is_removed(db, &reference.library_id, media_id)? {
        return Err(Error::MediaRemoved {
            media_id: media_id.to_owned(),
        });
    }
    if stored.relative_path != reference.relative_path {
        return Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        });
    }
    Ok(())
}

/// Abre o arquivo físico no disco para uma referência de mídia previamente validada.
///
/// Não acessa o banco de dados nem exige lock sobre a conexão SQLite, permitindo que
/// a abertura e o streaming ocorram de forma totalmente desacoplada da persistência.
pub fn open_media_file(reference: &MediaReference, root: &Path) -> Result<OpenMedia> {
    let canonical_root = canonical_root(root)?;
    let media_id = &reference.media_id;
    let candidate = resolve_within(&canonical_root, &reference.relative_path)?;
    let on_path = std::fs::symlink_metadata(&candidate).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::MediaMissingOnDisk {
            media_id: media_id.to_owned(),
        },
        _ => Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        },
    })?;
    if on_path.file_type().is_symlink() || is_reparse_point(&on_path) {
        return Err(Error::MediaIsSymlink {
            media_id: media_id.to_owned(),
        });
    }
    // Classifica o tipo antes de abrir: no Windows, abrir um diretório falha com
    // PermissionDenied e seria reportado como `MediaUnreadable`, enquanto no Unix a
    // abertura funciona e a recusa viria só do `metadata.is_file()` pós-abertura. Usar o
    // tipo já observado no caminho mantém `MediaNotAFile` idêntico nas duas plataformas e
    // preserva `MediaUnreadable` para falhas reais de abertura de um arquivo comum.
    if !on_path.is_file() {
        return Err(Error::MediaNotAFile {
            media_id: media_id.to_owned(),
        });
    }
    let resolved = std::fs::canonicalize(&candidate).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::MediaMissingOnDisk {
            media_id: media_id.to_owned(),
        },
        _ => Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        },
    })?;
    if !is_inside(&resolved, &canonical_root) {
        return Err(Error::MediaOutsideLibrary {
            media_id: media_id.to_owned(),
        });
    }
    let file = open_readonly(
        &canonical_root,
        &reference.relative_path,
        &resolved,
        media_id,
    )?;
    let metadata = file.metadata().map_err(|source| Error::MediaUnreadable {
        media_id: media_id.to_owned(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(Error::MediaNotAFile {
            media_id: media_id.to_owned(),
        });
    }
    verify_identity(&resolved, &metadata, media_id)?;
    let size = metadata.len();
    let mtime = mtime_seconds(&metadata);
    let name = file_name(&resolved);
    Ok(OpenMedia {
        media_id: media_id.to_owned(),
        file,
        size,
        mtime,
        kind: classify(&name, size),
        name,
        changed: size != reference.size || mtime != reference.mtime,
    })
}

/// Abre uma referência recém-lida do catálogo.
///
/// Revalida a referência no catálogo e abre o descritor no disco.
pub fn open_media_reference(
    db: &Database,
    reference: &MediaReference,
    root: &Path,
) -> Result<OpenMedia> {
    validate_media_reference(db, reference)?;
    open_media_file(reference, root)
}

/// Estado de disponibilidade registrado pela última varredura.
const AVAILABLE: &str = "available";

/// A mídia foi removida explicitamente do catálogo.
///
/// A remoção é lida da tabela própria, e não de `availability`: são fatos separados, e uma
/// mídia removida continua com a disponibilidade que a última varredura registrou.
fn is_removed(db: &Database, library_id: &str, media_id: &str) -> Result<bool> {
    let mut statement = db.connection().prepare(
        "SELECT 1
           FROM media_removal
          WHERE library_id = ?1 AND media_id = ?2",
    )?;
    let mut rows = statement.query(rusqlite::params![library_id, media_id])?;

    Ok(rows.next()?.is_some())
}

/// Linha de mídia relevante para a resolução.
struct StoredMedia {
    relative_path: String,
    size: u64,
    mtime: i64,
    availability: String,
}

/// Lê a linha de mídia da biblioteca indicada.
fn media_row(db: &Database, library_id: &str, media_id: &str) -> Result<StoredMedia> {
    let mut statement = db.connection().prepare(
        "SELECT relative_path, size, mtime, availability
         FROM media
         WHERE library_id = ?1 AND media_id = ?2",
    )?;
    let mut rows = statement.query(rusqlite::params![library_id, media_id])?;

    match rows.next()? {
        Some(row) => {
            let size: i64 = row.get(1)?;
            let size = u64::try_from(size).map_err(|_| Error::MediaCorruptSize {
                media_id: media_id.to_owned(),
            })?;
            Ok(StoredMedia {
                relative_path: row.get(0)?,
                size,
                mtime: row.get(2)?,
                availability: row.get(3)?,
            })
        }
        None => Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        }),
    }
}

/// Valida a raiz e devolve o caminho canônico dela.
///
/// A raiz já chega canonicalizada pela integração de plataforma. Se o componente final
/// virou um link simbólico, junction ou ponto de reparse, seguir o alvo autorizaria
/// arquivos fora da biblioteca: a substituição é recusada, nunca resolvida.
fn canonical_root(root: &Path) -> Result<PathBuf> {
    let path = root.to_path_buf();
    // `symlink_metadata` não segue o link, então o tipo do próprio componente final da raiz
    // é observável antes de qualquer canonicalização que o seguiria.
    let metadata = std::fs::symlink_metadata(&path).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::LibraryRootMissing { path: path.clone() },
        _ => Error::LibraryRootUnreadable {
            path: path.clone(),
            source,
        },
    })?;

    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(Error::LibraryRootIsLink { path });
    }

    if !metadata.is_dir() {
        return Err(Error::LibraryRootNotDirectory { path });
    }

    std::fs::canonicalize(&path).map_err(|source| Error::LibraryRootUnreadable { path, source })
}

/// Indica se o caminho está estritamente sob a raiz, comparando componentes.
fn is_inside(path: &Path, root: &Path) -> bool {
    path != root && path.starts_with(root)
}

/// Confere, onde o sistema permite, que o descritor aberto é o arquivo do caminho validado.
#[cfg(unix)]
fn verify_identity(resolved: &Path, opened: &Metadata, media_id: &str) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    let on_path = std::fs::symlink_metadata(resolved).map_err(|source| Error::MediaUnreadable {
        media_id: media_id.to_owned(),
        source,
    })?;

    if on_path.dev() != opened.dev() || on_path.ino() != opened.ino() {
        return Err(Error::MediaIdentityMismatch {
            media_id: media_id.to_owned(),
        });
    }
    Ok(())
}

/// Outside Unix, stable `std` does not expose descriptor identity.
#[cfg(not(unix))]
fn verify_identity(_resolved: &Path, opened: &Metadata, media_id: &str) -> Result<()> {
    if is_reparse_point(opened) {
        return Err(Error::MediaIsSymlink {
            media_id: media_id.to_owned(),
        });
    }
    Ok(())
}

/// Opens the validated file read-only without allowing path components to escape the root.
fn open_readonly(_root: &Path, _relative_path: &str, _path: &Path, media_id: &str) -> Result<File> {
    #[cfg(unix)]
    {
        use rustix::fs::{CWD, Mode, OFlags, openat};

        let directory_flags =
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
        let mut directory = openat(CWD, "/", directory_flags, Mode::empty()).map_err(|error| {
            Error::MediaUnreadable {
                media_id: media_id.to_owned(),
                source: io::Error::other(error),
            }
        })?;
        for component in _root.components().skip(1) {
            let std::path::Component::Normal(name) = component else {
                return Err(Error::MediaOutsideLibrary {
                    media_id: media_id.to_owned(),
                });
            };
            directory =
                openat(&directory, name, directory_flags, Mode::empty()).map_err(|error| {
                    Error::MediaUnreadable {
                        media_id: media_id.to_owned(),
                        source: io::Error::other(error),
                    }
                })?;
        }
        let components: Vec<_> = Path::new(_relative_path).components().collect();
        let mut file = None;
        for (index, component) in components.iter().enumerate() {
            let std::path::Component::Normal(name) = component else {
                return Err(Error::MediaOutsideLibrary {
                    media_id: media_id.to_owned(),
                });
            };
            let final_component = index + 1 == components.len();
            let flags = if final_component {
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW
            } else {
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
            };
            let opened = openat(&directory, *name, flags, Mode::empty()).map_err(|error| {
                Error::MediaUnreadable {
                    media_id: media_id.to_owned(),
                    source: io::Error::other(error),
                }
            })?;
            if final_component {
                file = Some(File::from(opened));
            } else {
                directory = opened;
            }
        }
        file.ok_or_else(|| Error::MediaOutsideLibrary {
            media_id: media_id.to_owned(),
        })
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;

        let root = std::fs::OpenOptions::new()
            .read(true)
            // Keeping delete sharing disabled pins the capability to this directory on Windows.
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(_root)
            .map_err(|source| Error::MediaUnreadable {
                media_id: media_id.to_owned(),
                source,
            })?;
        let metadata = root.metadata().map_err(|source| Error::MediaUnreadable {
            media_id: media_id.to_owned(),
            source,
        })?;
        if is_reparse_point(&metadata) {
            return Err(Error::LibraryRootIsLink {
                path: _root.to_path_buf(),
            });
        }
        if !metadata.is_dir() {
            return Err(Error::LibraryRootNotDirectory {
                path: _root.to_path_buf(),
            });
        }

        cap_std::fs::Dir::from_std_file(root)
            .open(_relative_path)
            .map(cap_std::fs::File::into_std)
            .map_err(|source| Error::MediaUnreadable {
                media_id: media_id.to_owned(),
                source,
            })
    }

    #[cfg(not(any(unix, windows)))]
    {
        std::fs::OpenOptions::new()
            .read(true)
            .open(_path)
            .map_err(|source| Error::MediaUnreadable {
                media_id: media_id.to_owned(),
                source,
            })
    }
}

/// Verdadeiro quando o metadado marca um ponto de reparse (symlink ou junction) no Windows.
#[cfg(windows)]
fn is_reparse_point(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Fora do Windows apenas links simbólicos existem, e já são detectados por `is_symlink`.
#[cfg(not(windows))]
fn is_reparse_point(_metadata: &Metadata) -> bool {
    false
}

/// Windows attribute marking a symbolic link, junction, or other reparse point.
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

#[cfg(windows)]
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;

#[cfg(windows)]
const FILE_SHARE_READ: u32 = 0x0000_0001;

#[cfg(windows)]
const FILE_SHARE_WRITE: u32 = 0x0000_0002;

/// Nome do arquivo no fim do caminho, para a classificação por extensão.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
#[cfg(all(test, windows))]
mod windows_open_tests {
    use super::open_readonly;
    #[test]
    fn an_intermediate_directory_link_cannot_escape_the_open_root()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("library");
        let outside = directory.path().join("outside");
        std::fs::create_dir_all(root.join("Course"))?;
        std::fs::create_dir_all(&outside)?;
        std::fs::write(outside.join("video.mp4"), b"synthetic video")?;
        let link = root.join("Course/Alias");
        if std::os::windows::fs::symlink_dir(&outside, &link).is_err() {
            eprintln!("skipped: creating a directory link requires Windows privileges");
            return Ok(());
        }

        let result = open_readonly(
            &root,
            "Course/Alias/video.mp4",
            &root.join("Course/Alias/video.mp4"),
            "media-1",
        );
        assert!(result.is_err(), "directory links must not escape the root");
        Ok(())
    }

    #[test]
    fn root_junction_is_not_followed() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let outside = directory.path().join("outside");
        let root_link = directory.path().join("library");
        std::fs::create_dir_all(&outside)?;
        std::fs::write(outside.join("video.mp4"), b"synthetic video")?;
        if std::os::windows::fs::symlink_dir(&outside, &root_link).is_err() {
            eprintln!("skipped: creating a directory link requires Windows privileges");
            return Ok(());
        }

        assert!(
            open_readonly(
                &root_link,
                "video.mp4",
                &root_link.join("video.mp4"),
                "media-1",
            )
            .is_err()
        );
        Ok(())
    }
}
