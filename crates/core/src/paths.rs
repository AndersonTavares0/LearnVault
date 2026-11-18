//! Normalization and resolution of relative paths of the library.
//!
//! The domain never treats a path string as identity: the identity is the
//! `media_id`. The relative path is only the file's current location inside the
//! library, and the comparison key (`relative_path_key`) is derived from it.
//!
//! Resolution is **purely lexical**: these functions do not touch the file system.
//! Lexical confinement is necessary but not sufficient: checking the real target
//! of the file — symbolic links and junctions — belongs to the media-resolution delivery,
//! which checks the open object.
//!
//! In Windows mode a component containing `:` is refused, because `:` opens an alternate
//! data stream (`lesson.mp4:stream`) instead of the intended file. On Unix systems
//! `:` is an ordinary character in file names and stays valid.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

/// Path comparison mode.
///
/// The mode is an explicit parameter, not a query to the operating system, so that
/// both behaviours can be verified in the same test environment.
///
/// In this project [`KeyMode::Insensitive`] corresponds to Windows mode: it is what
/// [`KeyMode::for_host`] returns there and it is the mode in which the Windows-specific path rules
/// apply — in particular the refusal of components with `:`, which would open an alternate
/// data stream. The insensitive mode must not be confused with a case-insensitive mount
/// on another system, which still does not have those rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    /// Case-sensitive.
    Sensitive,
    /// Case-insensitive.
    Insensitive,
}

impl KeyMode {
    /// Comparison mode of the current operating system.
    ///
    /// Windows does not distinguish case in paths and, among the project's declared
    /// targets, it is the only system where that is the rule. The other targets
    /// preserve case on the default file system; a mount that does not preserve it
    /// (for example a share or a mounted volume) is still treated
    /// as sensitive, and this is recorded as a known limitation.
    pub fn for_host() -> Self {
        if cfg!(windows) {
            KeyMode::Insensitive
        } else {
            KeyMode::Sensitive
        }
    }
}

/// Normalizes a relative path and produces the comparison key.
///
/// The path must be relative and cannot contain `.` or `..` components, empty
/// components, null bytes or components with a drive prefix (`C:`). Separators `\` and
/// `/` are converted to `/`. In [`KeyMode::Insensitive`] mode only ASCII letters
/// are folded to lowercase, the same behaviour as `COLLATE NOCASE` in SQLite.
///
/// Separators are normalized to `/` on any system, so the key of the same
/// file is the same when the library is used on different platforms.
/// This means that a file name containing `\` on Unix produces the same key
/// as an equivalent nested path; the collision is detected by the database's
/// uniqueness constraint, and the case is recorded as a known limitation.
pub fn relative_path_key(path: &str, mode: KeyMode) -> Result<String> {
    // The insensitive mode is Windows mode (see [`KeyMode::for_host`]), and it is in it that the
    // alternate-data-stream rule must apply.
    let normalized = normalize_relative(path, mode == KeyMode::Insensitive)?;

    Ok(match mode {
        KeyMode::Sensitive => normalized,
        KeyMode::Insensitive => normalized.to_ascii_lowercase(),
    })
}

/// Tells whether a path key is in the indicated area — the area itself or something inside
/// it.
///
/// The comparison respects the component boundary: `A` covers `A` and `A/x`, but not `AB`.
/// It is used by the rescan policy and by the catalog when handling unverified areas.
pub(crate) fn is_under_area(key: &str, area: &str) -> bool {
    key == area
        || (key.len() > area.len() && key.starts_with(area) && key.as_bytes()[area.len()] == b'/')
}

/// Resolves a relative path inside the library root.
///
/// The function confines the result by lexical analysis and refuses absolute paths, those with
/// `.` or `..` components, empty, malformed ones or with a drive prefix. No access
/// to the file system is done here.
pub fn resolve_within(root: &Path, relative: &str) -> Result<PathBuf> {
    let normalized = normalize_relative(relative, host_is_windows())?;

    let mut resolved = root.to_path_buf();
    for component in normalized.split('/') {
        resolved.push(component);
    }

    // Defence in depth: on Windows, `PathBuf::push` discards the accumulated path
    // when the component has its own prefix (for example `C:`). The per-component
    // validation already refuses that case; this check keeps the contract true even
    // if the validation changes.
    if !resolved.starts_with(root) {
        return Err(Error::InvalidRelativePath {
            path: relative.to_owned(),
        });
    }

    Ok(resolved)
}

/// Tells whether the current platform is Windows, where `:` opens an alternate data stream.
fn host_is_windows() -> bool {
    cfg!(windows)
}

fn normalize_relative(path: &str, windows: bool) -> Result<String> {
    if path.is_empty() || is_absolute_like(path) {
        return Err(Error::InvalidRelativePath {
            path: path.to_owned(),
        });
    }

    let mut components = Vec::new();
    for component in path.split(['/', '\\']) {
        // The validation is per component, not only at the start of the string: a component
        // such as `C:` in the middle of the path also discards the root when it is pushed.
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains('\0')
            || is_absolute_like(component)
            || (windows && component.contains(':'))
        {
            return Err(Error::InvalidRelativePath {
                path: path.to_owned(),
            });
        }
        components.push(component);
    }

    Ok(components.join("/"))
}

fn is_absolute_like(path: &str) -> bool {
    path.starts_with('/') || path.starts_with('\\') || has_drive_prefix(path)
}

fn has_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}
