//! Comandos IPC do desktop e as funções livres que eles embrulham.
//!
//! Cada wrapper `#[tauri::command]` é fino: delega para uma função livre que não conhece
//! o Tauri. Os testes exercitam as funções livres; o runtime do webview fica fora delas.
//!
//! O comando de importação não aceita argumentos: o webview apenas dispara o seletor
//! nativo, que roda no Rust, e recebe de volta somente o booleano de configuração. A
//! remoção aceita apenas a identidade opaca do curso — nunca um caminho.
//!
//! Os comandos de estudo aceitam apenas dados do catálogo (`media_id`, posição, conclusão e
//! limite). O dono do progresso é fixo no Rust: nenhum comando aceita `owner_id`.

use std::path::PathBuf;

use learnvault_core::{CatalogNode, Database, NodeRole};
use tauri::State;

use crate::config::DesktopConfigDto;
use crate::courses::CourseEntry;
use crate::dto::{
    CatalogDto, CatalogNodeDto, MediaRemovalImpactDto, MediaRemovalOutcomeDto, PomodoroDto,
    RecentStudyDto, RescanSummaryDto, StudyAnalyticsDto, StudyProgressDto,
};
use crate::error::IpcError;
use crate::state::DesktopState;

/// Constrói o DTO do catálogo agregado a partir dos dados persistidos no banco local.
///
/// Função livre e testável: não conhece o Tauri e qualquer falha do núcleo é traduzida
/// para um erro IPC sanitizado, sem ecoar o erro de `Display` do domínio. Cada curso
/// registrado vira um nó de topo com identidade opaca e título derivado da pasta, exposto
/// como `nodes` (árvore canônica) e como `categories` (lista de cursos de topo).
pub fn catalog(database: &Database, courses: &[CourseEntry]) -> Result<CatalogDto, IpcError> {
    let mut complete = true;
    let mut nodes = Vec::with_capacity(courses.len());
    let mut courses: Vec<_> = courses.iter().collect();
    courses.sort_by(|a, b| {
        let a_name = a
            .root
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let b_name = b
            .root
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        learnvault_core::natural_cmp(a_name, b_name).then_with(|| a.id.cmp(&b.id))
    });
    for course in courses {
        let inner = learnvault_core::build_catalog(database, &course.id)
            .map_err(|_| IpcError::catalog_unavailable())?;
        let mut categories: Vec<_> = inner
            .categories
            .into_iter()
            .map(learnvault_core::course_file_lessons)
            .collect();
        learnvault_core::apply_saved_catalog_order(
            database,
            crate::study::LOCAL_OWNER_ID,
            &course.id,
            &mut categories,
            "",
        )
        .map_err(|_| IpcError::catalog_unavailable())?;
        let shaped = learnvault_core::Catalog {
            complete: inner.complete,
            loose_files: inner.loose_files,
            categories,
        };
        complete = complete && shaped.complete;
        nodes.push(CatalogNodeDto::course(&course.id, &course.title(), shaped));
    }
    Ok(CatalogDto {
        complete,
        loose_files: Vec::new(),
        // O mesmo nó de topo de curso alimenta os dois campos: `nodes` é a árvore canônica
        // e `categories` é a lista de cursos de topo do contrato do IPC.
        nodes: nodes.clone(),
        categories: nodes,
    })
}

/// Salva a ordem de uma lista de irmãos real do catálogo local.
///
/// O dono é fixo no Rust. O webview só pode apontar para um curso registrado, um pai
/// reordenável e a permutação exata dos caminhos relativos que aparecem como irmãos.
pub fn reorder_catalog(
    database: &Database,
    courses: &[CourseEntry],
    course_id: &str,
    parent_path: &str,
    child_paths: &[String],
) -> Result<(), IpcError> {
    let course = courses
        .iter()
        .find(|course| course.id == course_id)
        .ok_or_else(IpcError::invalid_catalog_order)?;
    let siblings = reorderable_siblings(database, course, parent_path)?
        .ok_or_else(IpcError::invalid_catalog_order)?;
    if !same_paths(&siblings, child_paths) {
        return Err(IpcError::invalid_catalog_order());
    }
    let child_paths: Vec<_> = child_paths.iter().map(String::as_str).collect();
    learnvault_core::persist_catalog_order(
        database,
        crate::study::LOCAL_OWNER_ID,
        &course.id,
        parent_path,
        &child_paths,
    )
    .map_err(|error| match error {
        learnvault_core::Error::InvalidCatalogOrder => IpcError::invalid_catalog_order(),
        _ => IpcError::database_unavailable(),
    })
}

/// Remove somente a preferência da lista selecionada no catálogo local.
pub fn reset_catalog_order(
    database: &Database,
    courses: &[CourseEntry],
    course_id: &str,
    parent_path: &str,
) -> Result<(), IpcError> {
    let course = courses
        .iter()
        .find(|course| course.id == course_id)
        .ok_or_else(IpcError::invalid_catalog_order)?;
    reorderable_siblings(database, course, parent_path)?
        .ok_or_else(IpcError::invalid_catalog_order)?;
    learnvault_core::reset_catalog_order(
        database,
        crate::study::LOCAL_OWNER_ID,
        &course.id,
        parent_path,
    )
    .map_err(|error| match error {
        learnvault_core::Error::InvalidCatalogOrder => IpcError::invalid_catalog_order(),
        _ => IpcError::database_unavailable(),
    })
}

