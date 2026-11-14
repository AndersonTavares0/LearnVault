//! Course roots of a remote source (ADR 0004): browse the source one folder level at a time,
//! choose which folders are courses, and apply every scan split by those roots.
//!
//! All endpoints are administrator-only and read-only on the remote side: browsing only
//! issues one directory listing per request, and a root is a mapping in the database.

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{
    CourseSource, Database, Error, KeyMode, RescanSummary, RootError, ScanOutcome, SourceError,
    SourceRoot, apply_source_scan, demote_root,
    ids::new_library_id,
    list_roots, natural_cmp, partition_scan, promote_root, relative_path_key,
    scanner::{MediaKind, classify, is_ignored_directory},
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    auth::same_origin,
    catalog::{
        csrf_failed, internal_error, private_no_store, service_unavailable, status_private_no_store,
    },
    sources::{
        PROBE_TIMEOUT, RemoteClient, RemoteSource, admin_failure_response, json_error,
        require_admin, source_error_code,
    },
};

/// Longest relative path accepted from a client.
const MAX_PATH_CHARS: usize = 1024;
/// Folders listed by one browse request; the rest is reported as truncated.
const MAX_BROWSE_FOLDERS: usize = 2000;
/// Folders turned into roots by one "every folder inside" request.
const MAX_ROOTS_PER_REQUEST: usize = 500;

/// Normalizes a client path to a clean relative path that keeps its case, or `None`.
/// The empty string is the source root.
fn clean_path(path: &str) -> Option<String> {
    if path.chars().count() > MAX_PATH_CHARS {
        return None;
    }
    if path.is_empty() {
        return Some(String::new());
    }
    relative_path_key(path, KeyMode::Sensitive).ok()
}

fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn is_under(key: &str, area: &str) -> bool {
    key == area
        || (key.len() > area.len() && key.starts_with(area) && key.as_bytes()[area.len()] == b'/')
}

/// Finds the source, checking the administrator session (and the origin for changes).
/// `remote` also requires readable credentials, for endpoints that talk to the server.
async fn authorized_source(
    state: &AppState,
    headers: &HeaderMap,
    source_id: &str,
    changes: bool,
    remote: bool,
) -> Result<RemoteSource, Box<Response>> {
    if let Err(failure) = require_admin(state, headers).await {
        return Err(Box::new(admin_failure_response(failure)));
    }
    if changes && !same_origin(headers) {
        return Err(Box::new(csrf_failed()));
    }
    let Some(source) = state
        .sources_snapshot()
        .into_iter()
        .find(|source| source.id == source_id)
    else {
        return Err(Box::new(status_private_no_store(StatusCode::NOT_FOUND)));
    };
    if remote && source.credential_error {
        return Err(Box::new(json_error(
            StatusCode::BAD_REQUEST,
            "credentials_unavailable",
        )));
    }
    Ok(source)
}

/// Lists one folder of the source: its subfolders and how many files it holds directly.
async fn list_folder(
    source: &RemoteSource,
    path: &str,
) -> Result<(Vec<String>, usize, usize), Box<Response>> {
    let client = RemoteClient::new(source)
        .map_err(|_| Box::new(json_error(StatusCode::BAD_REQUEST, "invalid_source")))?;
    let entries = match tokio::time::timeout(PROBE_TIMEOUT, client.list(path)).await {
        Ok(Ok(entries)) => entries,
        Ok(Err(SourceError::NotFound)) => {
            return Err(Box::new(json_error(StatusCode::NOT_FOUND, "not_found")));
        }
        Ok(Err(error)) => {
            return Err(Box::new(json_error(
                StatusCode::BAD_GATEWAY,
                source_error_code(error),
            )));
        }
        Err(_) => {
            return Err(Box::new(json_error(StatusCode::GATEWAY_TIMEOUT, "timeout")));
        }
    };
    let (mut folders, mut media, mut other) = (Vec::new(), 0, 0);
    for entry in entries {
        let name = last_segment(&entry.path);
        if entry.is_dir {
            if !is_ignored_directory(name) {
                folders.push(entry.path);
            }
        } else if classify(name, entry.size) == MediaKind::Media {
            media += 1;
        } else {
            other += 1;
        }
    }
    folders.sort_by(|left, right| natural_cmp(last_segment(left), last_segment(right)));
    Ok((folders, media, other))
}

