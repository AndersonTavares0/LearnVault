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
//! - o caminho resolvido precisa estar estritamente sob a raiz da biblioteca;
//! - o componente final não pode ser um link simbólico;
//! - o resultado precisa ser um arquivo comum;
//! - no Unix, o arquivo aberto é comparado com o caminho validado pela identidade do
//!   descritor (`dev` e `ino`), de modo que uma troca nesse intervalo é detectada;
//! - a biblioteca nunca é escrita: a única operação sobre ela é abrir para leitura.
//!
//! # Limitações conhecidas
//!
//! Esta verificação **não elimina** a janela entre a validação e a abertura. No Unix, uma
//! troca nesse intervalo é detectada pela comparação de identidade; no Windows, o `std`
//! estável não expõe a identidade do descritor aberto, então a proteção fica restrita ao
//! componente final. Componentes **intermediários** que sejam links simbólicos, junctions
//! ou outros reparse points dependem da canonicalização e do confinamento resultante, e
//! uma troca desses componentes entre a validação e a abertura não é detectada no Windows.
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

use crate::db::Database;
use crate::error::{Error, Result};
use crate::paths::resolve_within;
use crate::scanner::{MediaKind, classify, mtime_seconds};

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
    /// Verdadeiro quando o arquivo difere do que a última varredura registrou.
    pub changed: bool,
}

/// Abre uma mídia da biblioteca para leitura.
///
/// A mídia é identificada por `library_id` e `media_id`. A raiz vem da integração de
/// plataforma, já resolvida; esta função apenas confirma que o arquivo está sob ela.
///
/// Devolve erro tipado quando o identificador é desconhecido, quando a linha está marcada
/// como ausente, quando o arquivo desapareceu do disco, quando o caminho armazenado é
/// inválido ou escapa da biblioteca, quando o resultado não é um arquivo comum, quando o
/// componente final é um link simbólico, quando a abertura falha ou quando o arquivo
/// aberto não corresponde ao caminho validado.
pub fn open_media(
    db: &Database,
    library_id: &str,
    media_id: &str,
    root: &Path,
) -> Result<OpenMedia> {
    let canonical_root = canonical_root(root)?;
    let stored = media_row(db, library_id, media_id)?;

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
    if on_path.file_type().is_symlink() {
        return Err(Error::MediaIsSymlink {
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

    let file = File::open(&resolved).map_err(|source| Error::MediaUnreadable {
        media_id: media_id.to_owned(),
        source,
    })?;

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
    let kind = classify(&file_name(&resolved), size);

    Ok(OpenMedia {
        media_id: media_id.to_owned(),
        file,
        size,
        mtime,
        kind,
        changed,
    })
}

/// Estado de disponibilidade registrado pela última varredura.
const AVAILABLE: &str = "available";

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
            Ok(StoredMedia {
                relative_path: row.get(0)?,
                size: u64::try_from(size).unwrap_or(0),
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
fn canonical_root(root: &Path) -> Result<PathBuf> {
    let path = root.to_path_buf();
    let metadata = std::fs::metadata(&path).map_err(|source| match source.kind() {
        io::ErrorKind::NotFound => Error::LibraryRootMissing { path: path.clone() },
        _ => Error::LibraryRootUnreadable {
            path: path.clone(),
            source,
        },
    })?;

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

/// No Windows o `std` estável não expõe a identidade do descritor aberto.
#[cfg(not(unix))]
fn verify_identity(_resolved: &Path, _opened: &Metadata, _media_id: &str) -> Result<()> {
    Ok(())
}

/// Nome do arquivo no fim do caminho, para a classificação por extensão.
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}
