//! Domínio do LearnVault.
//!
//! Este crate concentra as regras de domínio compartilhadas pelo servidor (Axum) e pelo
//! aplicativo desktop (Tauri). Ele não conhece HTTP, Tauri, async ou formatos de resposta:
//! expõe operações e erros tipados e recebe, como parâmetros explícitos, a raiz da
//! biblioteca já resolvida pela integração de plataforma, o `library_id` e o `owner_id`.
//!
//! Estado atual: workspace e persistência SQLite com cadeia única de migrações (1.2),
//! scanner da biblioteca e identidade de mídia (1.3), política de rescan (1.4), catálogo
//! derivado (1.5), resolução segura de mídia (1.6), progresso de estudo (1.7) e remoção
//! explícita de mídia (1.8).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod auth;
pub mod catalog;
pub mod courses;
pub mod db;
pub mod error;
pub mod ids;
pub mod legacy_import;
pub mod media;
pub mod ordering;
pub mod paths;
pub mod pomodoro;
pub mod removal;
pub mod rescan;
pub mod scanner;
pub mod study;

mod migrations;

pub use auth::{
    AuthSession, AuthUser, bootstrap_admin, bootstrap_admin_with_change_required, create_session,
    delete_session, find_user_by_username, has_admin, session_user, update_credentials,
    update_credentials_and_session,
};
pub use catalog::{
    Availability, Catalog, CatalogFile, CatalogNode, NodeRole, build_catalog, course_file_lessons,
    natural_cmp,
};
pub use courses::{
    CourseRoot, CourseRootError, LEGACY_LIBRARY_ID, course_id_from_path, course_title,
};
pub use db::Database;
pub use error::{Error, Result};
pub use legacy_import::{LegacyImportOptions, LegacyImportReport, import_go_database};
pub use media::{
    MediaReference, OpenMedia, find_media_by_path, find_media_library_id, open_media,
    open_media_file, open_media_reference, validate_media_reference,
};
pub use ordering::{CatalogOrder, apply_order, order_siblings, sibling_paths, validate_siblings};
pub use paths::KeyMode;
pub use pomodoro::{
    DailyMission, FocusDay, FocusKind, FocusSession, FocusSessionInput, FocusSummary,
    RewardSummary, claim_daily_mission, claim_daily_mission_today, focus_summary, focus_summary_on,
    record_focus_session, reward_summary, reward_summary_on, set_weekly_goal, weekly_goal,
};
pub use removal::{MediaRemovalImpact, MediaRemovalOutcome, media_removal_impact, remove_media};
pub use rescan::{RescanSummary, apply_scan, rescan};
pub use scanner::{MediaKind, ScanOutcome, ScannedFile, scan};
pub use study::{
    Flashcard, MAX_RECENT_STUDY, ReviewRating, StudyDay, StudyNote, StudyProgress, StudySummary,
    create_flashcard, create_study_note, delete_flashcard, delete_study_note, list_flashcards,
    list_study_notes, recent_study, record_position, review_flashcard, set_completed,
    study_progress, study_summary, study_summary_on, update_flashcard, update_study_note,
};
