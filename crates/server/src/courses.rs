//! Server course registry and the administrative endpoints that maintain it.
//!
//! Each course is an external folder registered individually and stays **read-only**:
//! registering does not copy, move or change anything inside it. The registry lives in a
//! versioned configuration file next to the database, never in the database nor inside the
//! courses — the physical root is platform configuration, not domain data.
//!
//! The identity of each course is derived from the canonical path ([`course_id_from_path`]),
//! so re-importing the same folder finds again the media and study progress already
//! persisted. The path never crosses the HTTP boundary: the interface knows only `id` and
//! `title`.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    Json,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{CourseRoot, LEGACY_LIBRARY_ID, course_id_from_path, course_title};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    auth::same_origin,
    catalog::{
        authenticated_user, credentials_required, csrf_failed, internal_error, private_no_store,
        service_unavailable, status_private_no_store,
    },
};

/// Name of the registry file inside the server's data directory.
pub const REGISTRY_FILE_NAME: &str = "courses.json";

/// Version of the registry file's format.
pub const REGISTRY_VERSION: u32 = 1;

/// Registered course: stable identity, displayed title and an already-validated root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredCourse {
    /// Stable identity used as the `library_id` in the database.
    pub id: String,
    /// Title displayed on the cards.
    pub title: String,
    /// Validated canonical root of the course. Never serialized to the client.
    pub root: CourseRoot,
}

/// Serialized format of the registry file.
#[derive(Debug, Serialize, Deserialize)]
struct RegistryFile {
    version: u32,
    courses: Vec<StoredCourse>,
}

/// Persisted entry of a course. The path is the only durable information.
#[derive(Debug, Serialize, Deserialize)]
struct StoredCourse {
    id: String,
    path: PathBuf,
}

/// Path of the registry file for a database. It sits next to the database, in the data directory;
/// a database with no parent directory uses the current directory.
pub(crate) fn registry_path(database_path: &Path) -> PathBuf {
    let parent = database_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join(REGISTRY_FILE_NAME)
}

/// Loads the persisted registry.
///
/// A missing, corrupted or unknown-version file results in an empty registry; each
/// entry is revalidated with [`CourseRoot::resolve`] and a root that was removed, became inaccessible or was
/// replaced by a link is silently dropped (the persisted list is not rewritten by
/// a read). No path appears in messages.
pub(crate) fn load_registry(path: &Path) -> Vec<RegisteredCourse> {
    let Ok(raw) = fs::read(path) else {
        return Vec::new();
    };
    let Ok(stored) = serde_json::from_slice::<RegistryFile>(&raw) else {
        return Vec::new();
    };
    if stored.version != REGISTRY_VERSION {
        return Vec::new();
    }
    let mut courses: Vec<RegisteredCourse> = Vec::new();
    for entry in stored.courses {
        let Ok(root) = CourseRoot::resolve(&entry.path) else {
            continue;
        };
        if courses.iter().any(|course| course.id == entry.id) {
            continue;
        }
        let title = course_title(root.as_path());
        courses.push(RegisteredCourse {
            id: entry.id,
            title,
            root,
        });
    }
    courses
}

/// Seed course of legacy mode: the root configured by `--library-root`/`LIBRARY_DIR`
/// uses the historical identity to preserve media, scans and progress already stored.
pub(crate) fn legacy_seed(root: &Path) -> Option<RegisteredCourse> {
    let root = CourseRoot::resolve(root).ok()?;
    Some(RegisteredCourse {
        id: LEGACY_LIBRARY_ID.to_owned(),
        title: course_title(root.as_path()),
        root,
    })
}

fn is_ignored_seed_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    name.starts_with('.')
        || name == "@eaDir"
        || name.eq_ignore_ascii_case("$RECYCLE.BIN")
        || name.eq_ignore_ascii_case("System Volume Information")
}

