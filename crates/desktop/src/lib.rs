//! LearnVault desktop application (Tauri 2).
//!
//! This slice exposes the IPC boundary for courses, catalog, rescan and study progress: the
//! webview imports (`desktop_import_course`) and removes (`desktop_remove_course`) courses,
//! reads the aggregated catalog (`desktop_catalog`), triggers the explicit indexing
//! (`desktop_rescan`) and syncs study through `desktop_study_progress`,
//! `desktop_record_position`, `desktop_set_completed` and `desktop_recent_study`. The catalog,
//! the rescan summary and the progress are derived by the `learnvault-core` core over a local
//! SQLite database, in the application's data directory.
//!
//! The owner of the study progress is a fixed local identifier chosen by Rust: no
//! study command accepts `owner_id`, so the webview only reads and writes the local owner's
//! progress and never another owner's.
//!
//! Beyond the IPC, the desktop delivers images/thumbnails, small materials, video, audio and
//! subtitles through a custom Tauri 2 protocol (`learnvault-media://localhost/...`, see
//! [`media`]). Delivery is by opaque `media_id`, justified only by the catalog, with a
//! content-type allowlist, support for HTTP ranges and an explicit limit per response.
//!
//! Courses' custom covers use a second protocol (`learnvault-cover://...`,
//! see [`covers`]): the image the user chooses in a native picker is validated by the
//! initial bytes, written atomically to the application's data directory and delivered by
//! the course's opaque identity, with no path crossing the webview.
//!
//! Each course is chosen by a native picker triggered by Rust (`tauri-plugin-dialog`,
//! internal and not granted to the webview) and persisted atomically and versioned in the
//! application's configuration directory — never inside the course folders nor in
//! SQLite. The webview never sends nor receives a path: only opaque identities and the boolean
//! `configured`.
//!
//! The rescan is always explicit: neither startup nor import triggers it, and
//! concurrent calls are refused with a stable error while a scan is in
//! progress. There is no worker, progress or cancellation in this slice.
//!
//! There is no network listener, HTTP server, shell, filesystem, http or updater plugin,
//! nor authentication in this slice. The desktop player plays local media through the custom
//! protocol and syncs position/completion over IPC. The course folders stay
//! external and read-only: the rescan and the media delivery only read them and no
//! course path is written.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod backup;
pub mod command;
pub mod config;
pub mod courses;
pub mod covers;
pub mod dto;
pub mod error;
pub mod media;
pub mod ordering;
pub mod search;
pub mod state;
pub mod study;
pub mod sync;

pub use command::{
    catalog, catalog_with_orders, desktop_catalog, desktop_check_update,
    desktop_claim_daily_mission, desktop_create_bookmark, desktop_create_flashcard,
    desktop_create_note, desktop_create_pdf_annotation, desktop_delete_bookmark,
    desktop_delete_flashcard, desktop_delete_note, desktop_delete_pdf_annotation,
    desktop_download_update, desktop_due_flashcard_count, desktop_focus_summary,
    desktop_import_course, desktop_import_course_cover, desktop_list_all_flashcards,
    desktop_list_bookmarks, desktop_list_favorites, desktop_list_flashcard_reviews,
    desktop_list_flashcards, desktop_list_focus_sessions, desktop_list_notes,
    desktop_list_pdf_annotations, desktop_media_capabilities, desktop_media_removal_impact,
    desktop_open_in_system_player, desktop_playback_preferences, desktop_recent_study,
    desktop_record_focus_session, desktop_record_position, desktop_remove_course,
    desktop_remove_course_cover, desktop_remove_media, desktop_rescan, desktop_reset_course_order,
    desktop_restart_to_update, desktop_review_flashcard, desktop_reward_summary,
    desktop_save_playback_preferences, desktop_search, desktop_set_completed,
    desktop_set_course_favorite, desktop_set_course_order, desktop_set_lesson_favorite,
    desktop_set_weekly_goal, desktop_study_progress, desktop_study_summary, desktop_sync_configure,
    desktop_sync_login, desktop_sync_logout, desktop_sync_now, desktop_sync_state,
    desktop_update_flashcard, desktop_update_note, desktop_update_pdf_annotation,
    desktop_weekly_goal, open_in_system_player,
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
    CatalogDto, CatalogFileDto, CatalogNodeDto, CoverDto, CoverRemovalDto, DailyMissionDto,
    DesktopMediaCapabilitiesDto, DesktopUpdateInfoDto, FlashcardDto, FocusDayDto, FocusSessionDto,
    FocusSummaryDto, MediaBookmarkDto, MediaRemovalImpactDto, MediaRemovalOutcomeDto,
    NewPdfAnnotationDto, PdfAnnotationDto, PdfRectDto, PlaybackPreferencesDto, RecentStudyDto,
    RescanSummaryDto, RewardSummaryDto, SearchCourseDto, SearchLessonDto, SearchNoteDto,
    SearchResultsDto, StudyFavoritesDto, StudyNoteDto, StudyProgressDto, StudySummaryDto,
    SyncResultDto, SyncStateDto,
};
pub use error::{IpcError, IpcErrorCode};
pub use ordering::{CourseOrders, OrderError, order_path};
pub use search::DEFAULT_SEARCH_LIMIT;
pub use state::{DesktopState, RescanGuard};
pub use study::{DEFAULT_RECENT_LIMIT, LOCAL_OWNER_ID};

