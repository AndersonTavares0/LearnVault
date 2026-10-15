//! Varredura da biblioteca física.
//!
//! O scanner percorre a raiz já resolvida pela integração de plataforma e devolve uma
//! lista plana e ordenada de arquivos, com a chave de localização de cada um. A
//! hierarquia de cursos, seções e aulas é derivada dessa lista pelo catálogo; o scanner
//! não a monta.
//!
//! A varredura é estritamente de leitura: nenhum arquivo ou diretório da biblioteca é
//! criado, alterado ou removido. Links simbólicos internos não são seguidos — nem
//! diretórios nem arquivos — para que a varredura não alcance nada fora da raiz. A raiz
//! em si é confiada: ela é resolvida pela integração de plataforma e, se for um link, o
//! próprio sistema operacional a resolve.
//!
//! Diretórios ou arquivos ilegíveis são registrados em [`ScanOutcome::inaccessible`] e a
//! varredura continua, sem interromper a biblioteca inteira por causa de um item. Se a
//! própria raiz não puder ser lida, a varredura falha com erro tipado em vez de se
//! apresentar como uma biblioteca vazia.

use std::path::Path;
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::paths::{KeyMode, relative_path_key};

/// Extensões reconhecidas como mídia reproduzível.
const MEDIA_EXTENSIONS: &[&str] = &["mp4", "webm", "mkv", "m3u8", "ts"];

/// Extensões reconhecidas como legenda.
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "vtt", "ass", "ssa"];

/// Tamanho mínimo para que um arquivo `.ts` seja tratado como mídia.
///
/// A extensão `.ts` também é usada por código-fonte TypeScript; arquivos pequenos são
/// material e não vídeo.
const MIN_TS_MEDIA_SIZE: u64 = 1024 * 1024;

/// Nomes de diretório ignorados durante a varredura.
const IGNORED_DIRECTORY_NAMES: &[&str] = &["temp", "0____"];

/// Prefixo de diretório ignorado durante a varredura.
const IGNORED_DIRECTORY_PREFIX: &str = "temp_";

/// Classificação de um arquivo encontrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// Arquivo reproduzível (vídeo ou fluxo).
    Media,
    /// Arquivo de legenda.
    Subtitle,
    /// Qualquer outro arquivo de apoio.
    Material,
}

/// Arquivo encontrado durante a varredura.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Caminho relativo à raiz, com separadores `/` e a caixa original.
    pub relative_path: String,
    /// Chave de comparação derivada do caminho, normalizada conforme o sistema.
    pub relative_path_key: String,
    /// Classificação do arquivo.
    pub kind: MediaKind,
    /// Tamanho em bytes.
    pub size: u64,
    /// Instante de modificação em segundos desde a época Unix.
    ///
    /// Vale `0` quando o sistema de arquivos não informa o instante ou quando o valor é
    /// anterior à época Unix. `0` significa "desconhecido", nunca "muito antigo".
    pub mtime: i64,
}

/// Resultado de uma varredura.
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    /// Arquivos encontrados, ordenados por caminho relativo.
    pub files: Vec<ScannedFile>,
    /// Caminhos relativos que não puderam ser lidos nem catalogados.
    ///
    /// Inclui diretórios e arquivos ilegíveis e nomes que o domínio recusa. Nunca
    /// contém caminho absoluto; a própria raiz é representada por `.`.
    pub inaccessible: Vec<String>,
}

/// Percorre a biblioteca e devolve os arquivos encontrados.
///
/// A raiz precisa existir, ser um diretório e poder ser lida; caso contrário, devolve
/// [`Error::LibraryRootMissing`], [`Error::LibraryRootNotDirectory`] ou
/// [`Error::LibraryRootUnreadable`].
pub fn scan(root: &Path) -> Result<ScanOutcome> {
    if !root.exists() {
        return Err(Error::LibraryRootMissing {
            path: root.to_path_buf(),
        });
    }
    if !root.is_dir() {
        return Err(Error::LibraryRootNotDirectory {
            path: root.to_path_buf(),
        });
    }
    // Confirma a leitura da raiz antes de percorrê-la: sem isto, uma raiz ilegível
    // devolveria uma varredura vazia e seria indistinguível de uma biblioteca sem
    // arquivos — e a política de rescan interpretaria a falha de acesso como ausência.
    std::fs::read_dir(root).map_err(|source| Error::LibraryRootUnreadable {
        path: root.to_path_buf(),
        source,
    })?;

    let mode = KeyMode::for_host();
    let mut files = Vec::new();
    let mut inaccessible = Vec::new();

    let mut walker = WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .into_iter();

    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                inaccessible.push(relative_to_root(root, error.path()));
                continue;
            }
        };

        let file_type = entry.file_type();

        // Um link simbólico é ignorado por completo: nem é seguido, nem é reportado.
        if file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || is_ignored_directory(&name) {
                walker.skip_current_dir();
            }
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => {
                inaccessible.push(relative_to_root(root, Some(entry.path())));
                continue;
            }
        };

        let relative_path = relative_to_root(root, Some(entry.path()));
        let Ok(key) = relative_path_key(&relative_path, mode) else {
            // Um nome que o domínio recusa não pode derrubar a biblioteca inteira: o
            // item é registrado como não catalogado e a varredura continua.
            inaccessible.push(relative_path);
            continue;
        };

        files.push(ScannedFile {
            relative_path,
            relative_path_key: key,
            kind: classify(&name, metadata.len()),
            size: metadata.len(),
            mtime: mtime_seconds(&metadata),
        });
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    Ok(ScanOutcome {
        files,
        inaccessible,
    })
}

fn classify(name: &str, size: u64) -> MediaKind {
    match extension(name).as_deref() {
        Some(extension) if SUBTITLE_EXTENSIONS.contains(&extension) => MediaKind::Subtitle,
        Some("ts") if size < MIN_TS_MEDIA_SIZE => MediaKind::Material,
        Some(extension) if MEDIA_EXTENSIONS.contains(&extension) => MediaKind::Media,
        _ => MediaKind::Material,
    }
}

fn extension(name: &str) -> Option<String> {
    let (_, extension) = name.rsplit_once('.')?;
    if extension.is_empty() {
        return None;
    }
    Some(extension.to_ascii_lowercase())
}

fn is_ignored_directory(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    IGNORED_DIRECTORY_NAMES.contains(&name.as_str()) || name.starts_with(IGNORED_DIRECTORY_PREFIX)
}

fn mtime_seconds(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Converte um caminho da varredura em caminho relativo à raiz.
///
/// Nunca devolve caminho absoluto: quando o caminho não pode ser convertido, ou quando
/// é a própria raiz, devolve `.`.
fn relative_to_root(root: &Path, path: Option<&Path>) -> String {
    let Some(path) = path else {
        return ".".to_owned();
    };
    let Ok(relative) = path.strip_prefix(root) else {
        return ".".to_owned();
    };

    let relative = relative.to_string_lossy().replace('\\', "/");
    if relative.is_empty() {
        ".".to_owned()
    } else {
        relative
    }
}
