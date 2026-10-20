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

use learnvault_core::Database;
use tauri::State;

use crate::config::DesktopConfigDto;
use crate::courses::CourseEntry;
use crate::dto::{
    CatalogDto, CatalogNodeDto, CoverDto, CoverRemovalDto, MediaRemovalImpactDto,
    MediaRemovalOutcomeDto, PomodoroDto, RecentStudyDto, RescanSummaryDto, StudyAnalyticsDto,
    StudyNoteDto, StudyNotesDto, StudyProgressDto,
};
use crate::error::IpcError;
use crate::ordering::CourseOrders;
use crate::state::DesktopState;

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
        let mut categories: Vec<learnvault_core::CatalogNode> = inner
            .categories
            .into_iter()
            .map(learnvault_core::course_file_lessons)
            .collect();
        learnvault_core::apply_order(&mut categories, orders.get(&course.id).unwrap_or(&empty));
        let shaped = learnvault_core::Catalog {
            complete: inner.complete,
            loose_files: inner.loose_files,
            categories,
        };
        complete = complete && shaped.complete;
        nodes.push(CatalogNodeDto::course(
            &course.id,
            &course.title(),
            shaped,
            crate::covers::cover_version(covers_dir, &course.id),
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

/// Lista as notas do dono local para a mídia indicada; o dono nunca cruza o IPC.
#[tauri::command]
pub async fn desktop_study_notes(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<StudyNotesDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.study_notes(&media_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Lista todas as notas do dono local, das mais recentes para as mais antigas.
///
/// O comando não recebe dono nem mídia: a listagem global é do dono local fixo, e o
/// envelope avisa quando o teto truncou o resultado.
#[tauri::command]
pub async fn desktop_study_notes_all(
    state: State<'_, DesktopState>,
) -> Result<StudyNotesDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.study_notes_all())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Cria nota para o dono local, sem aceitar identidade de dono do webview.
#[tauri::command]
pub async fn desktop_create_study_note(
    state: State<'_, DesktopState>,
    media_id: String,
    text: String,
    position_seconds: Option<i64>,
) -> Result<StudyNoteDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let database = state.lock_database()?;
        learnvault_core::create_note(
            &database,
            crate::study::LOCAL_OWNER_ID,
            &media_id,
            &text,
            position_seconds,
        )
        .map(StudyNoteDto::from)
        .map_err(crate::study::note_error)
    })
    .await
    .map_err(|_| IpcError::study_unavailable())?
}

/// Atualiza uma nota pertencente ao dono local.
#[tauri::command]
pub async fn desktop_update_study_note(
    state: State<'_, DesktopState>,
    note_id: i64,
    text: String,
    position_seconds: Option<i64>,
) -> Result<StudyNoteDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let database = state.lock_database()?;
        learnvault_core::update_note(
            &database,
            crate::study::LOCAL_OWNER_ID,
            note_id,
            &text,
            position_seconds,
        )
        .map(StudyNoteDto::from)
        .map_err(crate::study::note_error)
    })
    .await
    .map_err(|_| IpcError::study_unavailable())?
}

/// Exclui uma nota pertencente ao dono local.
#[tauri::command]
pub async fn desktop_delete_study_note(
    state: State<'_, DesktopState>,
    note_id: i64,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let database = state.lock_database()?;
        learnvault_core::delete_note(&database, crate::study::LOCAL_OWNER_ID, note_id)
            .map_err(crate::study::note_error)
    })
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

#[cfg(test)]
mod tests {
    use crate::error::IpcErrorCode;
    use crate::study::note_error;
    use learnvault_core::Error as CoreError;

    #[test]
    fn note_errors_keep_the_stable_wire_codes() {
        // O codigo no wire e o contrato da SPA: `media_not_found`, `note_not_found`,
        // `invalid_note`, `invalid_position` e `too_many_notes`.
        let cases = [
            (
                CoreError::MediaNotFound {
                    media_id: "m1".to_owned(),
                },
                IpcErrorCode::MediaNotFound,
                "media_not_found",
            ),
            (
                CoreError::StudyNoteNotFound,
                IpcErrorCode::NoteNotFound,
                "note_not_found",
            ),
            (
                CoreError::InvalidStudyNote,
                IpcErrorCode::InvalidNote,
                "invalid_note",
            ),
            (
                CoreError::InvalidStudyPosition {
                    position_seconds: -1,
                },
                IpcErrorCode::InvalidPosition,
                "invalid_position",
            ),
            (
                CoreError::TooManyNotes,
                IpcErrorCode::TooManyNotes,
                "too_many_notes",
            ),
        ];
        for (error, expected, wire) in cases {
            let mapped = note_error(error);
            assert_eq!(mapped.code, expected);
            assert_eq!(
                serde_json::to_value(mapped.code).unwrap(),
                serde_json::json!(wire)
            );
        }
    }

    #[test]
    fn an_unexpected_core_error_stays_generic() {
        // Um erro do nucleo que nao e de nota vira a falha generica: a mensagem do
        // dominio nao atravessa o IPC.
        let mapped = note_error(CoreError::InvalidOwnerId);
        assert_eq!(mapped.code, IpcErrorCode::StudyUnavailable);
    }
}