#[derive(Deserialize)]
pub(crate) struct BrowseQuery {
    #[serde(default)]
    path: String,
}

#[derive(Serialize)]
struct FolderResponse {
    name: String,
    path: String,
    /// `root` (already a course), `inside_root`, `contains_root` or `free`.
    state: &'static str,
}

#[derive(Serialize)]
struct BrowseResponse {
    path: String,
    folders: Vec<FolderResponse>,
    media_files: usize,
    other_files: usize,
    truncated: bool,
}

fn folder_state(path: &str, root_keys: &[String]) -> &'static str {
    let Ok(key) = relative_path_key(path, KeyMode::for_host()) else {
        return "free";
    };
    for root in root_keys {
        if key == *root {
            return "root";
        }
        if is_under(&key, root) {
            return "inside_root";
        }
        if is_under(root, &key) {
            return "contains_root";
        }
    }
    "free"
}

fn root_keys(roots: &[SourceRoot]) -> Vec<String> {
    roots
        .iter()
        .filter_map(|root| relative_path_key(&root.prefix, KeyMode::for_host()).ok())
        .collect()
}

async fn load_roots(state: &AppState, source_id: String) -> Result<Vec<SourceRoot>, Box<Response>> {
    if !state.has_blocking_capacity() {
        return Err(Box::new(service_unavailable()));
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return Err(Box::new(service_unavailable()));
    };
    match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        list_roots(&database, &source_id)
    })
    .await
    {
        Ok(Ok(roots)) => Ok(roots),
        _ => Err(Box::new(internal_error("source_roots_unavailable"))),
    }
}

/// `GET /api/admin/sources/{id}/browse?path=` — one folder level of the source.
pub(crate) async fn browse(
    State(state): State<AppState>,
    AxumPath(source_id): AxumPath<String>,
    Query(query): Query<BrowseQuery>,
    headers: HeaderMap,
) -> Response {
    let source = match authorized_source(&state, &headers, &source_id, false, true).await {
        Ok(source) => source,
        Err(response) => return *response,
    };
    let Some(path) = clean_path(&query.path) else {
        return json_error(StatusCode::BAD_REQUEST, "invalid_path");
    };
    let roots = match load_roots(&state, source.id.clone()).await {
        Ok(roots) => roots,
        Err(response) => return *response,
    };
    let (folders, media_files, other_files) = match list_folder(&source, &path).await {
        Ok(listing) => listing,
        Err(response) => return *response,
    };
    let keys = root_keys(&roots);
    let truncated = folders.len() > MAX_BROWSE_FOLDERS;
    let folders = folders
        .into_iter()
        .take(MAX_BROWSE_FOLDERS)
        .map(|folder| FolderResponse {
            name: last_segment(&folder).to_owned(),
            state: folder_state(&folder, &keys),
            path: folder,
        })
        .collect();
    private_no_store(
        Json(BrowseResponse {
            path,
            folders,
            media_files,
            other_files,
            truncated,
        })
        .into_response(),
    )
}

#[derive(Serialize)]
struct RootResponse {
    id: String,
    path: String,
    name: String,
}

impl From<&SourceRoot> for RootResponse {
    fn from(root: &SourceRoot) -> Self {
        Self {
            id: root.library_id.clone(),
            name: last_segment(&root.prefix).to_owned(),
            path: root.prefix.clone(),
        }
    }
}