/// Courses seeded from the configured root on first start.
///
/// Each first-level folder under the root becomes a course: a library of courses
/// is usually a directory with one folder per course, and requiring a manual import per folder
/// left the library empty right after deploy. When there is no valid subfolder, the
/// root itself is seeded as before, preserving layouts that keep the media
/// directly in it and the historical identity `library-1`.
pub(crate) fn seeded_courses(root: &Path) -> Vec<RegisteredCourse> {
    const MAX_AUTO_NESTED: usize = 100;

    let Ok(entries) = fs::read_dir(root) else {
        return legacy_seed(root).into_iter().collect();
    };
    let mut folders: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .map(|entry| entry.path())
        .filter(|path| !is_ignored_seed_dir(path))
        .collect();
    // File-system order varies; sorting keeps the seeded list stable.
    folders.sort();
    let mut courses: Vec<RegisteredCourse> = Vec::new();
    for folder in folders {
        let Ok(folder_root) = CourseRoot::resolve(&folder) else {
            continue;
        };
        let nested = learnvault_core::scan(folder_root.as_path())
            .map(|outcome| learnvault_core::detect_course_folders(&outcome.files))
            .unwrap_or_default();
        // Only split a top-level folder when it contains multiple nested courses; a single
        // nested subfolder keeps the top-level directory as the course root.
        if nested.len() > 1 && nested.len() <= MAX_AUTO_NESTED {
            let before = courses.len();
            for rel in nested {
                push_seeded_course(&mut courses, &folder_root.as_path().join(rel));
            }
            if courses.len() > before {
                continue;
            }
        }
        push_seeded_course_root(&mut courses, folder_root);
    }
    if courses.is_empty() {
        return legacy_seed(root).into_iter().collect();
    }
    courses
}

fn push_seeded_course(courses: &mut Vec<RegisteredCourse>, candidate: &Path) {
    if let Ok(course_root) = CourseRoot::resolve(candidate) {
        push_seeded_course_root(courses, course_root);
    }
}

fn push_seeded_course_root(courses: &mut Vec<RegisteredCourse>, root: CourseRoot) {
    let course = RegisteredCourse {
        id: course_id_from_path(root.as_path()),
        title: course_title(root.as_path()),
        root,
    };
    if !courses.iter().any(|registered| registered.id == course.id) {
        courses.push(course);
    }
}

/// Writes the registry in a serialized, atomic way.
///
/// The temporary file is created exclusively (`create_new`) and swapped by `rename`; on Windows,
/// since the standard API does not replace an existing destination, the destination is removed immediately
/// before the swap. Serialization is the responsibility of the state's write lock.
pub(crate) fn store_registry(
    path: &Path,
    courses: &[RegisteredCourse],
) -> Result<(), RegistryError> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory).map_err(|_| RegistryError::WriteFailed)?;

    let stored = RegistryFile {
        version: REGISTRY_VERSION,
        courses: courses
            .iter()
            .map(|course| StoredCourse {
                id: course.id.clone(),
                path: course.root.as_path().to_path_buf(),
            })
            .collect(),
    };
    let payload = serde_json::to_vec(&stored).map_err(|_| RegistryError::WriteFailed)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("courses");
    let temporary = directory.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&payload)?;
        file.sync_all()
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(RegistryError::WriteFailed);
    }

    if replace_file(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(RegistryError::WriteFailed);
    }
    Ok(())
}

/// Swaps the temporary file into the destination using only standard APIs.
///
/// Shared with the cover storage ([`crate::covers`]) so that the atomic swap
/// of a file in the data directory has a single implementation, with the same care
/// on Windows (the standard API does not replace an existing destination).
pub(crate) fn replace_file(source: &Path, dest: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        match fs::remove_file(dest) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    fs::rename(source, dest)
}

/// Stable failure of the registry. It never carries a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistryError {
    /// The registry could not be written safely.
    WriteFailed,
}

/// Response for a registered course, without the path.
#[derive(Serialize)]
struct CourseResponse {
    id: String,
    title: String,
}

impl From<&RegisteredCourse> for CourseResponse {
    fn from(course: &RegisteredCourse) -> Self {
        Self {
            id: course.id.clone(),
            title: course.title.clone(),
        }
    }
}

/// Body of the import request: the path of the course folder on the server.
#[derive(Deserialize)]
pub(crate) struct ImportRequest {
    path: String,
}