fn reorderable_siblings(
    database: &Database,
    course: &CourseEntry,
    parent_path: &str,
) -> Result<Option<Vec<String>>, IpcError> {
    let catalog = learnvault_core::build_catalog(database, &course.id)
        .map_err(|_| IpcError::database_unavailable())?;
    let mut roots: Vec<_> = catalog
        .categories
        .into_iter()
        .map(learnvault_core::course_file_lessons)
        .collect();
    if parent_path.is_empty() {
        return Ok(Some(roots.into_iter().map(|node| node.path).collect()));
    }
    let Some(parent) = find_node_mut(&mut roots, parent_path) else {
        return Ok(None);
    };
    if !matches!(parent.role, NodeRole::Category | NodeRole::Module) {
        return Ok(None);
    }
    Ok(Some(
        parent
            .children
            .iter()
            .map(|node| node.path.clone())
            .collect(),
    ))
}

fn find_node_mut<'a>(nodes: &'a mut [CatalogNode], path: &str) -> Option<&'a mut CatalogNode> {
    for node in nodes {
        if node.path == path {
            return Some(node);
        }
        if let Some(found) = find_node_mut(&mut node.children, path) {
            return Some(found);
        }
    }
    None
}

fn same_paths(existing: &[String], requested: &[String]) -> bool {
    if existing.len() != requested.len() {
        return false;
    }
    let existing: std::collections::HashSet<_> = existing.iter().collect();
    let requested: std::collections::HashSet<_> = requested.iter().collect();
    existing.len() == requested.len() && existing == requested
}

/// Comando IPC: devolve o catálogo do desktop.
///
/// O comando é `async` de propósito: um comando síncrono do Tauri roda na main thread do
/// webview, e a leitura bloqueante do SQLite travaria a UI. O trabalho é deslocado para
/// `spawn_blocking` do runtime do Tauri — sem adicionar `tokio` como dependência direta —
/// mantendo o contrato, os erros, o estado e as funções livres testáveis.
#[tauri::command]
pub async fn desktop_catalog(state: State<'_, DesktopState>) -> Result<CatalogDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.catalog())
        .await
        .map_err(|_| IpcError::database_unavailable())?
}

/// Comando IPC: persiste a ordem enviada para os irmãos atualmente exibidos.
#[tauri::command]
pub async fn desktop_reorder_catalog(
    state: State<'_, DesktopState>,
    course_id: String,
    parent_path: String,
    child_paths: Vec<String>,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.reorder_catalog(&course_id, &parent_path, &child_paths)
    })
    .await
    .map_err(|_| IpcError::database_unavailable())?
}

/// Comando IPC: restaura a ordenação natural somente para a lista selecionada.
#[tauri::command]
pub async fn desktop_reset_catalog_order(
    state: State<'_, DesktopState>,
    course_id: String,
    parent_path: String,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.reset_catalog_order(&course_id, &parent_path)
    })
    .await
    .map_err(|_| IpcError::database_unavailable())?
}

/// Comando IPC: abre o seletor nativo e registra a pasta escolhida como um curso.
///
/// Sem argumentos: o webview não envia caminho algum. O seletor e a gravação rodam em
/// `spawn_blocking` para não prender a main thread, e o resultado é apenas o booleano de
/// configuração. Cancelar preserva o estado anterior; uma falha vira erro sanitizado.
#[tauri::command]
pub async fn desktop_import_course(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DesktopConfigDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.import_course(|| pick_folder(&app)))
        .await
        .map_err(|_| IpcError::courses_config_unavailable())?
}

/// Comando IPC: remove um curso do registro pela identidade opaca.
///
/// A remoção não toca nos arquivos e é idempotente: uma identidade desconhecida apenas
/// devolve o estado atual. O webview nunca envia nem recebe um caminho.
#[tauri::command]
pub async fn desktop_remove_course(
    state: State<'_, DesktopState>,
    course_id: String,
) -> Result<DesktopConfigDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.remove_course(&course_id))
        .await
        .map_err(|_| IpcError::courses_config_unavailable())?
}

/// Comando IPC: executa o rescan explícito da biblioteca configurada.
///
/// Sem argumentos: o webview não envia caminho nem parâmetro algum, apenas dispara a
/// indexação quando o usuário pede. A varredura lê o sistema de arquivos e o SQLite de
/// forma bloqueante, então roda em `spawn_blocking` — como o catálogo —, sem `tokio`
/// direto. O resultado é o resumo sanitizado do núcleo; sem raiz configurada ou com a
/// raiz inacessível a falha vira um erro estável. Chamadas concorrentes devolvem
/// `rescan_in_progress`, sem cancelar nem corromper o estado.
#[tauri::command]
pub async fn desktop_rescan(state: State<'_, DesktopState>) -> Result<RescanSummaryDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.rescan())
        .await
        .map_err(|_| IpcError::rescan_unavailable())?
}

