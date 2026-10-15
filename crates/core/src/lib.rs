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
pub mod db;
pub mod error;
pub mod ids;
pub mod media;
pub mod paths;
pub mod removal;
pub mod rescan;
pub mod scanner;
pub mod study;

mod migrations;

pub use auth::{
    AuthSession, AuthUser, bootstrap_admin, create_session, delete_session, find_user_by_username,
    has_admin, session_user,
};
pub use catalog::{Availability, Catalog, CatalogFile, CatalogNode, NodeRole, build_catalog};
pub use db::Database;
pub use error::{Error, Result};
pub use media::{OpenMedia, open_media};
pub use media::{MediaReference, find_media_by_path};
pub use paths::KeyMode;
pub use removal::{MediaRemovalImpact, MediaRemovalOutcome, media_removal_impact, remove_media};
pub use rescan::{RescanSummary, apply_scan, rescan};
pub use scanner::{MediaKind, ScanOutcome, ScannedFile, scan};
pub use study::{
    MAX_RECENT_STUDY, StudyProgress, recent_study, record_position, set_completed, study_progress,
};