#[derive(Serialize)]
struct RootsResponse {
    roots: Vec<RootResponse>,
}

/// `GET /api/admin/sources/{id}/roots` — the folders chosen as courses.
pub(crate) async fn list(
    State(state): State<AppState>,
    AxumPath(source_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let source = match authorized_source(&state, &headers, &source_id, false, false).await {
        Ok(source) => source,
        Err(response) => return *response,
    };
    match load_roots(&state, source.id).await {
        Ok(roots) => private_no_store(
            Json(RootsResponse {
                roots: roots.iter().map(RootResponse::from).collect(),
            })
            .into_response(),
        ),
        Err(response) => *response,
    }
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum Pick {
    /// The folder itself is a course.
    Course,
    /// Every folder directly inside is a course.
    Children,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateRequest {
    path: String,
    mode: Pick,
}

#[derive(Serialize)]
struct Skipped {
    path: String,
    reason: &'static str,
}

#[derive(Serialize)]
struct CreatedResponse {
    created: Vec<RootResponse>,
    skipped: Vec<Skipped>,
}

fn root_reason(error: &RootError) -> &'static str {
    match error {
        RootError::Overlap { .. } => "overlaps_existing_root",
        RootError::InvalidPrefix(_) => "invalid_path",
        _ => "invalid_root",
    }
}

/// `POST /api/admin/sources/{id}/roots` — chooses a folder (or every folder inside it) as a
/// course. The folder must exist on the source; existing media moves to the new course and
/// the next scan fills in the rest.
pub(crate) async fn create(
    State(state): State<AppState>,
    AxumPath(source_id): AxumPath<String>,
    headers: HeaderMap,
    Json(request): Json<CreateRequest>,
) -> Response {
    let source = match authorized_source(&state, &headers, &source_id, true, true).await {
        Ok(source) => source,
        Err(response) => return *response,
    };
    let Some(path) = clean_path(&request.path) else {
        return json_error(StatusCode::BAD_REQUEST, "invalid_path");
    };
    if path.is_empty() && matches!(request.mode, Pick::Course) {
        return json_error(StatusCode::BAD_REQUEST, "invalid_path");
    }
    // The folder is listed first: it proves it exists and is readable, and gives the children.
    let (folders, ..) = match list_folder(&source, &path).await {
        Ok(listing) => listing,
        Err(response) => return *response,
    };
    let targets = match request.mode {
        Pick::Course => vec![path],
        Pick::Children => folders,
    };
    if targets.len() > MAX_ROOTS_PER_REQUEST {
        return json_error(StatusCode::BAD_REQUEST, "too_many_folders");
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let single = matches!(request.mode, Pick::Course);
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let mut database = database;
        let (mut created, mut skipped) = (Vec::new(), Vec::new());
        for target in targets {
            let library_id = new_library_id();
            match promote_root(
                &mut database,
                &source.id,
                &library_id,
                &target,
                KeyMode::for_host(),
            ) {
                Ok(_) => created.push(RootResponse {
                    id: library_id,
                    name: last_segment(&target).to_owned(),
                    path: target,
                }),
                Err(Error::SourceRoot(error)) => skipped.push(Skipped {
                    path: target,
                    reason: root_reason(&error),
                }),
                Err(error) => return Err(error),
            }
        }
        Ok((created, skipped))
    })
    .await;
    match outcome {
        // A single chosen folder that cannot become a root is a conflict, not a partial success.
        Ok(Ok((created, skipped))) if single && created.is_empty() => {
            let reason = skipped.first().map_or("invalid_root", |skip| skip.reason);
            json_error(StatusCode::CONFLICT, reason)
        }
        Ok(Ok((created, skipped))) => private_no_store(
            (
                StatusCode::CREATED,
                Json(CreatedResponse { created, skipped }),
            )
                .into_response(),
        ),
        _ => internal_error("source_roots_unavailable"),
    }
}

#[derive(Serialize)]
struct RemovedResponse {
    removed: bool,
}

/// `DELETE /api/admin/sources/{id}/roots/{root_id}` — the folder stops being a course of its
/// own and its media returns to the source course. Nothing on the remote server is touched.
pub(crate) async fn remove(
    State(state): State<AppState>,
    AxumPath((source_id, root_id)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let source = match authorized_source(&state, &headers, &source_id, true, false).await {
        Ok(source) => source,
        Err(response) => return *response,
    };
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let mut database = database;
        // Only a root of this very source can be removed through it.
        if !list_roots(&database, &source.id)?
            .iter()
            .any(|root| root.library_id == root_id)
        {
            return Ok(false);
        }
        demote_root(&mut database, &root_id).map(|_| true)
    })
    .await;
    match outcome {
        Ok(Ok(true)) => private_no_store(Json(RemovedResponse { removed: true }).into_response()),
        Ok(Ok(false)) => status_private_no_store(StatusCode::NOT_FOUND),
        _ => internal_error("source_roots_unavailable"),
    }
}