/// Comando IPC: lê o progresso de estudo do dono local em uma mídia conhecida.
///
/// O único argumento é o `media_id` vindo do catálogo: o dono é o identificador local fixo
/// resolvido no Rust ([`crate::study::LOCAL_OWNER_ID`]) e nunca cruza o IPC. Como o
/// catálogo, a leitura bloqueante do SQLite roda em `spawn_blocking` para não prender a
/// main thread. Uma mídia conhecida sem progresso responde com `recorded: false`; uma
/// mídia desconhecida vira o erro estável `media_not_found`.
#[tauri::command]
pub async fn desktop_study_progress(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<StudyProgressDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.study_progress(&media_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Comando IPC: registra a posição de retomada do dono local, preservando a conclusão.
///
/// O webview envia apenas `media_id` e `position_seconds` — nunca o dono. A regra de
/// independência entre posição e conclusão, a validação da posição negativa e a
/// idempotência de um valor repetido pertencem ao núcleo; o comando apenas delega em
/// `spawn_blocking`.
#[tauri::command]
pub async fn desktop_record_position(
    state: State<'_, DesktopState>,
    media_id: String,
    position_seconds: i64,
) -> Result<StudyProgressDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.record_position(&media_id, position_seconds))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Comando IPC: marca ou desmarca a conclusão do dono local, preservando a posição.
///
/// O webview envia apenas `media_id` e `completed`; o dono permanece no Rust. Uma mídia
/// desconhecida vira o erro estável `media_not_found` e nada é gravado.
#[tauri::command]
pub async fn desktop_set_completed(
    state: State<'_, DesktopState>,
    media_id: String,
    completed: bool,
) -> Result<StudyProgressDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.set_completed(&media_id, completed))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Comando IPC: lista o progresso de estudo do dono local, do mais recente ao mais antigo.
///
/// O limite é opcional; sem ele o adaptador aplica o padrão do contrato e o teto do núcleo
/// continua valendo. O histórico é sempre do dono local: o webview não envia `owner_id`.
#[tauri::command]
pub async fn desktop_recent_study(
    state: State<'_, DesktopState>,
    limit: Option<usize>,
) -> Result<RecentStudyDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.recent_study(limit))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Comando IPC: consulta métricas agregadas do estudo do dono local.
#[tauri::command]
pub async fn desktop_study_analytics(
    state: State<'_, DesktopState>,
) -> Result<StudyAnalyticsDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.study_analytics())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// L? a sess?o Pomodoro do dono local.
#[tauri::command]
pub async fn desktop_pomodoro(state: State<'_, DesktopState>) -> Result<PomodoroDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.pomodoro())
        .await
        .map_err(|_| IpcError::pomodoro_unavailable())?
}
/// Inicia ou retoma uma sess?o Pomodoro.
#[tauri::command]
pub async fn desktop_start_pomodoro(
    state: State<'_, DesktopState>,
    duration_seconds: i64,
) -> Result<PomodoroDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.start_pomodoro(duration_seconds))
        .await
        .map_err(|_| IpcError::pomodoro_unavailable())?
}
/// Pausa uma sess?o Pomodoro.
#[tauri::command]
pub async fn desktop_pause_pomodoro(
    state: State<'_, DesktopState>,
    elapsed_seconds: i64,
) -> Result<PomodoroDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.pause_pomodoro(elapsed_seconds))
        .await
        .map_err(|_| IpcError::pomodoro_unavailable())?
}
/// Reinicia uma sess?o Pomodoro.
#[tauri::command]
pub async fn desktop_reset_pomodoro(
    state: State<'_, DesktopState>,
) -> Result<PomodoroDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.reset_pomodoro())
        .await
        .map_err(|_| IpcError::pomodoro_unavailable())?
}

/// Comando IPC: consulta o impacto da remoção de uma mídia.
#[tauri::command]
pub async fn desktop_media_removal_impact(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<MediaRemovalImpactDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.media_removal_impact(&media_id))
        .await
        .map_err(|_| IpcError::database_unavailable())?
}

/// Comando IPC: remove uma mídia do catálogo local (tombstone persistente).
#[tauri::command]
pub async fn desktop_remove_media(
    state: State<'_, DesktopState>,
    media_id: String,
    confirm_progress: bool,
) -> Result<MediaRemovalOutcomeDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.remove_media(&media_id, confirm_progress))
        .await
        .map_err(|_| IpcError::database_unavailable())?
}

/// Abre o seletor nativo de diretórios e devolve o caminho escolhido, se houver.
///
/// O diálogo é interno ao Rust (`tauri-plugin-dialog`); ele não é exposto ao webview.
/// Cancelar devolve `None`, sem caminho nem erro.
fn pick_folder(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;

    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|path| path.into_path().ok())
}
