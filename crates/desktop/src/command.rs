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

use std::path::{Path, PathBuf};

use learnvault_core::{Database, FocusKind, FocusSessionInput};
use tauri::State;

use crate::config::DesktopConfigDto;
use crate::courses::CourseEntry;
use crate::dto::{
    CatalogDto, CatalogNodeDto, CoverDto, CoverRemovalDto, FocusSessionDto, FocusSummaryDto,
    MediaRemovalImpactDto, MediaRemovalOutcomeDto, RecentStudyDto, RescanSummaryDto,
    RewardSummaryDto, StudyNoteDto, StudyProgressDto, StudySummaryDto,
};
use crate::error::IpcError;
use crate::ordering::CourseOrders;
use crate::state::DesktopState;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
/// Camel-case IPC payload for recording one focus session.
pub struct FocusSessionRequest {
    id: String,
    kind: String,
    planned_seconds: i64,
    actual_seconds: i64,
    completed: bool,
    started_at: i64,
    ended_at: i64,
}

/// Constrói o DTO do catálogo agregado a partir dos dados persistidos no banco local.
///
/// Função livre e testável: não conhece o Tauri e qualquer falha do núcleo é traduzida
/// para um erro IPC sanitizado, sem ecoar o erro de `Display` do domínio. Cada curso
/// registrado vira um nó de topo com identidade opaca, título derivado da pasta e a versão
/// da capa personalizada (derivada do diretório de capas, nunca de um caminho do curso),
/// exposto como `nodes` (árvore canônica) e como `categories` (lista de cursos de topo).
///
/// A preferência de ordenação é aplicada por sobreposição sobre a ordem natural da
/// subárvore de cada curso; um curso sem preferência permanece na ordem automática.
pub fn catalog(
    database: &Database,
    courses: &[CourseEntry],
    covers_dir: &Path,
) -> Result<CatalogDto, IpcError> {
    catalog_with_orders(database, courses, &CourseOrders::new(), covers_dir)
}

/// Como [`catalog`], aplicando as preferências de ordenação manual por curso.
pub fn catalog_with_orders(
    database: &Database,
    courses: &[CourseEntry],
    orders: &CourseOrders,
    covers_dir: &Path,
) -> Result<CatalogDto, IpcError> {
    let empty = learnvault_core::CatalogOrder::new();
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
        let is_complete = inner.complete;
        let (mut categories, loose_files) = learnvault_core::shape_course_catalog(inner);
        learnvault_core::apply_order(&mut categories, orders.get(&course.id).unwrap_or(&empty));
        let shaped = learnvault_core::Catalog {
            complete: is_complete,
            loose_files,
            categories,
        };
        complete = complete && shaped.complete;
        let source_folder = course
            .root
            .as_path()
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .map(str::to_owned);
        nodes.push(CatalogNodeDto::course(
            &course.id,
            &course.title(),
            shaped,
            crate::covers::cover_version(covers_dir, &course.id),
            source_folder,
        ));
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

/// Comando IPC: abre o seletor nativo de imagens e registra a capa de um curso.
///
/// O webview envia apenas a identidade opaca do curso — nunca um caminho. O seletor, a
/// leitura e a gravação atômica rodam em `spawn_blocking` para não prender a main thread. O
/// tipo (magic bytes) e o tamanho são validados no Rust: uma imagem fora do contrato vira
/// `invalid_cover` e nada é gravado. Cancelar preserva a capa anterior.
#[tauri::command]
pub async fn desktop_import_course_cover(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
    course_id: String,
) -> Result<CoverDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.import_course_cover(&course_id, || pick_cover(&app))
    })
    .await
    .map_err(|_| IpcError::cover_unavailable())?
}

