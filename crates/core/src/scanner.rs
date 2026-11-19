//! Scan of the physical library.
//!
//! The scanner walks the root already resolved by the platform integration and returns a
//! flat, ordered list of files, with the location key of each. The
//! hierarchy of courses, sections and lessons is derived from that list by the catalog; the scanner
//! does not assemble it.
//!
//! The scan is strictly read-only: no file or directory of the library is
//! created, changed or removed. Symbolic links are not followed — neither the root, nor
//! inner directories or files — so the scan reaches nothing outside the root. The
//! root is resolved and validated by the platform integration; if it is still a link, the
//! scan does not follow it (the root is trusted only when it is a real directory).
//!
//! Unreadable directories or files are recorded in [`ScanOutcome::inaccessible`] and the
//! scan continues, without interrupting the whole library because of one item. If the
//! root itself cannot be read, the scan fails with a typed error instead of presenting
//! itself as an empty library.

use std::path::Path;
use std::time::UNIX_EPOCH;

use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::paths::{KeyMode, relative_path_key};

/// Extensions recognized as playable media.
const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mov", "webm", "mkv", "m3u8", "ts", "ogv", "avi", "wmv", "flv", "mp3", "wav",
    "ogg", "oga", "m4a", "flac", "aac", "opus", "weba", "wma",
];

/// Extensions recognized as subtitles.
const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "vtt", "ass", "ssa", "sub"];

/// Minimum size for a `.ts` file to be treated as media.
///
/// The `.ts` extension is also used by TypeScript source code; small files are
/// material and not video.
const MIN_TS_MEDIA_SIZE: u64 = 1024 * 1024;

/// Directory names ignored during the scan.
const IGNORED_DIRECTORY_NAMES: &[&str] = &["temp", "0____"];

/// Directory prefix ignored during the scan.
const IGNORED_DIRECTORY_PREFIX: &str = "temp_";

/// Classification of a file found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// Playable file (video or stream).
    Media,
    /// Subtitle file.
    Subtitle,
    /// Any other supporting file.
    Material,
}

/// File found during the scan.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Path relative to the root, with `/` separators and the original case.
    pub relative_path: String,
    /// Comparison key derived from the path, normalized according to the system.
    pub relative_path_key: String,
    /// Classification of the file.
    pub kind: MediaKind,
    /// Size in bytes.
    pub size: u64,
    /// Modification instant in seconds since the Unix epoch.
    ///
    /// It is `0` when the file system does not report the instant or when the value is
    /// before the Unix epoch. `0` means "unknown", never "very old".
    pub mtime: i64,
}

/// Result of a scan.
#[derive(Debug, Clone)]
pub struct ScanOutcome {
    /// Files found, ordered by relative path.
    pub files: Vec<ScannedFile>,
    /// Relative paths that could not be read nor catalogued.
    ///
    /// It includes unreadable directories and files and names the domain refuses. It never
    /// contains an absolute path; the root itself is represented by `.`.
    pub inaccessible: Vec<String>,
}

/// Walks the library and returns the files found.
///
/// The root must exist, be a directory and be readable; otherwise it returns
/// [`Error::LibraryRootMissing`], [`Error::LibraryRootNotDirectory`] or
/// [`Error::LibraryRootUnreadable`].
pub fn scan(root: &Path) -> Result<ScanOutcome> {
    if !root.exists() {
        return Err(Error::LibraryRootMissing {
            path: root.to_path_buf(),
        });
    }
    if !root.is_dir() {
        return Err(Error::LibraryRootNotDirectory {
            path: root.to_path_buf(),
        });
    }
    // Confirms the root can be read before walking it: without this, an unreadable root
    // would return an empty scan and be indistinguishable from a library with no
    // files — and the rescan policy would interpret the access failure as absence.
    std::fs::read_dir(root).map_err(|source| Error::LibraryRootUnreadable {
        path: root.to_path_buf(),
        source,
    })?;

    let mode = KeyMode::for_host();
    let mut files = Vec::new();
    let mut inaccessible = Vec::new();

    let mut walker = WalkDir::new(root)
        .follow_links(false)
        // The root is not followed when it is a link: without this, `follow_links(false)` would still
        // follow a symlink/junction at the top and the scan would reach another tree.
        .follow_root_links(false)
        .min_depth(1)
        .into_iter();

    while let Some(entry) = walker.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                inaccessible.push(relative_to_root(root, error.path()));
                continue;
            }
        };

        let file_type = entry.file_type();

        // A symbolic link is ignored entirely: it is neither followed nor reported.
        if file_type.is_symlink() {
            continue;
        }

        // On Windows a junction or other reparse point may not report as a symlink: refuse it
        // explicitly, without descending into it.
        #[cfg(windows)]
        {
            let linked = entry
                .metadata()
                .is_ok_and(|metadata| crate::media::is_link_or_reparse_point(&metadata));
            if linked {
                if file_type.is_dir() {
                    walker.skip_current_dir();
                }
                continue;
            }
        }

        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || is_ignored_directory(&name) {
                walker.skip_current_dir();
            }
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }

        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => {
                inaccessible.push(relative_to_root(root, Some(entry.path())));
                continue;
            }
        };

        let relative_path = relative_to_root(root, Some(entry.path()));
        let Ok(key) = relative_path_key(&relative_path, mode) else {
            // A name the domain refuses cannot bring down the whole library: the
            // item is recorded as not catalogued and the scan continues.
            inaccessible.push(relative_path);
            continue;
        };

        files.push(ScannedFile {
            relative_path,
            relative_path_key: key,
            kind: classify(&name, metadata.len()),
            size: metadata.len(),
            mtime: mtime_seconds(&metadata),
        });
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    Ok(ScanOutcome {
        files,
        inaccessible,
    })
}

