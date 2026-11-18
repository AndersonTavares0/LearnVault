//! Path configuration of the desktop.
//!
//! The configuration state exposed to the webview is only a boolean: the desktop knows whether
//! a library root was chosen, but never delivers the path. The root itself is
//! selected and persisted by [`crate::library`], outside the library and outside the database.
//!
//! The only path resolved here is the local database's, which lives in the application's own
//! data directory — never in the library, which stays external and read-only.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Name of the local database file inside the data directory.
pub const DATABASE_FILE_NAME: &str = "learnvault.sqlite";

/// Configuration state of the desktop exposed to the webview.
///
/// The shape is deliberately minimal: the webview learns only whether a library is
/// configured, never where it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DesktopConfigDto {
    /// True only when there is a configured, valid library root.
    pub configured: bool,
}

/// Path of the local database inside the data directory.
pub fn database_path(data_dir: &Path) -> PathBuf {
    data_dir.join(DATABASE_FILE_NAME)
}