/// Applies a finished scan of a source: split by its roots, one library each, plus the files
/// outside every root for the source itself. A source without roots is applied as a whole.
/// A failed scan is recorded for every library so none of them looks verified.
pub(crate) fn apply_scan_by_roots(
    database: &mut Database,
    source_id: &str,
    outcome: Result<ScanOutcome, SourceError>,
) -> learnvault_core::Result<RescanSummary> {
    let roots = list_roots(database, source_id)?;
    if roots.is_empty() {
        return apply_source_scan(database, source_id, outcome);
    }
    let scan = match outcome {
        Ok(scan) => scan,
        Err(error) => {
            for root in &roots {
                let _ = apply_source_scan(database, &root.library_id, Err(error));
            }
            return apply_source_scan(database, source_id, Err(error));
        }
    };
    let part = partition_scan(scan, &roots, KeyMode::for_host())?;
    let mut total = RescanSummary {
        complete: true,
        ..RescanSummary::default()
    };
    let libraries = part
        .roots
        .into_iter()
        .chain(std::iter::once((source_id.to_owned(), part.remainder)));
    for (library_id, scan) in libraries {
        let summary = apply_source_scan(database, &library_id, Ok(scan))?;
        total.scan_id = total.scan_id.max(summary.scan_id);
        total.complete &= summary.complete;
        total.inserted += summary.inserted;
        total.updated += summary.updated;
        total.changed += summary.changed;
        total.moved += summary.moved;
        total.marked_missing += summary.marked_missing;
        total.preserved += summary.preserved;
        total.ambiguous.extend(summary.ambiguous);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::{clean_path, folder_state};

    #[test]
    fn client_paths_are_clean_relative_paths_or_refused() {
        assert_eq!(clean_path("").as_deref(), Some(""));
        assert_eq!(
            clean_path("Cursos/Pentest").as_deref(),
            Some("Cursos/Pentest")
        );
        assert_eq!(
            clean_path("Cursos\\Pentest").as_deref(),
            Some("Cursos/Pentest")
        );
        for bad in ["..", "a/../b", "/abs", "a//b", "a/", ".", "C:/x", "a\0b"] {
            assert_eq!(clean_path(bad), None, "{bad:?}");
        }
        assert_eq!(clean_path(&"a/".repeat(600)), None);
    }

    #[test]
    fn a_folder_is_classified_against_the_existing_roots() {
        let roots = vec!["Cursos/Rust".to_owned()];
        assert_eq!(folder_state("Cursos/Rust", &roots), "root");
        assert_eq!(folder_state("Cursos/Rust/01 Intro", &roots), "inside_root");
        assert_eq!(folder_state("Cursos", &roots), "contains_root");
        assert_eq!(folder_state("Cursos/RustX", &roots), "free");
        assert_eq!(folder_state("Outros", &roots), "free");
    }
}