use tauri::Manager;

/// Initializes the Tauri webview pointing at the SPA embedded in `frontend-dist`.
///
/// In `setup`, the state opens the local database in the application's data directory, loads and
/// revalidates the courses from the configuration directory, and registers only the commands for
/// courses, catalog, rescan and study. The dialog plugin is registered for Rust's internal use
/// and is not granted to the webview by the ACL. The ACL's deny-by-default guarantees that nothing
/// outside the permissions declared in `capabilities/` is executable by the webview.
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let config_dir = app.path().app_config_dir()?;
            let state = DesktopState::open(&data_dir, &config_dir).map_err(|_| {
                std::io::Error::other("learnvault desktop: banco de dados local indisponível")
            })?;
            app.manage(state.clone());
            // A sync at start covers whatever was left in the queue from the previous run.
            state.request_sync();
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
            command::desktop_study_summary,
            command::desktop_list_notes,
            command::desktop_create_note,
            command::desktop_update_note,
            command::desktop_delete_note,
            command::desktop_list_bookmarks,
            command::desktop_create_bookmark,
            command::desktop_delete_bookmark,
            command::desktop_list_pdf_annotations,
            command::desktop_create_pdf_annotation,
            command::desktop_update_pdf_annotation,
            command::desktop_delete_pdf_annotation,
            command::desktop_list_flashcards,
            command::desktop_list_all_flashcards,
            command::desktop_list_flashcard_reviews,
            command::desktop_due_flashcard_count,
            command::desktop_create_flashcard,
            command::desktop_update_flashcard,
            command::desktop_delete_flashcard,
            command::desktop_review_flashcard,
            command::desktop_list_favorites,
            command::desktop_set_course_favorite,
            command::desktop_set_lesson_favorite,
            command::desktop_record_focus_session,
            command::desktop_list_focus_sessions,
            command::desktop_focus_summary,
            command::desktop_reward_summary,
            command::desktop_claim_daily_mission,
            command::desktop_weekly_goal,
            command::desktop_set_weekly_goal,
            command::desktop_media_removal_impact,
            command::desktop_remove_media,
            command::desktop_import_course_cover,
            command::desktop_remove_course_cover,
            command::desktop_set_course_order,
            command::desktop_reset_course_order,
            command::desktop_search,
            command::desktop_check_update,
            command::desktop_download_update,
            command::desktop_restart_to_update,
            command::desktop_media_capabilities,
            command::desktop_open_in_system_player,
            command::desktop_playback_preferences,
            command::desktop_save_playback_preferences,
            command::desktop_sync_state,
            command::desktop_sync_configure,
            command::desktop_sync_login,
            command::desktop_sync_logout,
            command::desktop_sync_now
        ])
        .run(tauri::generate_context!())
        .expect("falha ao executar o aplicativo desktop do LearnVault");
}
