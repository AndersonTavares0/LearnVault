//! DTOs serializáveis do catálogo e do progresso de estudo do desktop.
//!
//! Os tipos do núcleo não são serializados diretamente: cada enum vira uma string estável
//! e o shape snake_case (`loose_files`, `media_id`, ...) é idêntico ao do adaptador HTTP,
//! de modo que o `normalizeCatalog` e o `normalizeStudyProgress` da SPA aceitam o payload do
//! desktop sem alteração.

use learnvault_core::{
    Availability, Catalog, CatalogFile, CatalogNode, MediaKind, NodeRole, RescanSummary,
    StudyProgress,
};
use serde::Serialize;

/// Catálogo do desktop derivado do banco local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogDto {
    /// Verdadeiro quando a última varredura cobriu toda a biblioteca.
    pub complete: bool,
    /// Arquivos na raiz da biblioteca, que não pertencem a nenhuma categoria.
    pub loose_files: Vec<CatalogFileDto>,
    /// Árvore recursiva canônica, o contrato preferido pela SPA.
    pub nodes: Vec<CatalogNodeDto>,
    /// Projeção legada do serviço Go (`categories[].courses[].modules[].lessons[]`),
    /// mantida para os consumidores antigos durante a transição.
    pub categories: Vec<LegacyCategoryDto>,
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

/// Categoria da projeção legada, na forma do serviço Go (só `courses`, sem `children`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyCategoryDto {
    /// Título apresentado.
    pub title: String,
    /// Caminho relativo completo que identifica o nó.
    pub path: String,
    /// Cursos da categoria, na gramática legada.
    pub courses: Vec<LegacyCourseDto>,
}

/// Curso da projeção legada do serviço Go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyCourseDto {
    /// Título apresentado.
    pub title: String,
    /// Caminho relativo completo que identifica o nó.
    pub path: String,
    /// Módulos do curso, na gramática legada.
    pub modules: Vec<LegacyModuleDto>,
}

/// Módulo da projeção legada do serviço Go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyModuleDto {
    /// Título apresentado.
    pub title: String,
    /// Caminho relativo completo que identifica o nó.
    pub path: String,
    /// Aulas do módulo, na gramática legada.
    pub lessons: Vec<LegacyLessonDto>,
}

/// Aula da projeção legada do serviço Go; os arquivos são classificados por tipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LegacyLessonDto {
    /// Título apresentado.
    pub title: String,
    /// Caminho relativo completo que identifica o nó.
    pub path: String,
    /// Arquivos de mídia da aula.
    pub media: Vec<CatalogFileDto>,
    /// Arquivos de legenda da aula.
    pub subtitles: Vec<CatalogFileDto>,
    /// Materiais da aula.
    pub materials: Vec<CatalogFileDto>,
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
        // `nodes` carrega a árvore recursiva canônica produzida pelo núcleo; `categories`
        // mantém a projeção legada (categorias, cursos, módulos e aulas) do serviço Go para
        // os consumidores antigos. São shapes diferentes de propósito: `nodes` é o contrato
        // preferido e `categories` preserva a compatibilidade da transição.
        let nodes: Vec<CatalogNodeDto> = catalog
            .categories
            .iter()
            .cloned()
            .map(CatalogNodeDto::from)
            .collect();
        let categories = legacy_categories(&catalog.categories);
        Self {
            complete: catalog.complete,
            loose_files: catalog
                .loose_files
                .into_iter()
                .map(CatalogFileDto::from)
                .collect(),
            nodes,
            categories,
        }
    }
}

/// Projeta a árvore canônica na forma legada do serviço Go
/// (`categories[].courses[].modules[].lessons[]`), usada pelos consumidores antigos.
///
/// A projeção desce por nós intermediários de papel desconhecido e por módulos direto sob a
/// categoria para que nenhuma aula seja descartada. O que não cabe na gramática legada
/// (arquivos soltos de categoria/curso/módulo e nós de topo que não são categoria) continua
/// disponível, sem perda, na árvore canônica `nodes`.
fn legacy_categories(nodes: &[CatalogNode]) -> Vec<LegacyCategoryDto> {
    nodes
        .iter()
        .filter(|node| node.role == NodeRole::Category)
        .map(legacy_category)
        .collect()
}

fn legacy_category(node: &CatalogNode) -> LegacyCategoryDto {
    LegacyCategoryDto {
        title: node.title.clone(),
        path: node.path.clone(),
        courses: node
            .children
            .iter()
            .map(|child| legacy_course_entry(node, child))
            .collect(),
    }
}

