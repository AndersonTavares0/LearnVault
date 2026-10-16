//! Comandos IPC do desktop e as funções livres que eles embrulham.
//!
//! Cada wrapper `#[tauri::command]` é fino: delega para uma função livre que não conhece
//! o Tauri. Os testes exercitam as funções livres; o runtime do webview fica fora delas.
//!
//! O comando de seleção da biblioteca não aceita argumentos: o webview apenas dispara o
//! seletor nativo, que roda no Rust, e recebe de volta somente o booleano de configuração.
//!
//! Os comandos de estudo aceitam apenas dados do catálogo (`media_id`, posição, conclusão e
//! limite). O dono do progresso é fixo no Rust: nenhum comando aceita `owner_id`.

use std::path::PathBuf;

use learnvault_core::Database;
use tauri::State;

use crate::config::DesktopConfigDto;
use crate::dto::{CatalogDto, RecentStudyDto, RescanSummaryDto, StudyProgressDto};
use crate::error::IpcError;
use crate::state::DesktopState;

/// Identificador local da biblioteca, o mesmo usado pelo servidor.
///
/// O desktop lê a mesma tabela do domínio, então reusa a identidade para não criar um
/// catálogo paralelo quando a configuração da raiz chegar.
pub const LIBRARY_ID: &str = "library-1";

/// Constrói o DTO do catálogo a partir dos dados persistidos no banco local.
///
/// Função livre e testável: não conhece o Tauri e qualquer falha do núcleo é traduzida
/// para um erro IPC sanitizado, sem ecoar o erro de `Display` do domínio.
pub fn catalog(database: &Database) -> Result<CatalogDto, IpcError> {
    learnvault_core::build_catalog(database, LIBRARY_ID)
        .map(CatalogDto::from)
        .map_err(|_| IpcError::catalog_unavailable())
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

/// Comando IPC: devolve a situação de configuração do desktop.
///
/// Sempre responde com o booleano; o caminho da biblioteca nunca cruza o IPC.
#[tauri::command]
pub fn desktop_config(state: State<'_, DesktopState>) -> DesktopConfigDto {
    state.config_status()
}

/// Comando IPC: abre o seletor nativo e persiste a raiz da biblioteca escolhida.
///
/// Sem argumentos: o webview não envia caminho algum. O seletor e a gravação rodam em
/// `spawn_blocking` para não prender a main thread, e o resultado é apenas o booleano de
/// configuração. Cancelar preserva o estado anterior; uma falha vira erro sanitizado.
#[tauri::command]
pub async fn desktop_select_library(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DesktopConfigDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.select_library(|| pick_folder(&app)))
        .await
        .map_err(|_| IpcError::library_config_unavailable())?
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
