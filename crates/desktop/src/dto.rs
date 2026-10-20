//! DTOs serializáveis do catálogo e do progresso de estudo do desktop.
//!
//! Os tipos do núcleo não são serializados diretamente: cada enum vira uma string estável
//! e o shape snake_case (`loose_files`, `media_id`, ...) é idêntico ao do adaptador HTTP,
//! de modo que o `normalizeCatalog` e o `normalizeStudyProgress` da SPA aceitam o payload do
//! desktop sem alteração.

use learnvault_core::{
    Availability, Catalog, CatalogFile, CatalogNode, MediaKind, NodeRole, PomodoroState,
    PomodoroStatus, RescanSummary, StudyAnalytics, StudyNote, StudyProgress,
};
use serde::Serialize;

/// Catálogo do desktop derivado do banco local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogDto {
    /// Verdadeiro quando a última varredura cobriu todos os cursos.
    pub complete: bool,
    /// Arquivos soltos de uma biblioteca global. O desktop agrega cursos e mantém esta
    /// lista vazia: arquivos soltos pertencem ao nó do curso correspondente.
    pub loose_files: Vec<CatalogFileDto>,
    /// Árvore recursiva canônica, o contrato preferido pela SPA.
    pub nodes: Vec<CatalogNodeDto>,
    /// Cursos, na ordem de importação. Cada nó de topo tem papel `course`.
    pub categories: Vec<CatalogNodeDto>,
}

/// Nó da árvore do catálogo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogNodeDto {
    /// Título apresentado, sem prefixos numéricos nem separadores de ordenação.
    pub title: String,
    /// Caminho relativo completo que identifica o nó. Vazio no nó de topo do curso.
    pub path: String,
    /// Papel do nó: `category`, `course`, `module` ou `lesson`.
    pub role: &'static str,
    /// Identidade do curso que contém o nó, presente apenas no nó de topo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_id: Option<String>,
    /// Segundos de modificação da capa personalizada do curso, quando existir. É a versão
    /// usada pela SPA para invalidar a URL (`?v=`); ausente quando não há capa.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_version: Option<i64>,
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
    /// Modificação da mídia em segundos desde a época Unix na última observação.
    pub mtime_seconds: i64,
    /// Tipo do arquivo: `media`, `subtitle` ou `material`.
    pub kind: &'static str,
    /// Disponibilidade na última varredura: `available` ou `missing`.
    pub availability: &'static str,
    /// Verdadeiro quando o arquivo está sob uma área não verificada.
    pub unverified: bool,
}

