//! Local registry and safe persistence of the desktop's courses.
//!
//! Each course is an external folder chosen by the user through a native picker triggered
//! by Rust. The webview never sends nor receives a path: the only data that crosses the
//! IPC are the course's opaque identity and the configuration boolean.
//!
//! This layer resolves a chosen path to its canonical form, refuses folders that are not
//! readable directories (including symbolic links, junctions and reparse points at
//! the top) and writes the canonicalized list to a versioned configuration file of the
//! application itself.
//!
//! Security rules preserved here:
//!
//! - the courses are external and **read-only**: nothing is created, moved or modified
//!   inside them, not even when a choice is refused;
//! - the configuration lives outside the course folders and outside the SQLite database;
//! - the write is serialized: the temporary file is created exclusively (`create_new`, without
//!   following a link or reusing an existing file) and the swap uses `rename`; on Windows,
//!   since the standard APIs do not replace an existing destination, it is removed immediately
//!   before the swap, leaving an unavoidable window in which the configuration may not exist;
//! - reading revalidates each root at startup: a configuration that is missing, corrupted, of an
//!   unknown version or with a removed/inaccessible folder simply does not produce that course;
//! - no error message carries a file-system path.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use learnvault_core::{CourseRoot, LEGACY_LIBRARY_ID, course_id_from_path, course_title};
use serde::{Deserialize, Serialize};

/// Serializes the configuration writes across the whole process.
///
/// Without the lock, two concurrent writes would compete for the same destination and could
/// interleave temporary files; with it, the configuration on disk is always the result of a
/// complete write, in arrival order. The lock belongs to this function, never to SQLite.
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// Maximum number of temporary names tried before giving up on the write.
const MAX_TEMP_ATTEMPTS: u32 = 32;

/// Version of the courses' configuration format.
pub const CONFIG_VERSION: u32 = 2;

/// Name of the configuration file inside the app's configuration directory.
pub const CONFIG_FILE_NAME: &str = "library.json";

/// Course registered on the desktop: stable identity and an already-validated root.
///
/// The path is private on purpose: it exists only for Rust and is never serialized
/// to the webview. `Debug` also hides the path, so that a diagnostic log
/// does not leak the course's location.
#[derive(Clone, PartialEq, Eq)]
pub struct CourseEntry {
    /// Stable identity used as the `library_id` in the database.
    pub id: String,
    /// Validated canonical root of the course.
    pub root: CourseRoot,
}

impl CourseEntry {
    /// Creates the entry, deriving the identity from the canonical path.
    pub fn from_root(root: CourseRoot) -> Self {
        Self {
            id: course_id_from_path(root.as_path()),
            root,
        }
    }

    /// Displayed title, derived from the course folder's name.
    pub fn title(&self) -> String {
        course_title(self.root.as_path())
    }
}

impl std::fmt::Debug for CourseEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CourseEntry")
            .field("id", &self.id)
            .field("root", &self.root)
            .finish()
    }
}

/// Stable failure of the configuration persistence. It never carries a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryError {
    /// The configuration could not be written safely.
    WriteFailed,
}

impl std::fmt::Display for LibraryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            LibraryError::WriteFailed => "não foi possível gravar a configuração do aplicativo",
        })
    }
}

impl std::error::Error for LibraryError {}

/// Serialized format of the configuration, versioned to allow a future migration.
#[derive(Debug, Serialize, Deserialize)]
struct StoredConfig {
    version: u32,
    courses: Vec<StoredCourse>,
}

/// Persisted entry of a course. The path is the only durable information.
#[derive(Debug, Serialize, Deserialize)]
struct StoredCourse {
    id: String,
    path: PathBuf,
}

/// Earlier format (version 1): a single library root.
///
/// The old root becomes a single course with the historical identity
/// ([`LEGACY_LIBRARY_ID`]), so that the media, scans and study progress already
/// stored stay associated with the same `library_id`.
#[derive(Debug, Deserialize)]
struct LegacyConfig {
    version: u32,
    library_root: PathBuf,
}

/// Loads and revalidates the persisted courses.
///
/// It returns an empty list when the configuration is missing, corrupted or of an unknown
/// version, and drops each course whose folder no longer passes validation (removed,
/// inaccessible or turned into a link). Reading never fails fatally nor creates
/// anything in the file system.
pub fn load_courses(config_dir: &Path) -> Vec<CourseEntry> {
    let Ok(raw) = fs::read(config_dir.join(CONFIG_FILE_NAME)) else {
        return Vec::new();
    };

    let stored: Vec<StoredCourse> = if let Ok(config) = serde_json::from_slice::<StoredConfig>(&raw)
    {
        if config.version != CONFIG_VERSION {
            return Vec::new();
        }
        config.courses
    } else if let Ok(legacy) = serde_json::from_slice::<LegacyConfig>(&raw) {
        if legacy.version != 1 {
            return Vec::new();
        }
        let Some(root) = CourseRoot::resolve(&legacy.library_root).ok() else {
            return Vec::new();
        };
        return vec![CourseEntry {
            id: LEGACY_LIBRARY_ID.to_owned(),
            root,
        }];
    } else {
        return Vec::new();
    };

    let mut courses: Vec<CourseEntry> = Vec::new();
    for entry in stored {
        let Ok(root) = CourseRoot::resolve(&entry.path) else {
            continue;
        };
        if courses.iter().any(|course| course.id == entry.id) {
            continue;
        }
        courses.push(CourseEntry { id: entry.id, root });
    }
    courses
}