/// Comando IPC: remove a capa personalizada de um curso pela identidade opaca.
///
/// A remoção não toca na pasta do curso e é idempotente: um curso sem capa devolve
/// `removed: false`. Nenhum caminho cruza o IPC em nenhuma direção.
#[tauri::command]
pub async fn desktop_remove_course_cover(
    state: State<'_, DesktopState>,
    course_id: String,
) -> Result<CoverRemovalDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.remove_course_cover(&course_id))
        .await
        .map_err(|_| IpcError::cover_unavailable())?
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

/// Comando IPC: define a ordem manual dos irmãos de um nó do curso.
///
/// O webview envia apenas a identidade opaca do curso, o caminho relativo do pai e a
/// ordem desejada dos filhos (caminhos relativos). Nenhum caminho do sistema de arquivos
/// cruza o IPC. A validação (permutação exata dos irmãos) e a gravação atômica rodam no
/// Rust, em `spawn_blocking`; uma lista inválida vira `invalid_order` sem gravar nada.
#[tauri::command]
pub async fn desktop_set_course_order(
    state: State<'_, DesktopState>,
    course_id: String,
    parent: String,
    children: Vec<String>,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.set_course_order(&course_id, &parent, &children)
    })
    .await
    .map_err(|_| IpcError::order_unavailable())?
}

/// Comando IPC: restaura a ordem automática de um pai específico ou de todo o curso.
///
/// `parent` ausente restaura o curso inteiro. A resposta diz se havia uma preferência a
/// remover, para que a interface seja idempotente sem tocar nos arquivos do curso.
#[tauri::command]
pub async fn desktop_reset_course_order(
    state: State<'_, DesktopState>,
    course_id: String,
    parent: Option<String>,
) -> Result<bool, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.reset_course_order(&course_id, parent.as_deref())
    })
    .await
    .map_err(|_| IpcError::order_unavailable())?
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

/// Comando IPC: resumo de ofensiva e dias ativos do dono local.
///
/// O webview não envia `owner_id`; o resumo cobre todo o histórico local e a semana
/// corrente, sempre no fuso do desktop.
#[tauri::command]
pub async fn desktop_study_summary(
    state: State<'_, DesktopState>,
) -> Result<StudySummaryDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.study_summary())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Comando IPC: lista as anotações do dono local.
///
/// Se `mediaId` for informado, filtra por essa mídia; caso contrário, devolve todas
/// as anotações registradas para busca global.
#[tauri::command]
pub async fn desktop_list_notes(
    state: State<'_, DesktopState>,
    media_id: Option<String>,
) -> Result<Vec<StudyNoteDto>, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_notes(media_id.as_deref()))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Comando IPC: cria uma nova anotação de estudo para o dono local.
#[tauri::command]
pub async fn desktop_create_note(
    state: State<'_, DesktopState>,
    media_id: String,
    position_seconds: i64,
    content: String,
) -> Result<StudyNoteDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.create_note(&media_id, position_seconds, &content)
    })
    .await
    .map_err(|_| IpcError::study_unavailable())?
}

