//! DTOs serializáveis do catálogo do desktop.
//!
//! Os tipos do núcleo não são serializados diretamente: cada enum vira uma string estável
//! e o shape snake_case (`loose_files`, `media_id`, ...) é idêntico ao do adaptador HTTP,
//! de modo que o `normalizeCatalog` da SPA aceita o payload do desktop sem alteração.

use learnvault_core::{
    Availability, Catalog, CatalogFile, CatalogNode, MediaKind, NodeRole, RescanSummary,
};
use serde::Serialize;

/// Catálogo do desktop derivado do banco local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogDto {
    /// Verdadeiro quando a última varredura cobriu toda a biblioteca.
    pub complete: bool,
    /// Arquivos na raiz da biblioteca, que não pertencem a nenhuma categoria.
    pub loose_files: Vec<CatalogFileDto>,
    /// Categorias, ordenadas por caminho.
    pub categories: Vec<CatalogNodeDto>,
}

/// Nó da árvore do catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogNodeDto {
    /// Título apresentado, sem prefixos numéricos nem separadores de ordenação.
    pub title: String,
    /// Caminho relativo completo que identifica o nó.
    pub path: String,
    /// Papel do nó: `category`, `course`, `module` ou `lesson`.
    pub role: &'static str,
    /// Arquivos da aula; vazio fora de nós `lesson`.
    pub files: Vec<CatalogFileDto>,
    /// Arquivos diretamente no nó, sem pertencer a uma aula.
    pub loose_files: Vec<CatalogFileDto>,
    /// Nós filhos, ordenados por caminho.
    pub children: Vec<CatalogNodeDto>,
}

/// Arquivo do catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogFileDto {
    /// Identidade estável do arquivo, independente da localização.
    pub media_id: String,
    /// Nome do arquivo, sem diretórios.
    pub name: String,
    /// Caminho relativo completo, usado como referência estrutural.
    pub path: String,
    /// Tamanho em bytes na última observação.
    pub size: u64,
    /// Tipo do arquivo: `media`, `subtitle` ou `material`.
    pub kind: &'static str,
    /// Disponibilidade na última varredura: `available` ou `missing`.
    pub availability: &'static str,
    /// Verdadeiro quando o arquivo está sob uma área não verificada.
    pub unverified: bool,
}

impl From<Catalog> for CatalogDto {
    fn from(catalog: Catalog) -> Self {
        Self {
            complete: catalog.complete,
            loose_files: catalog
                .loose_files
                .into_iter()
                .map(CatalogFileDto::from)
                .collect(),
            categories: catalog
                .categories
                .into_iter()
                .map(CatalogNodeDto::from)
                .collect(),
        }
    }
}

impl From<CatalogNode> for CatalogNodeDto {
    fn from(node: CatalogNode) -> Self {
        Self {
            title: node.title,
            path: node.path,
            role: node_role(node.role),
            files: node.files.into_iter().map(CatalogFileDto::from).collect(),
            loose_files: node
                .loose_files
                .into_iter()
                .map(CatalogFileDto::from)
                .collect(),
            children: node
                .children
                .into_iter()
                .map(CatalogNodeDto::from)
                .collect(),
        }
    }
}

impl From<CatalogFile> for CatalogFileDto {
    fn from(file: CatalogFile) -> Self {
        Self {
            media_id: file.media_id,
            name: file.name,
            path: file.path,
            size: file.size,
            kind: media_kind(file.kind),
            availability: availability(file.availability),
            unverified: file.unverified,
        }
    }
}

/// Traduz o papel de um nó para a string estável do contrato.
fn node_role(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Category => "category",
        NodeRole::Course => "course",
        NodeRole::Module => "module",
        NodeRole::Lesson => "lesson",
    }
}

/// Traduz o tipo de um arquivo para a string estável do contrato.
fn media_kind(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Media => "media",
        MediaKind::Subtitle => "subtitle",
        MediaKind::Material => "material",
    }
}

/// Traduz a disponibilidade de um arquivo para a string estável do contrato.
fn availability(value: Availability) -> &'static str {
    match value {
        Availability::Available => "available",
        Availability::Missing => "missing",
    }
}

/// Resumo de um rescan explícito da biblioteca, sem caminho do sistema de arquivos.
///
/// É a projeção serializável de [`learnvault_core::RescanSummary`]: só as contagens do
/// contrato, sem o identificador interno do histórico. Nenhum campo carrega localização.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RescanSummaryDto {
    /// Verdadeiro quando a varredura cobriu toda a biblioteca.
    pub complete: bool,
    /// Arquivos novos inseridos no catálogo.
    pub inserted: usize,
    /// Registros já conhecidos que a varredura reafirmou.
    pub updated: usize,
    /// Registros marcados como ausentes.
    pub marked_missing: usize,
    /// Registros preservados por estarem em uma área não verificada.
    pub preserved: usize,
    /// Chaves relativas ambíguas recusadas nesta varredura.
    pub ambiguous: Vec<String>,
}

impl From<RescanSummary> for RescanSummaryDto {
    fn from(summary: RescanSummary) -> Self {
        Self {
            complete: summary.complete,
            inserted: summary.inserted,
            updated: summary.updated,
            marked_missing: summary.marked_missing,
            preserved: summary.preserved,
            ambiguous: summary.ambiguous,
        }
    }
}