/// Writes the canonicalized list to the configuration directory.
///
/// If `config_dir` is inside some course — a wrong configuration — the write is
/// refused **before** any creation, so that nothing is created inside the course
/// folders. The configuration directory is created outside the roots, and the file is written to
/// a temporary file and then renamed over the destination. On Windows, the destination is removed
/// immediately before the swap, leaving a small non-atomic window.
pub fn store_courses(config_dir: &Path, courses: &[CourseEntry]) -> Result<(), LibraryError> {
    if config_dir_is_inside(config_dir, courses) {
        return Err(LibraryError::WriteFailed);
    }

    fs::create_dir_all(config_dir).map_err(|_| LibraryError::WriteFailed)?;

    let payload = serde_json::to_vec(&StoredConfig {
        version: CONFIG_VERSION,
        courses: courses
            .iter()
            .map(|course| StoredCourse {
                id: course.id.clone(),
                path: course.root.as_path().to_path_buf(),
            })
            .collect(),
    })
    .map_err(|_| LibraryError::WriteFailed)?;

    write_atomically(&config_dir.join(CONFIG_FILE_NAME), &payload)
}

/// Tells whether the configuration directory is inside some registered course.
///
/// It compares canonical forms of the nearest existing ancestor, so it works even
/// when the configuration directory does not exist yet. The comparison is against the already
/// canonicalized roots, avoiding false negatives from a prefix difference (for example the
/// Windows verbatim prefix).
fn config_dir_is_inside(config_dir: &Path, courses: &[CourseEntry]) -> bool {
    let mut probe = config_dir;
    let resolved = loop {
        if probe.exists() {
            break fs::canonicalize(probe).ok();
        }
        match probe.parent() {
            Some(parent) => probe = parent,
            None => break None,
        }
    };
    courses.iter().any(|course| {
        resolved.as_ref().map_or_else(
            || config_dir.starts_with(course.root.as_path()),
            |resolved| resolved.starts_with(course.root.as_path()),
        )
    })
}

/// Writes `bytes` to `dest` in a serialized way.
///
/// The temporary file is created with `create_new` and then swapped by `rename`. The lock
/// [`STORE_LOCK`] guarantees that concurrent writes do not interleave. On Windows, the
/// destination is removed immediately before `rename`, leaving a window without the file.
fn write_atomically(dest: &Path, bytes: &[u8]) -> Result<(), LibraryError> {
    let directory = dest.parent().ok_or(LibraryError::WriteFailed)?;
    let _guard = STORE_LOCK.lock().map_err(|_| LibraryError::WriteFailed)?;

    let temporary = create_temporary(directory, bytes)?;
    if replace_file(&temporary, dest).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(LibraryError::WriteFailed);
    }

    Ok(())
}

/// Creates a **new** temporary file with the content, without following a link or reusing an existing
/// file.
///
/// Each attempt opens with `create_new`, which refuses any path that already exists — a regular
/// file, a symlink or a junction. This way a pre-existing temporary file is never
/// overwritten nor used as the target of the write; the write simply tries the next
/// name. The content is synced before the swap.
fn create_temporary(directory: &Path, bytes: &[u8]) -> Result<PathBuf, LibraryError> {
    for attempt in 0..MAX_TEMP_ATTEMPTS {
        let candidate = temporary_candidate(directory, attempt);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                if file
                    .write_all(bytes)
                    .and_then(|()| file.sync_all())
                    .is_err()
                {
                    let _ = fs::remove_file(&candidate);
                    return Err(LibraryError::WriteFailed);
                }
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(LibraryError::WriteFailed),
        }
    }
    Err(LibraryError::WriteFailed)
}

/// Candidate name of a temporary file inside the configuration directory.
///
/// The first keeps the historical format `.{CONFIG_FILE_NAME}.{pid}.tmp`; the following
/// attempts add a counter, so as not to collide with a temporary file left by an earlier
/// run or by another write.
fn temporary_candidate(directory: &Path, attempt: u32) -> PathBuf {
    if attempt == 0 {
        directory.join(format!(".{CONFIG_FILE_NAME}.{}.tmp", std::process::id()))
    } else {
        directory.join(format!(
            ".{CONFIG_FILE_NAME}.{}.{}.tmp",
            std::process::id(),
            attempt
        ))
    }
}

/// Replaces the destination using only standard APIs.
///
/// Outside Windows, `fs::rename` swaps the name on the same file system and replaces the
/// destination. On Windows, the standard API does not replace an existing destination: removing it
/// immediately before `rename` leaves a non-atomic window in which the destination may not
/// exist.
fn replace_file(source: &Path, dest: &Path) -> Result<(), LibraryError> {
    #[cfg(windows)]
    {
        match fs::remove_file(dest) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(LibraryError::WriteFailed),
        }
        if fs::rename(source, dest).is_err() {
            return Err(LibraryError::WriteFailed);
        }
        Ok(())
    }

    #[cfg(not(windows))]
    fs::rename(source, dest).map_err(|_| LibraryError::WriteFailed)
}
