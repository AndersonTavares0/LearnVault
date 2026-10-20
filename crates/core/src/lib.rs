//! Domínio do LearnVault.
//!
//! Este crate concentra as regras de domínio compartilhadas pelo servidor (Axum) e pelo
//! aplicativo desktop (Tauri). Ele não conhece HTTP, Tauri, async ou formatos de resposta:
//! expõe operações e erros tipados e recebe, como parâmetros explícitos, a raiz da
//! biblioteca já resolvida pela integração de plataforma, o `library_id` e o `owner_id`.
//!
//! Estado atual: workspace e persistência SQLite com cadeia única de migrações (1.2),
//! scanner da biblioteca e identidade de mídia (1.3), política de rescan (1.4), catálogo
//! derivado (1.5), resolução segura de mídia (1.6), progresso de estudo (1.7), remoção
//! explícita de mídia (1.8) e notas pessoais (1.9).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod auth;
pub mod catalog;
pub mod catalog_order;
pub mod courses;
pub mod db;
pub mod discovery;
pub mod error;
pub mod ids;
pub mod interpretation;
pub mod legacy_import;
pub mod media;
pub mod notes;
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
pub use catalog_order::{
    apply_catalog_order, apply_saved_catalog_order, catalog_order, persist_catalog_order,
    reset_catalog_order,
};
pub use courses::{
    CourseRoot, CourseRootError, LEGACY_LIBRARY_ID, course_id_from_path, course_title,
};
pub use db::Database;
pub use discovery::{
    DiscoveryEntryKind, DiscoveryFileKind, DiscoveryNode, DiscoveryReport, DiscoveryStatus,
    DiscoverySummary, discover,
};
pub use error::{Error, Result};
pub use interpretation::{
    InterpretationPreview, OrganizationSuggestion, SuggestionConfidence, interpret,
};
pub use legacy_import::{LegacyImportOptions, LegacyImportReport, import_go_database};
pub use media::{
    MediaReference, OpenMedia, find_media_by_path, find_media_library_id, open_media,
    open_media_reference,
};
pub use notes::{
    MAX_NOTE_TEXT_CHARS, MAX_NOTES_PER_MEDIA, MAX_NOTES_PER_OWNER, StudyNote, create_note,
    delete_note, list_all_notes, list_notes, update_note,
};
pub use ordering::{CatalogOrder, apply_order, order_siblings, sibling_paths, validate_siblings};
pub use paths::KeyMode;
pub use pomodoro::{
    DEFAULT_POMODORO_SECONDS, PomodoroState, PomodoroStatus, pause_pomodoro, pomodoro_state,
    reset_pomodoro, start_pomodoro,
};
pub use removal::{MediaRemovalImpact, MediaRemovalOutcome, media_removal_impact, remove_media};
pub use rescan::{RescanSummary, apply_scan, rescan};
pub use scanner::{MediaKind, ScanOutcome, ScannedFile, scan};
pub use study::{
    MAX_RECENT_STUDY, StudyAnalytics, StudyProgress, recent_study, record_position, set_completed,
    study_analytics, study_progress,
};