/// Comando IPC: atualiza o texto de uma anotação pertencente ao dono local.
#[tauri::command]
pub async fn desktop_update_note(
    state: State<'_, DesktopState>,
    note_id: String,
    content: String,
) -> Result<StudyNoteDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.update_note(&note_id, &content))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Comando IPC: exclui uma anotação de estudo pertencente ao dono local.
#[tauri::command]
pub async fn desktop_delete_note(
    state: State<'_, DesktopState>,
    note_id: String,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.delete_note(&note_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Persiste uma sessão Pomodoro idempotente para o perfil local.
#[tauri::command]
pub async fn desktop_record_focus_session(
    state: State<'_, DesktopState>,
    session: FocusSessionRequest,
) -> Result<FocusSessionDto, IpcError> {
    let kind = match session.kind.as_str() {
        "focus" => FocusKind::Focus,
        "short" => FocusKind::ShortBreak,
        "long" => FocusKind::LongBreak,
        _ => return Err(IpcError::invalid_focus_session()),
    };
    let input = FocusSessionInput {
        id: session.id,
        kind,
        planned_seconds: session.planned_seconds,
        actual_seconds: session.actual_seconds,
        completed: session.completed,
        started_at: session.started_at,
        ended_at: session.ended_at,
    };
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.record_focus_session(input))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Lê as métricas de foco persistidas no banco local.
#[tauri::command]
pub async fn desktop_focus_summary(
    state: State<'_, DesktopState>,
) -> Result<FocusSummaryDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.focus_summary())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Lê XP, metas e missões persistidos no banco local.
#[tauri::command]
pub async fn desktop_reward_summary(
    state: State<'_, DesktopState>,
) -> Result<RewardSummaryDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.reward_summary())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Resgata uma missão diária do perfil local.
#[tauri::command]
pub async fn desktop_claim_daily_mission(
    state: State<'_, DesktopState>,
    mission_id: String,
) -> Result<bool, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.claim_daily_mission(&mission_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Lê a meta semanal do perfil local.
#[tauri::command]
pub async fn desktop_weekly_goal(state: State<'_, DesktopState>) -> Result<i64, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.weekly_goal())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Atualiza a meta semanal do perfil local.
#[tauri::command]
pub async fn desktop_set_weekly_goal(
    state: State<'_, DesktopState>,
    weekly_goal: i64,
) -> Result<i64, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.set_weekly_goal(weekly_goal))
        .await
        .map_err(|_| IpcError::study_unavailable())?
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

/// Abre o seletor nativo de imagens e devolve o arquivo escolhido, se houver.
///
/// Filtro por extensão apenas para orientar o usuário: a decisão real do formato é dos
/// bytes iniciais do arquivo ([`crate::covers::detect_image_type`]), nunca do nome. Como o
/// seletor de pastas, o diálogo é interno ao Rust e não é exposto ao webview.
fn pick_cover(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;

    app.dialog()
        .file()
        .set_title("Escolha a imagem da capa")
        .add_filter("Imagem (PNG, JPEG, WebP)", &["png", "jpg", "jpeg", "webp"])
        .blocking_pick_file()
        .and_then(|path| path.into_path().ok())
}

/// Consulta a disponibilidade de nova versão via tauri-plugin-updater de forma não-bloqueante e segura.
///
/// Diferencia a ausência de atualização (`available: false`) de falhas de conexão/rede (`IpcError::update_check_failed()`),
/// mantendo o aplicativo totalmente funcional em modo offline sem travar nem expor dados sensíveis.
#[tauri::command]
pub async fn desktop_check_update(
    app: tauri::AppHandle,
) -> Result<crate::dto::DesktopUpdateInfoDto, IpcError> {
    use tauri_plugin_updater::UpdaterExt;

    let current_version = app.package_info().version.to_string();
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(_) => {
            return Ok(crate::dto::DesktopUpdateInfoDto {
                available: false,
                current_version,
                version: None,
                date: None,
                body: None,
            });
        }
    };

    match updater.check().await {
        Ok(Some(update)) => Ok(crate::dto::DesktopUpdateInfoDto {
            available: true,
            current_version,
            version: Some(update.version),
            date: update.date.map(|d| d.to_string()),
            body: update.body,
        }),
        Ok(None) => Ok(crate::dto::DesktopUpdateInfoDto {
            available: false,
            current_version,
            version: None,
            date: None,
            body: None,
        }),
        Err(_) => Err(IpcError::update_check_failed()),
    }
}

/// Baixa e aplica a atualização assinada (Ed25519) e reinicia o aplicativo.
#[tauri::command]
pub async fn desktop_install_update(app: tauri::AppHandle) -> Result<bool, IpcError> {
    use tauri_plugin_updater::UpdaterExt;

    let updater = app.updater().map_err(|_| IpcError::update_unavailable())?;
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => return Ok(false),
        Err(_) => return Err(IpcError::update_check_failed()),
    };

    update
        .download_and_install(|_downloaded, _total| {}, || {})
        .await
        .map_err(|_| IpcError::update_unavailable())?;

    app.restart();
}