/// Uma aula direto sob a categoria não tem curso físico: sintetiza o curso com o título da
/// categoria, como a projeção da SPA, para não perder a aula.
fn legacy_course_entry(category: &CatalogNode, child: &CatalogNode) -> LegacyCourseDto {
    if child.role == NodeRole::Lesson {
        LegacyCourseDto {
            title: category.title.clone(),
            path: category.path.clone(),
            modules: vec![legacy_module(child)],
        }
    } else {
        legacy_course(child)
    }
}

fn legacy_course(node: &CatalogNode) -> LegacyCourseDto {
    LegacyCourseDto {
        title: node.title.clone(),
        path: node.path.clone(),
        modules: node.children.iter().map(legacy_module).collect(),
    }
}

fn legacy_module(node: &CatalogNode) -> LegacyModuleDto {
    LegacyModuleDto {
        title: node.title.clone(),
        path: node.path.clone(),
        lessons: legacy_lessons(node),
    }
}

/// Aulas de um módulo: desce por qualquer nó intermediário (grupo ou papel desconhecido)
/// para que a gramática legada nunca descarte uma aula da subárvore.
fn legacy_lessons(node: &CatalogNode) -> Vec<LegacyLessonDto> {
    if node.role == NodeRole::Lesson {
        vec![legacy_lesson(node)]
    } else {
        node.children.iter().flat_map(legacy_lessons).collect()
    }
}

fn legacy_lesson(node: &CatalogNode) -> LegacyLessonDto {
    let mut media = Vec::new();
    let mut subtitles = Vec::new();
    let mut materials = Vec::new();
    for file in node.files.iter().chain(node.loose_files.iter()) {
        let dto = CatalogFileDto::from(file.clone());
        match file.kind {
            MediaKind::Media => media.push(dto),
            MediaKind::Subtitle => subtitles.push(dto),
            MediaKind::Material => materials.push(dto),
        }
    }
    LegacyLessonDto {
        title: node.title.clone(),
        path: node.path.clone(),
        media,
        subtitles,
        materials,
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
    /// Quantidade de chaves relativas ambíguas recusadas nesta varredura.
    ///
    /// É apenas a contagem: as chaves em si são caminhos relativos e nunca cruzam o IPC.
    pub ambiguous_count: usize,
}

impl From<RescanSummary> for RescanSummaryDto {
    fn from(summary: RescanSummary) -> Self {
        Self {
            complete: summary.complete,
            inserted: summary.inserted,
            updated: summary.updated,
            marked_missing: summary.marked_missing,
            preserved: summary.preserved,
            ambiguous_count: summary.ambiguous.len(),
        }
    }
}

/// Progresso de estudo do dono local do desktop em uma mídia.
///
/// O shape é o mesmo contrato snake_case do adaptador HTTP, então o
/// `normalizeStudyProgress` da SPA aceita o payload do desktop sem alteração. `recorded`
/// distingue "nunca estudada" de "estudada na posição zero": a ausência de progresso não é
/// um erro do domínio, e o campo evita que a interface confunda as duas situações.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StudyProgressDto {
    /// Identidade estável da mídia, nunca o caminho do arquivo.
    pub media_id: String,
    /// Ponto de retomada em segundos.
    pub position_seconds: i64,
    /// Indica que a aula foi concluída.
    pub completed: bool,
    /// Instante da última alteração real, ou `None` quando nada foi gravado.
    pub updated_at: Option<i64>,
    /// Verdadeiro quando existe estado persistido para a mídia.
    pub recorded: bool,
}

impl From<StudyProgress> for StudyProgressDto {
    fn from(progress: StudyProgress) -> Self {
        Self {
            media_id: progress.media_id,
            position_seconds: progress.position_seconds,
            completed: progress.completed,
            updated_at: Some(progress.updated_at),
            recorded: true,
        }
    }
}

impl StudyProgressDto {
    /// Estado neutro de uma mídia conhecida sem progresso: nada foi gravado ainda.
    pub fn absent(media_id: &str) -> Self {
        Self {
            media_id: media_id.to_owned(),
            position_seconds: 0,
            completed: false,
            updated_at: None,
            recorded: false,
        }
    }
}

/// Histórico recente de estudo do dono local do desktop.
///
/// É a projeção serializável do mesmo contrato `{ items: [...] }` do adaptador HTTP, do
/// mais recente ao mais antigo. Só mídias presentes no catálogo local aparecem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentStudyDto {
    /// Itens de progresso, já ordenados pelo núcleo.
    pub items: Vec<StudyProgressDto>,
}
