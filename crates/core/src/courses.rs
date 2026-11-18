//! Registered courses: validated root, stable identity and derived title.
//!
//! The physical root of each course is platform configuration, not domain data. This
//! module gathers what the server and the desktop need to share: the safe validation
//! of the chosen folder, the stable identity used as the `library_id` in the database and the title
//! shown on the cards.
//!
//! A course's folder is external and **read-only**: resolving a root never creates,
//! moves or changes anything inside it.

use std::fmt;
use std::path::{Path, PathBuf};

/// Root of a course, already validated and canonicalized.
///
/// The path is private on purpose: it exists only for Rust and is never serialized
/// to the webview. `Debug` also hides the path, so that a diagnostic log
/// does not leak the course's location.
#[derive(Clone, PartialEq, Eq)]
pub struct CourseRoot(PathBuf);

impl CourseRoot {
    /// Validates and canonicalizes a candidate path for a course root.
    ///
    /// The path must exist, be a directory, not be a symbolic link, junction
    /// or reparse point at the top itself, and be readable (`read_dir`). The result is the
    /// canonical form — the only form persisted. No file is created or modified.
    pub fn resolve(path: &Path) -> Result<Self, CourseRootError> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(CourseRootError::Missing);
            }
            Err(_) => return Err(CourseRootError::NotReadable),
        };

        if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
            return Err(CourseRootError::Link);
        }
        if !metadata.is_dir() {
            return Err(CourseRootError::NotADirectory);
        }

        let canonical = std::fs::canonicalize(path).map_err(|_| CourseRootError::NotReadable)?;
        std::fs::read_dir(&canonical).map_err(|_| CourseRootError::NotReadable)?;

        Ok(Self(canonical))
    }

    /// Canonical path of the root, for internal Rust use only.
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Debug for CourseRoot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CourseRoot(<oculto>)")
    }
}

/// Stable failure of validating a course root. It never carries a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourseRootError {
    /// The chosen path does not exist.
    Missing,
    /// The chosen path is not a directory.
    NotADirectory,
    /// The chosen path could not be read (canonicalization or listing).
    NotReadable,
    /// The chosen path is a link, junction or reparse point.
    Link,
}

impl fmt::Display for CourseRootError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            CourseRootError::Missing => "a pasta do curso não existe",
            CourseRootError::NotADirectory => "a pasta do curso não é um diretório",
            CourseRootError::NotReadable => "a pasta do curso não pode ser lida",
            CourseRootError::Link => "a pasta do curso não pode ser um link",
        })
    }
}

impl std::error::Error for CourseRootError {}

/// Legacy identifier of the first library.
///
/// The root configured by the old deployment (and the root persisted by the desktop before
/// individual courses) stays associated with this identity, so that the media, scans
/// and study progress already stored are not lost when the course is reloaded.
pub const LEGACY_LIBRARY_ID: &str = "library-1";

/// Stable identity of a course derived from the canonical path.
///
/// The identity must be stable across runs so that re-importing the same folder
/// finds again the media and the study progress already persisted, and it must be opaque so
/// that the file-system path never crosses the webview boundary.
pub fn course_id_from_path(path: &Path) -> String {
    let digest = uuid::Uuid::new_v5(
        &uuid::Uuid::NAMESPACE_URL,
        path.as_os_str().as_encoded_bytes(),
    );
    format!("course-{digest}")
}

/// Displayed title of a course: the folder name, without ordering prefixes.
///
/// A folder with no usable name (for example the root of a volume) gets a neutral
/// title, so the card never ends up empty.
pub fn course_title(root: &Path) -> String {
    let fallback = "Curso";
    let Some(name) = root.file_name().and_then(|name| name.to_str()) else {
        return fallback.to_owned();
    };
    let cleaned = crate::catalog::clean_title(name);
    if !cleaned.is_empty() {
        return crate::titles::presentable(&cleaned);
    }
    // A folder named "2024" or "01" loses everything in the cleanup; the number is the only title
    // that tells apart the courses of a library organized by year, so it is preserved.
    let trimmed = name
        .trim()
        .trim_end_matches(|character: char| "._- •\t".contains(character))
        .trim();
    if trimmed.is_empty() {
        fallback.to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// Name of a course's immediate parent folder on disk, without exposing full paths.
pub fn course_source_folder(root: &Path) -> Option<String> {
    root.parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(str::to_owned)
}

/// True when the metadata marks a reparse point (junction or symlink) on Windows.
#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Outside Windows, junctions and reparse points do not exist; links are already detected.
#[cfg(not(windows))]
fn is_reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{CourseRoot, CourseRootError, course_id_from_path, course_title};
    use tempfile::tempdir;

    #[test]
    fn keeps_a_numeric_folder_name_as_the_course_title() {
        // A library organized by year uses folders such as "2024"; the cleanup removes
        // the whole name, and without preserving the number all the courses would end up the same.
        assert_eq!(course_title(Path::new("/cursos/ENEM/2024")), "2024");
        assert_eq!(course_title(Path::new("/cursos/ENEM/01")), "01");
        assert_eq!(
            course_title(Path::new("/cursos/ENEM/01 - Fisica")),
            "Fisica"
        );
    }

    #[test]
    fn resolve_rejects_missing_files_and_directories_pass() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("arquivo.txt");
        std::fs::write(&file, "x").unwrap();

        assert_eq!(
            CourseRoot::resolve(&temp.path().join("nao-existe")),
            Err(CourseRootError::Missing)
        );
        assert_eq!(
            CourseRoot::resolve(&file),
            Err(CourseRootError::NotADirectory)
        );

        let root = CourseRoot::resolve(temp.path()).unwrap();
        assert_eq!(
            root.as_path(),
            temp.path().canonicalize().unwrap().as_path()
        );
    }

    #[test]
    fn id_is_stable_and_opaque() {
        let temp = tempdir().unwrap();
        let other = tempdir().unwrap();
        let first = course_id_from_path(temp.path());
        let second = course_id_from_path(temp.path());

        assert_eq!(first, second);
        assert_ne!(first, course_id_from_path(other.path()));
        assert!(first.starts_with("course-"));
        assert!(!first.contains(temp.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn title_is_the_cleaned_folder_name() {
        let temp = tempdir().unwrap();
        let course = temp.path().join("01 - Física");
        std::fs::create_dir(&course).unwrap();

        assert_eq!(course_title(&course), "Física");
    }

    #[test]
    fn title_reads_a_release_style_folder_name_as_words() {
        assert_eq!(
            course_title(Path::new("/cursos/Quimica_Para_Disciplinas")),
            "Quimica Para Disciplinas"
        );
        assert_eq!(
            course_title(Path::new("/cursos/Go.Golang.Do.zero")),
            "Go Golang Do zero"
        );
    }

    #[test]
    fn title_falls_back_when_the_folder_has_no_usable_name() {
        assert_eq!(course_title(Path::new("/")), "Curso");
        // A purely numeric name is a valid title: only the absence of a name falls to the neutral one.
        assert_eq!(course_title(Path::new("42")), "42");
    }
}