/// Classifies a file from its name and its size in bytes.
///
/// It is the single source of truth for classification: the scanner uses it during the scan and the
/// catalog reuses it over the persisted data, so the same file never appears with
/// different types depending on the layer that presents it.
pub fn classify(name: &str, size: u64) -> MediaKind {
    match extension(name).as_deref() {
        Some(extension) if SUBTITLE_EXTENSIONS.contains(&extension) => MediaKind::Subtitle,
        Some("ts") if size < MIN_TS_MEDIA_SIZE => MediaKind::Material,
        Some(extension) if MEDIA_EXTENSIONS.contains(&extension) && is_showcase_video(name) => {
            MediaKind::Material
        }
        Some(extension) if MEDIA_EXTENSIONS.contains(&extension) => MediaKind::Media,
        _ => MediaKind::Material,
    }
}

/// Names that are a course showcase and not a lesson: the whole name, without extension, must be one of
/// these terms. A title that merely contains the word (like "Trailer tips") is still a lesson.
const SHOWCASE_NAMES: &[&str] = &[
    "trailer",
    "preview",
    "promo",
    "sample",
    "teaser",
    "advertisement",
    "course trailer",
    "course preview",
    "intro promo",
];

fn is_showcase_video(name: &str) -> bool {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let normalized = stem
        .chars()
        .map(|character| {
            if character == '_' || character == '-' {
                ' '
            } else {
                character.to_ascii_lowercase()
            }
        })
        .collect::<String>();
    let normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");
    SHOWCASE_NAMES.contains(&normalized.as_str())
}

fn extension(name: &str) -> Option<String> {
    let (_, extension) = name.rsplit_once('.')?;
    if extension.is_empty() {
        return None;
    }
    Some(extension.to_ascii_lowercase())
}

/// Tells whether a directory is ignored by the scan (temporary ones).
pub fn is_ignored_directory(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    IGNORED_DIRECTORY_NAMES.contains(&name.as_str()) || name.starts_with(IGNORED_DIRECTORY_PREFIX)
}

pub(crate) fn mtime_seconds(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

/// Converts a path from the scan into a path relative to the root.
///
/// It never returns an absolute path: when the path cannot be converted, or when
/// it is the root itself, it returns `.`.
fn relative_to_root(root: &Path, path: Option<&Path>) -> String {
    let Some(path) = path else {
        return ".".to_owned();
    };
    let Ok(relative) = path.strip_prefix(root) else {
        return ".".to_owned();
    };

    let relative = relative.to_string_lossy().replace('\\', "/");
    if relative.is_empty() {
        ".".to_owned()
    } else {
        relative
    }
}

#[cfg(test)]
mod showcase_tests {
    use super::{MediaKind, classify};

    const LARGE: u64 = 10_000_000;

    #[test]
    fn exact_showcase_names_are_material_not_lessons() {
        for name in [
            "Trailer.mp4",
            "course_trailer.mkv",
            "Course Preview.mp4",
            "sample.webm",
            "Intro-Promo.mp4",
        ] {
            assert_eq!(classify(name, LARGE), MediaKind::Material, "{name}");
        }
    }

    #[test]
    fn lessons_that_only_contain_the_word_stay_media() {
        for name in [
            "01 Trailer tips.mp4",
            "Sample code.mp4",
            "Preview of the API.mp4",
            "Lesson 3.mp4",
        ] {
            assert_eq!(classify(name, LARGE), MediaKind::Media, "{name}");
        }
    }
}
