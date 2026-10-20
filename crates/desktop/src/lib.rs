//! Aplicativo desktop do LearnVault (Tauri 2).
//!
//! Esta fatia expõe a fronteira IPC de cursos, catálogo, rescan e progresso de estudo: o
//! webview importa (`desktop_import_course`) e remove (`desktop_remove_course`) cursos,
//! lê o catálogo agregado (`desktop_catalog`), aciona a indexação explícita
//! (`desktop_rescan`) e sincroniza o estudo por `desktop_study_progress`,
//! `desktop_record_position`, `desktop_set_completed` e `desktop_recent_study`. O catálogo,
//! o resumo do rescan e o progresso são derivados pelo núcleo `learnvault-core` sobre um
//! banco SQLite local, no diretório de dados do aplicativo.
//!
//! O dono do progresso de estudo é um identificador local fixo escolhido pelo Rust: nenhum
//! comando de estudo aceita `owner_id`, então o webview só lê e grava o progresso do dono
//! local e nunca o de outro dono.
//!
//! Além do IPC, o desktop entrega imagens/thumbnails, materiais pequenos, vídeo, áudio e
//! legendas por um protocolo customizado do Tauri 2 (`learnvault-media://localhost/...`, ver
//! [`media`]). A entrega é por `media_id` opaco, justificada apenas no catálogo, com allowlist
//! de tipo de conteúdo, suporte a faixas HTTP e limite explícito por resposta.
//!
//! As capas personalizadas dos cursos usam um segundo protocolo (`learnvault-cover://...`,
//! ver [`covers`]): a imagem escolhida pelo usuário em um seletor nativo é validada pelos
//! bytes iniciais, gravada atomicamente no diretório de dados do aplicativo e entregue por
//! identidade opaca de curso, sem que nenhum caminho cruze o webview.
//!
//! Cada curso é escolhido por um seletor nativo acionado pelo Rust (`tauri-plugin-dialog`,
//! interno e não concedido ao webview) e persistido de forma atômica e versionada no
//! diretório de configuração do aplicativo — nunca dentro das pastas de curso nem no
//! SQLite. O webview nunca envia nem recebe um caminho: só identidades opacas e o booleano
//! `configured`.
//!
//! O rescan é sempre explícito: nem a inicialização nem a importação o disparam, e
//! chamadas concorrentes são recusadas com um erro estável enquanto uma varredura está em
//! andamento. Não há worker, progresso nem cancelamento nesta fatia.
//!
//! Não há listener de rede, servidor HTTP, plugin de shell, filesystem, http ou updater,
//! nem autenticação nesta fatia. O player do desktop reproduz mídia local pelo protocolo
//! customizado e sincroniza posição/conclusão por IPC. As pastas de curso permanecem
//! externas e somente leitura: o rescan e a entrega de mídia apenas as leem e nenhum
//! caminho de curso é escrito.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod command;
pub mod config;
pub mod courses;
pub mod covers;
pub mod dto;
pub mod error;
pub mod media;
pub mod ordering;
pub mod state;
pub mod study;

pub use command::{
    catalog, catalog_with_orders, desktop_catalog, desktop_create_study_note,
    desktop_delete_study_note, desktop_import_course, desktop_import_course_cover,
    desktop_media_removal_impact, desktop_pause_pomodoro, desktop_pomodoro, desktop_recent_study,
    desktop_record_position, desktop_remove_course, desktop_remove_course_cover,
    desktop_remove_media, desktop_rescan, desktop_reset_course_order, desktop_reset_pomodoro,
    desktop_set_completed, desktop_set_course_order, desktop_start_pomodoro,
    desktop_study_analytics, desktop_study_notes, desktop_study_notes_all, desktop_study_progress,
    desktop_update_study_note,
};
pub use config::{DATABASE_FILE_NAME, DesktopConfigDto, database_path};
pub use courses::{
    CONFIG_FILE_NAME, CONFIG_VERSION, CourseEntry, LibraryError, load_courses, store_courses,
};
pub use covers::{
    COVER_HOST, COVER_SCHEME, COVERS_DIR_NAME, CoverError, CoverMethod, CoverRejection,
    CoverRequest, ImageType, MAX_COVER_BYTES, RawCoverRequest, detect_image_type,
    parse_cover_request, valid_course_id,
};
pub use dto::{
    CatalogDto, CatalogFileDto, CatalogNodeDto, CoverDto, CoverRemovalDto, MediaRemovalImpactDto,
    MediaRemovalOutcomeDto, PomodoroDto, RecentStudyDto, RescanSummaryDto, StudyAnalyticsDto,
    StudyNoteDto, StudyNotesDto, StudyProgressDto,
};
pub use error::{IpcError, IpcErrorCode};
pub use ordering::{CourseOrders, OrderError, order_path};
pub use state::{DesktopState, RescanGuard};
pub use study::{DEFAULT_RECENT_LIMIT, LOCAL_OWNER_ID};

use tauri::Manager;

/// Inicializa o webview Tauri apontando para a SPA embutida em `frontend-dist`.
///
/// No `setup`, o estado abre o banco local no diretório de dados do aplicativo, carrega e
/// revalida os cursos do diretório de configuração, e registra apenas os comandos de
/// cursos, catálogo, rescan e estudo. O plugin de diálogo é registrado para uso interno do
/// Rust e não é concedido ao webview pelo ACL. O deny-by-default do ACL garante que nada
/// fora das permissões declaradas em `capabilities/` seja executável pelo webview.
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let config_dir = app.path().app_config_dir()?;
            let state = DesktopState::open(&data_dir, &config_dir).map_err(|_| {
                std::io::Error::other("learnvault desktop: banco de dados local indisponível")
            })?;
            app.manage(state);
            Ok(())
        });
    covers::register(media::register(builder))
        .invoke_handler(tauri::generate_handler![
            command::desktop_catalog,
            command::desktop_import_course,
            command::desktop_remove_course,
            command::desktop_rescan,
            command::desktop_study_progress,
            command::desktop_record_position,
            command::desktop_set_completed,
            command::desktop_recent_study,
            command::desktop_study_analytics,
            command::desktop_pomodoro,
            command::desktop_start_pomodoro,
            command::desktop_pause_pomodoro,
            command::desktop_reset_pomodoro,
            command::desktop_media_removal_impact,
            command::desktop_remove_media,
            command::desktop_import_course_cover,
            command::desktop_remove_course_cover,
            command::desktop_set_course_order,
            command::desktop_reset_course_order,
            command::desktop_study_notes,
            command::desktop_study_notes_all,
            command::desktop_create_study_note,
            command::desktop_update_study_note,
            command::desktop_delete_study_note
        ])
        .run(tauri::generate_context!())
        .expect("falha ao executar o aplicativo desktop do LearnVault");
}