#[derive(Serialize)]
struct ImportResponse {
    course: CourseResponse,
}

#[derive(Serialize)]
struct RemoveResponse {
    removed: bool,
}

/// `POST /api/admin/courses` — registers a folder as a course.
///
/// The folder must exist, be a readable directory and not be a link/junction. The
/// validation and the write run off the Tokio threads; the course starts to exist in the
/// catalog after the next indexing. A folder already registered is refused with `400`.
pub(crate) async fn import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ImportRequest>,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(crate::StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !user.is_admin {
        return status_private_no_store(StatusCode::FORBIDDEN);
    }
    if user.must_change_credentials {
        return credentials_required();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let courses = Arc::clone(&state.courses);
    let courses_file = state.courses_file.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        import_course(&courses, &courses_file, &request.path)
    })
    .await;

    match outcome {
        Ok(Ok(course)) => private_no_store(
            Json(ImportResponse {
                course: (&course).into(),
            })
            .into_response(),
        ),
        Ok(Err(ImportFailure::InvalidPath)) => {
            json_error(StatusCode::BAD_REQUEST, "invalid_course_path")
        }
        Ok(Err(ImportFailure::Duplicate)) => {
            json_error(StatusCode::BAD_REQUEST, "course_already_registered")
        }
        Ok(Err(ImportFailure::WriteFailed)) | Err(_) => {
            internal_error("course_registry_unavailable")
        }
    }
}

/// `DELETE /api/admin/courses/{id}` — removes a course from the registry.
///
/// It removes only the registration: the files stay intact and the course's catalog stops
/// being served. Removing an unknown course answers `200` with `removed: false`.
pub(crate) async fn remove(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(crate::StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !user.is_admin {
        return status_private_no_store(StatusCode::FORBIDDEN);
    }
    if user.must_change_credentials {
        return credentials_required();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let courses = Arc::clone(&state.courses);
    let courses_file = state.courses_file.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        remove_course(&courses, &courses_file, &course_id)
    })
    .await;

    match outcome {
        Ok(Ok(removed)) => private_no_store(Json(RemoveResponse { removed }).into_response()),
        Ok(Err(_)) | Err(_) => internal_error("course_registry_unavailable"),
    }
}

#[derive(Debug)]
enum ImportFailure {
    InvalidPath,
    Duplicate,
    WriteFailed,
}

/// Validates the folder, registers the course and persists the list under the write lock.
///
/// The in-memory list is only swapped after the write completes: a write failure does not
/// leave the state diverging from the file.
fn import_course(
    courses: &std::sync::RwLock<Vec<RegisteredCourse>>,
    courses_file: &Path,
    path: &str,
) -> Result<RegisteredCourse, ImportFailure> {
    let candidate = Path::new(path);
    let root = CourseRoot::resolve(candidate).map_err(|_| ImportFailure::InvalidPath)?;
    let course = RegisteredCourse {
        id: course_id_from_path(root.as_path()),
        title: course_title(root.as_path()),
        root,
    };

    let mut guard = courses.write().map_err(|_| ImportFailure::WriteFailed)?;
    if guard.iter().any(|registered| registered.id == course.id) {
        return Err(ImportFailure::Duplicate);
    }
    let mut next = guard.clone();
    next.push(course.clone());
    store_registry(courses_file, &next).map_err(|_| ImportFailure::WriteFailed)?;
    *guard = next;
    Ok(course)
}

/// Removes the course from the registry and persists the remaining list.
fn remove_course(
    courses: &std::sync::RwLock<Vec<RegisteredCourse>>,
    courses_file: &Path,
    course_id: &str,
) -> Result<bool, RegistryError> {
    let mut guard = courses.write().map_err(|_| RegistryError::WriteFailed)?;
    let next: Vec<RegisteredCourse> = guard
        .iter()
        .filter(|course| course.id != course_id)
        .cloned()
        .collect();
    let removed = next.len() != guard.len();
    if removed {
        store_registry(courses_file, &next)?;
        *guard = next;
    }
    Ok(removed)
}