impl From<CatalogNode> for CatalogNodeDto {
    fn from(node: CatalogNode) -> Self {
        Self {
            title: node.title,
            path: node.path,
            role: node_role(node.role),
            course_id: None,
            cover_version: None,
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

impl CatalogNodeDto {
    /// Nó de topo de um curso registrado: identidade opaca, título da pasta, a versão da
    /// capa personalizada (quando existir) e a subárvore derivada dos dados persistidos.
    pub fn course(
        course_id: &str,
        title: &str,
        catalog: Catalog,
        cover_version: Option<i64>,
    ) -> Self {
        Self {
            title: title.to_owned(),
            path: String::new(),
            role: "course",
            course_id: Some(course_id.to_owned()),
            cover_version,
            files: Vec::new(),
            loose_files: catalog
                .loose_files
                .into_iter()
                .map(CatalogFileDto::from)
                .collect(),
            children: catalog
                .categories
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
            mtime_seconds: file.mtime_seconds,
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

/// Resumo de um rescan explícito dos cursos, sem caminho do sistema de arquivos.
///
/// É a projeção serializável agregada de [`learnvault_core::RescanSummary`]: só as
/// contagens do contrato, sem o identificador interno do histórico. Nenhum campo carrega
/// localização.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RescanSummaryDto {
    /// Verdadeiro quando todas as varreduras cobriram os cursos inteiros.
    pub complete: bool,
    /// Quantidade de cursos considerados na agregação.
    pub courses: usize,
    /// Quantidade de cursos cuja pasta não pôde ser varrida.
    pub failed: usize,
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
            courses: 1,
            failed: 0,
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

/// Métricas agregadas do estudo do dono local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StudyAnalyticsDto {
    /// Quantidade de mídias com estado de estudo não removidas.
    pub tracked_count: usize,
    /// Quantidade concluída.
    pub completed_count: usize,
    /// Soma das posições de retomada em segundos.
    pub total_position_seconds: i64,
    /// Última atividade, se houver.
    pub last_activity_at: Option<i64>,
}

/// Estado Pomodoro serializável para o webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PomodoroDto {
    /// Estado textual (`idle`, `running` ou `paused`).
    pub status: &'static str,
    /// Duração alvo em segundos.
    pub duration_seconds: i64,
    /// Tempo transcorrido em segundos.
    pub elapsed_seconds: i64,
    /// Instante da última alteração.
    pub updated_at: i64,
}

impl From<PomodoroState> for PomodoroDto {
    fn from(value: PomodoroState) -> Self {
        Self {
            status: match value.status {
                PomodoroStatus::Idle => "idle",
                PomodoroStatus::Running => "running",
                PomodoroStatus::Paused => "paused",
            },
            duration_seconds: value.duration_seconds,
            elapsed_seconds: value.elapsed_seconds,
            updated_at: value.updated_at,
        }
    }
}

impl From<StudyAnalytics> for StudyAnalyticsDto {
    fn from(value: StudyAnalytics) -> Self {
        Self {
            tracked_count: value.tracked_count,
            completed_count: value.completed_count,
            total_position_seconds: value.total_position_seconds,
            last_activity_at: value.last_activity_at,
        }
    }
}

/// DTO do impacto de remoção de mídia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MediaRemovalImpactDto {
    /// Identificador da mídia consultada.
    pub media_id: String,
    /// Caminho relativo da mídia no catálogo.
    pub relative_path: String,
    /// Disponibilidade segundo a última varredura ("available" ou "missing").
    pub availability: &'static str,
    /// Quantidade de donos distintos com progresso nesta mídia.
    pub progress_owners: usize,
    /// Se a mídia já havia sido removida do catálogo.
    pub already_removed: bool,
}

impl From<learnvault_core::MediaRemovalImpact> for MediaRemovalImpactDto {
    fn from(impact: learnvault_core::MediaRemovalImpact) -> Self {
        Self {
            media_id: impact.media_id,
            relative_path: impact.relative_path,
            availability: availability(impact.availability),
            progress_owners: impact.progress_owners,
            already_removed: impact.already_removed,
        }
    }
}

/// DTO do resultado de remoção de mídia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MediaRemovalOutcomeDto {
    /// Identificador da mídia removida.
    pub media_id: String,
    /// Verdadeiro quando a remoção foi gravada agora; falso se já estava removida.
    pub removed_now: bool,
    /// Quantidade de donos com progresso preservado.
    pub preserved_owners: usize,
}

impl From<learnvault_core::MediaRemovalOutcome> for MediaRemovalOutcomeDto {
    fn from(outcome: learnvault_core::MediaRemovalOutcome) -> Self {
        Self {
            media_id: outcome.media_id,
            removed_now: outcome.removed_now,
            preserved_owners: outcome.preserved_owners,
        }
    }
}

/// DTO da capa personalizada de um curso.
///
/// Carrega apenas a versão (segundos de modificação) que a SPA usa para invalidar a URL da
/// imagem. Nem o caminho do arquivo nem o diretório de capas cruzam o IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CoverDto {
    /// Versão da capa do curso; ausente quando o curso ainda não tem capa.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_version: Option<i64>,
}

/// DTO do resultado da remoção da capa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CoverRemovalDto {
    /// Verdadeiro quando havia uma capa e ela foi removida agora.
    pub removed: bool,
}
/// Nota pessoal do dono local no shape snake_case compartilhado com o HTTP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StudyNoteDto {
    /// Identificador estável da nota.
    pub note_id: i64,
    /// Identidade estável da mídia associada.
    pub media_id: String,
    /// Texto simples.
    pub text: String,
    /// Posição opcional de reprodução em segundos.
    pub position_seconds: Option<i64>,
    /// Instante de criação em segundos Unix.
    pub created_at: i64,
    /// Instante da última alteração em segundos Unix.
    pub updated_at: i64,
}

impl From<StudyNote> for StudyNoteDto {
    fn from(note: StudyNote) -> Self {
        Self {
            note_id: note.note_id,
            media_id: note.media_id,
            text: note.text,
            position_seconds: note.position_seconds,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}

/// Lista de notas do dono local.
///
/// É a projeção serializável do mesmo contrato `{ items: [...], truncated }` do adaptador
/// HTTP: os dois transportes entregam o mesmo envelope para a SPA, que valida a forma
/// antes de usar os itens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StudyNotesDto {
    /// Notas do dono local, já ordenadas pelo núcleo.
    pub items: Vec<StudyNoteDto>,
    /// Verdadeiro quando a listagem global do dono atingiu o teto de notas.
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use super::{StudyNote, StudyNoteDto, StudyNotesDto};

    fn note() -> StudyNoteDto {
        StudyNoteDto::from(StudyNote {
            note_id: 7,
            media_id: "m1".to_owned(),
            text: "nota simples".to_owned(),
            position_seconds: Some(12),
            created_at: 1,
            updated_at: 2,
        })
    }

    #[test]
    fn the_notes_list_uses_the_same_envelope_as_the_http_adapter() {
        // A SPA valida `{ items: [...] }` nos dois transportes: um array puro faria o
        // painel de notas recusar a lista inteira no desktop.
        let value = serde_json::to_value(StudyNotesDto {
            items: vec![note()],
            truncated: false,
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "items": [{
                    "note_id": 7,
                    "media_id": "m1",
                    "text": "nota simples",
                    "position_seconds": 12,
                    "created_at": 1,
                    "updated_at": 2
                }],
                "truncated": false
            })
        );
    }

    #[test]
    fn an_empty_notes_list_keeps_the_envelope() {
        let value = serde_json::to_value(StudyNotesDto {
            items: Vec::new(),
            truncated: false,
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "items": [], "truncated": false })
        );
    }
}
