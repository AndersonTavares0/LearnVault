//! Resolução segura de mídia.
//!
//! O núcleo recebe `library_id` e `media_id` — nunca um caminho fornecido pelo cliente. O
//! caminho relativo é lido do banco, revalidado, canonicalizado e confinado sob a raiz da
//! biblioteca, e o arquivo é devolvido **já aberto** em modo somente leitura, com os
//! metadados lidos do próprio descritor. Devolver o descritor evita que o adaptador abra
//! de novo um caminho que pode ter mudado entre a validação e o uso.
//!
//! # O que esta resolução garante
//!
//! - o caminho persistido é revalidado a cada chamada; nada é confiado ao banco;
//! - a raiz precisa estar estritamente sob a forma canônica já validada: um componente
//!   final que virou link simbólico, junction ou ponto de reparse é recusado, nunca seguido;
//! - o caminho resolvido precisa estar estritamente sob a raiz da biblioteca;
//! - o componente final não pode ser um link simbólico nem, no Windows, um ponto de reparse;
//! - o resultado precisa ser um arquivo comum;
//! - no Unix, o arquivo aberto é comparado com o caminho validado pela identidade do
//!   descritor (`dev` e `ino`), de modo que uma troca nesse intervalo é detectada;
//! - no Windows, o componente final é aberto **sem seguir** um ponto de reparse, e o
//!   descritor resultante é recusado se for um link/junction, de modo que uma troca do
//!   componente final nesse intervalo é detectada no próprio descritor;
//! - a biblioteca nunca é escrita: a única operação sobre ela é abrir para leitura.
//!
//! # Limitações conhecidas
//!
//! Esta verificação **não elimina** a janela entre a validação e a abertura. No Unix, uma
//! troca nesse intervalo é detectada pela comparação de identidade; no Windows, a troca do
//! **componente final** é detectada pelo atributo de reparse do descritor aberto, mas o
//! `std` estável não expõe `dev`/`ino` nem a identidade do diretório pai. A própria raiz
//! substituída por link/junction é recusada. Componentes **intermediários** — inclusive
//! ancestrais da própria raiz — que sejam links simbólicos, junctions ou outros reparse
//! points dependem da canonicalização e do confinamento resultante; uma troca desses
//! componentes entre a validação e a abertura **não** é detectada em nenhuma plataforma,
//! porque a proteção completa exigiria resolução relativa a um descritor de diretório
//! (`openat`/`RESOLVE_BENEATH` no Linux, handles relativos no Windows) e está fora desta
//! fatia. Uma troca do arquivo por um **hardlink** para fora também é indistinguível de um
//! arquivo comum. Esses dois limites são residuais e registrados aqui.
//!
//! A aplicação opera em modo somente leitura, mas **não** presume que a biblioteca esteja
//! montada como somente leitura: outro processo pode alterar o diretório. Fechar essa
//! lacuna exigiria APIs específicas de plataforma e não se justifica no modelo de ameaça
//! atual; fica registrada como endurecimento futuro, condicionado a necessidade real.
//!
//! # Responsabilidade de quem chama
//!
//! A raiz é um parâmetro explícito e precisa ser a raiz da biblioteca indicada. Este
//! núcleo confina o arquivo sob a raiz recebida, mas não tem como saber se ela pertence
//! ao `library_id` informado: manter esse vínculo é responsabilidade da integração. O
//! adaptador HTTP ou Tauri resolve o mapa `library_id` → raiz antes de chamar.

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

    let file = open_readonly(&resolved, media_id)?;

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
    let file = open_readonly(&resolved, media_id)?;
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

/// Fora do Unix o `std` estável não expõe `dev`/`ino` do descritor aberto.
///
/// No Windows a defesa vem da abertura: o componente final é aberto **sem seguir** um
/// ponto de reparse ([`open_readonly`]). Se o caminho foi trocado por um link, junction ou
/// outro reparse point entre a canonicalização e a abertura, o descritor carrega o atributo
/// de reparse e é recusado. Uma troca de componente **intermediário** por um reparse point
/// nessa janela permanece um limite conhecido: o `std` estável não compara a identidade do
/// diretório pai aberto.
#[cfg(not(unix))]
fn verify_identity(_resolved: &Path, opened: &Metadata, media_id: &str) -> Result<()> {
    if is_reparse_point(opened) {
        return Err(Error::MediaIsSymlink {
            media_id: media_id.to_owned(),
        });
    }
    Ok(())
}

/// Abre o arquivo validado somente para leitura.
///
/// No Windows o componente final é aberto sem seguir um ponto de reparse
/// (`FILE_FLAG_OPEN_REPARSE_POINT`), de modo que uma troca por link/junction entre a
/// canonicalização e a abertura é observável no próprio descritor e recusada em
/// [`verify_identity`]. A flag é ignorada para arquivos comuns.
fn open_readonly(path: &Path, media_id: &str) -> Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path).map_err(|source| Error::MediaUnreadable {
        media_id: media_id.to_owned(),
        source,
    })
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

/// Atributo de arquivo que marca um ponto de reparse no Windows.
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

/// Flag de abertura que não segue o componente final de reparse no Windows.
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// Nome do arquivo no fim do caminho, para a classificação por extensão.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
