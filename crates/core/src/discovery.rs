//! Descoberta física de uma pasta de curso.
//!
//! Esta camada só lê o sistema de arquivos. Ela não atribui papéis pedagógicos, não grava
//! no banco e não altera a política de rescan. A árvore devolvida preserva inclusive itens
//! ignorados, não suportados e inacessíveis para que uma prévia nunca esconda informação.

use std::{fs, path::Path};

use serde::Serialize;

use crate::{
    error::{Error, Result},
    scanner::MediaKind,
};

const IGNORED_DIRECTORY_NAMES: &[&str] = &["temp", "0____"];
const IGNORED_DIRECTORY_PREFIX: &str = "temp_";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Identifica se um item descoberto é uma pasta ou um arquivo.
pub enum DiscoveryEntryKind {
    /// Uma pasta que pode conter outros nós.
    Directory,
    /// Um arquivo físico.
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "status", content = "reason")]
/// Resultado da leitura e classificação de um item físico.
pub enum DiscoveryStatus {
    /// O item foi reconhecido e está disponível para interpretação.
    Found,
    /// O item foi deliberadamente ignorado, com o motivo apresentado.
    Ignored(String),
    /// O item existe, mas não pôde ser lido.
    Inaccessible(String),
    /// O item foi lido, mas sua extensão não é suportada.
    Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
/// Classificação física de um arquivo reconhecido.
pub enum DiscoveryFileKind {
    /// Arquivo reproduzível.
    Media,
    /// Legenda ou faixa de texto sincronizada.
    Subtitle,
    /// Material complementar.
    Material,
}

impl From<MediaKind> for DiscoveryFileKind {
    fn from(kind: MediaKind) -> Self {
        match kind {
            MediaKind::Media => Self::Media,
            MediaKind::Subtitle => Self::Subtitle,
            MediaKind::Material => Self::Material,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// Nó da árvore física apresentada na prévia de importação.
pub struct DiscoveryNode {
    /// Nome visível no diretório pai.
    pub name: String,
    /// Caminho relativo, usando `/` como separador.
    pub path: String,
    /// Tipo físico do item.
    pub entry_kind: DiscoveryEntryKind,
    /// Resultado da leitura e classificação física.
    pub status: DiscoveryStatus,
    /// Tipo do arquivo quando reconhecido.
    pub file_kind: Option<DiscoveryFileKind>,
    /// Tamanho do arquivo, quando aplicável.
    pub size: Option<u64>,
    /// Filhos ordenados por nome.
    pub children: Vec<DiscoveryNode>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
/// Contagens dos resultados da descoberta física.
pub struct DiscoverySummary {
    /// Quantidade de itens reconhecidos.
    pub found: usize,
    /// Quantidade de itens ignorados.
    pub ignored: usize,
    /// Quantidade de itens inacessíveis.
    pub inaccessible: usize,
    /// Quantidade de itens sem extensão suportada.
    pub unsupported: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
/// Árvore física e contagens produzidas por uma descoberta.
pub struct DiscoveryReport {
    /// Árvore física completa sob a pasta selecionada.
    pub root: DiscoveryNode,
    /// Contagens por resultado da descoberta.
    pub summary: DiscoverySummary,
}

/// Lê uma pasta explicitamente selecionada e devolve a árvore física completa.
pub fn discover(root: &Path) -> Result<DiscoveryReport> {
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
    fs::read_dir(root).map_err(|source| Error::LibraryRootUnreadable {
        path: root.to_path_buf(),
        source,
    })?;

    let mut summary = DiscoverySummary::default();
    let root_name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_owned());
    let root_node = read_directory(root, ".", root_name, &mut summary, true);
    Ok(DiscoveryReport {
        root: root_node,
        summary,
    })
}

fn read_directory(
    path: &Path,
    relative: &str,
    name: String,
    summary: &mut DiscoverySummary,
    root: bool,
) -> DiscoveryNode {
    let mut children = Vec::new();
    let read = match fs::read_dir(path) {
        Ok(read) => read,
        Err(_) => {
            if !root {
                summary.inaccessible += 1;
            }
            return DiscoveryNode {
                name,
                path: relative.to_owned(),
                entry_kind: DiscoveryEntryKind::Directory,
                status: DiscoveryStatus::Inaccessible("diretório não pôde ser lido".to_owned()),
                file_kind: None,
                size: None,
                children,
            };
        }
    };
    let mut entries = Vec::new();
    let mut enumeration_failures = Vec::new();
    for (index, result) in read.enumerate() {
        match result {
            Ok(entry) => entries.push(entry),
            Err(_) => enumeration_failures.push(index),
        }
    }
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let entry_name = entry.file_name().to_string_lossy().into_owned();
        let child_path = join_relative(relative, &entry_name);
        let metadata = match fs::symlink_metadata(entry.path()) {
            Ok(metadata) => metadata,
            Err(_) => {
                summary.inaccessible += 1;
                children.push(DiscoveryNode {
                    name: entry_name,
                    path: child_path,
                    entry_kind: DiscoveryEntryKind::File,
                    status: DiscoveryStatus::Inaccessible(
                        "metadados não puderam ser lidos".to_owned(),
                    ),
                    file_kind: None,
                    size: None,
                    children: Vec::new(),
                });
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            summary.ignored += 1;
            children.push(DiscoveryNode {
                name: entry_name,
                path: child_path,
                entry_kind: if metadata.is_dir() {
                    DiscoveryEntryKind::Directory
                } else {
                    DiscoveryEntryKind::File
                },
                status: DiscoveryStatus::Ignored("link simbólico não é seguido".to_owned()),
                file_kind: None,
                size: None,
                children: Vec::new(),
            });
        } else if metadata.is_dir() {
            if entry_name.starts_with('.') || ignored_directory(&entry_name) {
                summary.ignored += 1;
                children.push(DiscoveryNode {
                    name: entry_name,
                    path: child_path,
                    entry_kind: DiscoveryEntryKind::Directory,
                    status: DiscoveryStatus::Ignored("diretório oculto ou temporário".to_owned()),
                    file_kind: None,
                    size: None,
                    children: Vec::new(),
                });
            } else {
                children.push(read_directory(
                    &entry.path(),
                    &child_path,
                    entry_name,
                    summary,
                    false,
                ));
            }
        } else if metadata.is_file() {
            let status = if entry_name.starts_with('.') {
                summary.ignored += 1;
                DiscoveryStatus::Ignored("arquivo oculto".to_owned())
            } else if let Some(kind) = supported_file_kind(&entry_name, metadata.len()) {
                summary.found += 1;
                let node = DiscoveryNode {
                    name: entry_name,
                    path: child_path,
                    entry_kind: DiscoveryEntryKind::File,
                    status: DiscoveryStatus::Found,
                    file_kind: Some(kind),
                    size: Some(metadata.len()),
                    children: Vec::new(),
                };
                children.push(node);
                continue;
            } else {
                summary.unsupported += 1;
                DiscoveryStatus::Unsupported("extensão não reconhecida para importação".to_owned())
            };
            children.push(DiscoveryNode {
                name: entry_name,
                path: child_path,
                entry_kind: DiscoveryEntryKind::File,
                status,
                file_kind: None,
                size: Some(metadata.len()),
                children: Vec::new(),
            });
        }
    }
    for index in enumeration_failures {
        let name = format!("<entrada inacessível {index}>");
        summary.inaccessible += 1;
        children.push(DiscoveryNode {
            name: name.clone(),
            path: join_relative(relative, &name),
            entry_kind: DiscoveryEntryKind::File,
            status: DiscoveryStatus::Inaccessible(
                "uma entrada do diretório não pôde ser enumerada".to_owned(),
            ),
            file_kind: None,
            size: None,
            children: Vec::new(),
        });
    }
    DiscoveryNode {
        name,
        path: relative.to_owned(),
        entry_kind: DiscoveryEntryKind::Directory,
        status: DiscoveryStatus::Found,
        file_kind: None,
        size: None,
        children,
    }
}

fn join_relative(parent: &str, name: &str) -> String {
    if parent == "." {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

fn ignored_directory(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    IGNORED_DIRECTORY_NAMES.contains(&lower.as_str()) || lower.starts_with(IGNORED_DIRECTORY_PREFIX)
}

fn supported_file_kind(name: &str, size: u64) -> Option<DiscoveryFileKind> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    if extension == "ts" && size < 1024 * 1024 {
        return Some(DiscoveryFileKind::Material);
    }
    if [
        "mp4", "webm", "mkv", "m3u8", "ts", "ogv", "mp3", "wav", "ogg", "oga", "m4a", "flac",
        "aac", "opus", "weba", "wma",
    ]
    .contains(&extension.as_str())
    {
        return Some(DiscoveryFileKind::Media);
    }
    if ["srt", "vtt", "ass", "ssa"].contains(&extension.as_str()) {
        return Some(DiscoveryFileKind::Subtitle);
    }
    if [
        "pdf", "txt", "md", "rtf", "doc", "docx", "odt", "ppt", "pptx", "odp", "xls", "xlsx",
        "ods", "png", "jpg", "jpeg", "gif", "webp", "zip",
    ]
    .contains(&extension.as_str())
    {
        return Some(DiscoveryFileKind::Material);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn preserves_unicode_duplicates_and_non_supported_items_in_physical_tree() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("Módulo/Estrutura Profunda")).unwrap();
        fs::create_dir_all(temp.path().join("Outro")).unwrap();
        fs::write(
            temp.path().join("Módulo/Estrutura Profunda/aula.mp4"),
            b"video",
        )
        .unwrap();
        fs::write(temp.path().join("Outro/aula.mp4"), b"video").unwrap();
        fs::write(
            temp.path().join("Módulo/Estrutura Profunda/nota.pdf"),
            b"pdf",
        )
        .unwrap();
        fs::write(
            temp.path().join("Módulo/Estrutura Profunda/dados.xyz"),
            b"unknown",
        )
        .unwrap();
        fs::create_dir(temp.path().join("temp_cache")).unwrap();
        let report = discover(temp.path()).unwrap();
        let modulo = report
            .root
            .children
            .iter()
            .find(|node| node.name == "Módulo")
            .unwrap();
        let deep = &modulo.children[0].children[0];
        assert_eq!(deep.path, "Módulo/Estrutura Profunda/aula.mp4");
        let outro = report
            .root
            .children
            .iter()
            .find(|node| node.name == "Outro")
            .unwrap();
        assert_eq!(outro.children[0].path, "Outro/aula.mp4");
        assert_eq!(report.summary.found, 3);
        assert_eq!(report.summary.unsupported, 1);
        assert_eq!(report.summary.ignored, 1);
    }
}