/// Stable JSON error, with no additional cache policy.
fn json_error(status: StatusCode, message: &'static str) -> Response {
    private_no_store((status, Json(ErrorBody { error: message })).into_response())
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

#[cfg(test)]
mod tests {
    use std::sync::RwLock;

    use tempfile::tempdir;

    use super::{
        ImportFailure, RegisteredCourse, RegistryFile, import_course, load_registry, registry_path,
        remove_course, seeded_courses, store_registry,
    };

    fn write_registry_file(path: &std::path::Path, contents: &str) {
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn seeding_registers_each_top_level_folder_as_a_course() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("library");
        std::fs::create_dir(&library).unwrap();
        std::fs::create_dir(library.join("01 - Física")).unwrap();
        std::fs::create_dir(library.join("02 - Matemática")).unwrap();
        std::fs::write(library.join("leia-me.txt"), "solto").unwrap();

        let seeded = seeded_courses(&library);
        let titles: Vec<&str> = seeded.iter().map(|course| course.title.as_str()).collect();
        assert_eq!(titles, ["Física", "Matemática"]);
        // Names that the cleanup would zero out stay distinct: without this, a library organized by
        // year would register several courses named "Course".
        std::fs::create_dir(library.join("2024")).unwrap();
        std::fs::create_dir(library.join("2025")).unwrap();
        let years: Vec<String> = seeded_courses(&library)
            .into_iter()
            .map(|course| course.title)
            .filter(|title| title.chars().all(|character| character.is_ascii_digit()))
            .collect();
        assert_eq!(years, ["2024", "2025"]);
        assert!(
            seeded
                .iter()
                .all(|course| course.id != learnvault_core::LEGACY_LIBRARY_ID),
            "pastas de curso usam a identidade derivada do caminho"
        );
    }

    #[test]
    fn seeding_ignores_hidden_and_system_metadata_directories() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("library");
        std::fs::create_dir(&library).unwrap();
        std::fs::create_dir(library.join(".hidden-statfs")).unwrap();
        std::fs::create_dir(library.join(".Trash")).unwrap();
        std::fs::create_dir(library.join("@eaDir")).unwrap();
        std::fs::create_dir(library.join("$RECYCLE.BIN")).unwrap();
        std::fs::create_dir(library.join("System Volume Information")).unwrap();
        std::fs::create_dir(library.join("Valid Course")).unwrap();

        let seeded = seeded_courses(&library);
        let titles: Vec<&str> = seeded.iter().map(|course| course.title.as_str()).collect();
        assert_eq!(titles, ["Valid Course"]);
    }

    #[test]
    fn seeding_detects_nested_courses_under_category_folders() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("library");

        // Category folder with two nested courses under a subcategory
        for rel in [
            "01 - Tech/Security/Course Alpha/01 Intro/01.mp4",
            "01 - Tech/Security/Course Alpha/02 Next/02.mp4",
            "01 - Tech/Security/Course Beta/01 Start/01.mp4",
            "01 - Tech/Security/Course Beta/02 End/02.mp4",
            // Direct course folder under root
            "02 - Math/01 Algebra/01.mp4",
            "02 - Math/02 Calculus/02.mp4",
        ] {
            let p = library.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"video").unwrap();
        }

        let seeded = seeded_courses(&library);
        let titles: Vec<&str> = seeded.iter().map(|course| course.title.as_str()).collect();
        assert_eq!(titles, ["Course Alpha", "Course Beta", "Math"]);
    }

    #[test]
    fn seeding_preserves_top_level_course_with_single_nested_folder() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("library");

        for rel in [
            "Course/Rust from zero/01 Ownership/Lesson.mp4",
            "Course/Rust from zero/02 Borrowing/Lesson.mp4",
        ] {
            let p = library.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, b"video").unwrap();
        }

        let seeded = seeded_courses(&library);
        let titles: Vec<&str> = seeded.iter().map(|course| course.title.as_str()).collect();
        assert_eq!(titles, ["Course"]);
    }

    #[test]
    fn seeding_falls_back_to_the_library_root_when_there_is_no_subfolder() {
        let temp = tempdir().unwrap();
        let library = temp.path().join("library");
        std::fs::create_dir(&library).unwrap();
        std::fs::write(library.join("aula.mp4"), "mídia").unwrap();

        let seeded = seeded_courses(&library);
        assert_eq!(seeded.len(), 1);
        assert_eq!(seeded[0].id, learnvault_core::LEGACY_LIBRARY_ID);
        assert_eq!(seeded[0].root.as_path(), library.canonicalize().unwrap());
    }

    #[test]
    fn registry_round_trips_courses_and_drops_invalid_roots() {
        let temp = tempdir().unwrap();
        let data = temp.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let database = data.join("learnvault.sqlite");
        let registry = registry_path(&database);
        assert_eq!(registry, data.join("courses.json"));

        let course_a = temp.path().join("01 - Física");
        let course_b = temp.path().join("Matemática");
        std::fs::create_dir(&course_a).unwrap();
        std::fs::create_dir(&course_b).unwrap();

        let courses = RwLock::new(Vec::new());
        let first = import_course(&courses, &registry, course_a.to_str().unwrap()).unwrap();
        let second = import_course(&courses, &registry, course_b.to_str().unwrap()).unwrap();
        assert_eq!(first.title, "Física");
        assert_eq!(second.title, "Matemática");

        // The folder removed before the reload is silently dropped.
        std::fs::remove_dir(&course_b).unwrap();
        let loaded = load_registry(&registry);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, first.id);
    }

    #[test]
    fn import_rejects_duplicates_and_invalid_paths() {
        let temp = tempdir().unwrap();
        let registry = temp.path().join("courses.json");
        let course = temp.path().join("Curso");
        std::fs::create_dir(&course).unwrap();
        let courses = RwLock::new(Vec::new());

        let missing = import_course(&courses, &registry, "nao-existe");
        assert!(matches!(missing, Err(ImportFailure::InvalidPath)));

        import_course(&courses, &registry, course.to_str().unwrap()).unwrap();
        let duplicate = import_course(&courses, &registry, course.to_str().unwrap());
        assert!(matches!(duplicate, Err(ImportFailure::Duplicate)));
    }

    #[test]
    fn remove_is_idempotent_and_persists_the_remaining_courses() {
        let temp = tempdir().unwrap();
        let registry = temp.path().join("courses.json");
        let course = temp.path().join("Curso");
        std::fs::create_dir(&course).unwrap();
        let courses = RwLock::new(Vec::new());
        let registered = import_course(&courses, &registry, course.to_str().unwrap()).unwrap();

        assert!(remove_course(&courses, &registry, &registered.id).unwrap());
        assert!(!remove_course(&courses, &registry, &registered.id).unwrap());
        assert!(load_registry(&registry).is_empty());
        assert!(courses.read().unwrap().is_empty());
    }

    #[test]
    fn load_discards_unknown_versions_and_corrupt_files() {
        let temp = tempdir().unwrap();
        let registry = temp.path().join("courses.json");

        write_registry_file(&registry, "not json");
        assert!(load_registry(&registry).is_empty());

        let course = temp.path().join("Curso");
        std::fs::create_dir(&course).unwrap();
        let unknown = serde_json::json!({
            "version": 99,
            "courses": [{ "id": "course-1", "path": course }]
        });
        write_registry_file(&registry, &unknown.to_string());
        assert!(load_registry(&registry).is_empty());
    }

    #[test]
    fn store_writes_a_versioned_file_without_temporaries_left_behind() {
        let temp = tempdir().unwrap();
        let registry = temp.path().join("nested").join("courses.json");
        let course = temp.path().join("Curso");
        std::fs::create_dir(&course).unwrap();
        let root = learnvault_core::CourseRoot::resolve(&course).unwrap();
        let courses = vec![RegisteredCourse {
            id: "course-1".to_owned(),
            title: "Curso".to_owned(),
            root,
        }];

        store_registry(&registry, &courses).unwrap();
        let raw = std::fs::read_to_string(&registry).unwrap();
        let parsed: RegistryFile = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.courses.len(), 1);

        let leftovers: Vec<_> = std::fs::read_dir(registry.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }
}
