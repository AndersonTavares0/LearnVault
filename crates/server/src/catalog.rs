//! HTTP endpoints of the catalog, of the administrative rescan and of the course registry.

use std::{
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use learnvault_core::{
    Availability, CatalogFile, CatalogNode, CourseRoot, CourseRootError, MediaKind, NodeRole,
    RescanSummary, ScanOutcome,
};
use serde::Serialize;

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token, same_origin},
    courses::RegisteredCourse,
    ordering::CourseOrders,
    sources::RemoteSource,
};

/// Body limit of the ordering request, above the router's default limit (8 KiB).
///
/// Reordering a module with many lessons (or a course with many modules) produces a body with
/// one relative path per sibling; the generous limit avoids a surprise `413` on a large
/// library, without affecting the other routes.
pub(crate) const ORDER_BODY_LIMIT: usize = 512 * 1024;

enum RescanFailure {
    Busy,
    Capacity,
    Failed,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Serialize)]
struct CatalogResponse {
    complete: bool,
    loose_files: Vec<CatalogFileResponse>,
    /// Canonical recursive tree: the top-level courses and their subtrees.
    nodes: Vec<CatalogNodeResponse>,
}

#[derive(Serialize, Clone)]
struct CatalogNodeResponse {
    title: String,
    path: String,
    role: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    course_id: Option<String>,
    /// Modification seconds of the course's custom cover, when it exists. It is the
    /// version the SPA uses to invalidate the URL (`?v=`); absent when there is no cover.
    #[serde(skip_serializing_if = "Option::is_none")]
    cover_version: Option<i64>,
    /// Name of the course's immediate source folder on disk (parent directory), without exposing absolute paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    source_folder: Option<String>,
    /// Assessment of the course's organization. Present only on course nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    structure: Option<StructureResponse>,
    /// True on courses that live in a remote source (the source or a folder chosen from it).
    /// Absent on local courses; the SPA uses the field to avoid offering actions that only exist for
    /// local courses (cover, order, removal from the registry).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    remote: bool,
    files: Vec<CatalogFileResponse>,
    loose_files: Vec<CatalogFileResponse>,
    children: Vec<CatalogNodeResponse>,
}

/// Confidence level of the course's organization and the reasons that lowered it.
#[derive(Serialize, Clone)]
struct StructureResponse {
    confidence: &'static str,
    concerns: Vec<&'static str>,
}

fn structure_response(
    assessment: learnvault_core::structure::StructureAssessment,
) -> StructureResponse {
    use learnvault_core::structure::{Concern, Confidence};
    StructureResponse {
        confidence: match assessment.confidence {
            Confidence::High => "high",
            Confidence::Medium => "medium",
            Confidence::Low => "low",
        },
        concerns: assessment
            .concerns
            .into_iter()
            .map(|concern| match concern {
                Concern::NoLessons => "no_lessons",
                Concern::LooseMedia => "loose_media",
                Concern::LessonsWithoutNumbering => "lessons_without_numbering",
                Concern::LooseMaterials => "loose_materials",
            })
            .collect(),
    }
}

#[derive(Serialize, Clone)]
struct CatalogFileResponse {
    media_id: String,
    name: String,
    path: String,
    size: u64,
    mtime_seconds: i64,
    kind: &'static str,
    availability: &'static str,
    unverified: bool,
}

#[derive(Serialize)]
pub(crate) struct RescanResponse {
    pub(crate) scan_id: i64,
    pub(crate) complete: bool,
    pub(crate) courses: usize,
    pub(crate) failed_courses: usize,
    pub(crate) inserted: usize,
    pub(crate) updated: usize,
    pub(crate) marked_missing: usize,
    pub(crate) preserved: usize,
    /// Number of ambiguous relative keys refused in this scan.
    ///
    /// It is only the count: the keys themselves are relative paths and never cross the API.
    pub(crate) ambiguous_count: usize,
}

pub(crate) async fn catalog(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => user,
        Ok(Some(_)) => return credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };

    // Avoids holding the database mutex just to return 503 under saturation.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let courses = state.courses_snapshot();
    let sources = state.sources_snapshot();
    let orders = state.orders_snapshot();
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let covers = crate::covers::covers_dir(&state.config.database_path);
    match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let allowed = if user.is_admin {
            None
        } else {
            learnvault_core::list_user_course_access(&database, &user.user_id)?
        };
        let is_allowed = |id: &str| {
            allowed
                .as_ref()
                .is_none_or(|allowed| allowed.iter().any(|candidate| candidate == id))
        };
        let filtered_courses: Vec<RegisteredCourse> = courses
            .into_iter()
            .filter(|course| is_allowed(&course.id))
            .collect();
        build_course_catalog(
            &database,
            &filtered_courses,
            &sources,
            &is_allowed,
            &orders,
            &covers,
        )
    })
    .await
    {
        Ok(Ok(catalog)) => catalog_response(&headers, catalog),
        Ok(Err(_)) | Err(_) => internal_error("catalog unavailable"),
    }
}

/// Serializes the catalog only once and lets the client revalidate through the `ETag`.
///
/// The response is still always revalidated (`private, no-cache`), so a `304`
/// never serves stale data: it only avoids retransmitting the whole tree when nothing changed.
/// The hash covers the serialized body, which includes the ordering and the covers applied.
fn catalog_response(request: &HeaderMap, catalog: CatalogResponse) -> Response {
    let Ok(body) = serde_json::to_string(&catalog) else {
        return internal_error("catalog unavailable");
    };
    let etag = HeaderValue::from_str(&format!("\"{}\"", crate::auth::hash_session_token(&body)))
        .unwrap_or_else(|_| HeaderValue::from_static("\"0\""));
    let matches = request
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            let current = etag.to_str().unwrap_or_default();
            value
                .split(',')
                .map(str::trim)
                .any(|candidate| candidate == current)
        });
    let mut response = if matches {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            body,
        )
            .into_response()
    };
    response.headers_mut().insert(header::ETAG, etag);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    response
}

/// Aggregates the catalog of the registered courses.
///
/// Each course becomes a top-level node with the `course` role, its own identity and the subtree
/// derived from the persisted data. The loose files of the course folder stay on the course
/// node itself; there is no longer a global root of loose files. A course with no scan shows up
/// empty, and the response is flagged as incomplete if any course is incomplete.
///
/// Each subtree receives the course's manual ordering preference ([`CourseOrders`]),
/// applied as an overlay over the natural order; courses stay in the automatic order.
fn build_course_catalog(
    database: &learnvault_core::Database,
    courses: &[RegisteredCourse],
    sources: &[RemoteSource],
    is_allowed: &dyn Fn(&str) -> bool,
    orders: &CourseOrders,
    covers_dir: &Path,
) -> Result<CatalogResponse, learnvault_core::Error> {
    let empty = learnvault_core::CatalogOrder::new();
    let mut complete = true;
    let mut nodes = Vec::with_capacity(courses.len());
    let mut courses: Vec<_> = courses.iter().collect();
    courses.sort_by(|a, b| {
        let a_name = a
            .root
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&a.title);
        let b_name = b
            .root
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&b.title);
        learnvault_core::natural_cmp(a_name, b_name).then_with(|| a.id.cmp(&b.id))
    });
    for course in courses {
        let catalog = learnvault_core::build_catalog(database, &course.id)?;
        complete = complete && catalog.complete;
        let (mut children, loose_files) = learnvault_core::shape_course_catalog(catalog);
        learnvault_core::apply_order(&mut children, orders.get(&course.id).unwrap_or(&empty));
        let source_folder = learnvault_core::course_source_folder(course.root.as_path());
        let structure = structure_response(learnvault_core::structure::assess_parts(
            &children,
            &loose_files,
        ));
        nodes.push(CatalogNodeResponse {
            title: course.title.clone(),
            path: String::new(),
            role: "course",
            course_id: Some(course.id.clone()),
            cover_version: crate::covers::cover_version(covers_dir, &course.id),
            source_folder,
            structure: Some(structure),
            remote: false,
            files: Vec::new(),
            loose_files: loose_files.into_iter().map(Into::into).collect(),
            children: children.into_iter().map(Into::into).collect(),
        });
    }
    // Remote sources use the same catalog derived from the database; they have no local folder, custom
    // cover nor source folder. Manual ordering still works by identity.
    //
    // A source with no roots is a single course. With roots (ADR 0004), each chosen folder is a
    // course of its own, with the root's identity, and the rest of the source stays visible as the
    // source's course for as long as it has any file. Access is decided by identity.
    let mut sources: Vec<&RemoteSource> = sources.iter().collect();
    sources.sort_by(|left, right| {
        learnvault_core::natural_cmp(&left.name, &right.name).then_with(|| left.id.cmp(&right.id))
    });
    for source in sources {
        let roots = learnvault_core::list_roots(database, &source.id)?;
        // Titles are told apart only among the roots this user can see, so a name never hints
        // at a course the user has no access to.
        let visible: Vec<_> = roots
            .iter()
            .filter(|root| is_allowed(&root.library_id))
            .cloned()
            .collect();
        let titles = root_titles(&visible);
        let mut chosen: Vec<_> = visible.iter().zip(titles).collect();
        chosen.sort_by(|(left, left_title), (right, right_title)| {
            learnvault_core::natural_cmp(left_title, right_title)
                .then_with(|| left.library_id.cmp(&right.library_id))
        });
        for (root, title) in chosen {
            let skip = root.prefix.split('/').count();
            let (node, node_complete) =
                remote_node(database, orders, &empty, title, &root.library_id, skip)?;
            complete = complete && node_complete;
            nodes.push(node);
        }
        if !is_allowed(&source.id) {
            continue;
        }
        let (node, node_complete) =
            remote_node(database, orders, &empty, source.name.clone(), &source.id, 0)?;
        // An unreadable area outside every root still makes the answer incomplete, even when
        // the rest of the source has nothing to show.
        complete = complete && node_complete;
        let has_files = !node.children.is_empty() || !node.loose_files.is_empty();
        // The rest of a split source only shows while something is left in it.
        if roots.is_empty() || has_files {
            nodes.push(node);
        }
    }
    Ok(CatalogResponse {
        complete,
        loose_files: Vec::new(),
        nodes,
    })
}

/// Course node of a remote library (a whole source, or one root of it) and whether its last
/// scan was complete.
fn remote_node(
    database: &learnvault_core::Database,
    orders: &CourseOrders,
    empty: &learnvault_core::CatalogOrder,
    title: String,
    library_id: &str,
    skip: usize,
) -> Result<(CatalogNodeResponse, bool), learnvault_core::Error> {
    let catalog = learnvault_core::build_catalog_below(database, library_id, skip)?;
    let complete = catalog.complete;
    let (mut children, loose_files) = learnvault_core::shape_course_catalog(catalog);
    learnvault_core::apply_order(&mut children, orders.get(library_id).unwrap_or(empty));
    let structure = structure_response(learnvault_core::structure::assess_parts(
        &children,
        &loose_files,
    ));
    let node = CatalogNodeResponse {
        title,
        path: String::new(),
        role: "course",
        course_id: Some(library_id.to_owned()),
        cover_version: None,
        source_folder: None,
        structure: Some(structure),
        remote: true,
        files: Vec::new(),
        loose_files: loose_files.into_iter().map(Into::into).collect(),
        children: children.into_iter().map(Into::into).collect(),
    };
    Ok((node, complete))
}

/// Titles of the roots of one source: the folder name, plus as many parent folders in
/// parentheses as it takes to tell equal names apart (`Redes (Pentest)`, `Redes (A/x)`).
fn root_titles(roots: &[learnvault_core::SourceRoot]) -> Vec<String> {
    let segments: Vec<Vec<&str>> = roots
        .iter()
        .map(|root| root.prefix.split('/').collect())
        .collect();
    let title = |parts: &[&str], depth: usize| {
        let (parents, name) = parts.split_at(parts.len() - 1);
        let keep = depth.saturating_sub(1).min(parents.len());
        let name = learnvault_core::titles::presentable(name[0]);
        match keep {
            0 => name,
            _ => format!("{} ({})", name, parents[parents.len() - keep..].join("/")),
        }
    };
    segments
        .iter()
        .enumerate()
        .map(|(index, parts)| {
            // The shallowest depth at which no other visible root has the same title.
            for depth in 1..=parts.len() {
                let mine = title(parts, depth);
                let clash = segments
                    .iter()
                    .enumerate()
                    .any(|(other, others)| other != index && title(others, depth) == mine);
                if !clash {
                    return mine;
                }
            }
            title(parts, parts.len())
        })
        .collect()
}

pub(crate) async fn rescan(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return service_unavailable(),
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
    // Avoids holding locks just to return 503 under saturation.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let Ok(_local_lock) = state.rescan_lock.clone().try_lock_owned() else {
        return status_private_no_store(StatusCode::CONFLICT);
    };

    let courses = state.courses_snapshot();
    if courses.is_empty() {
        return private_no_store(
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "no_courses_registered",
                }),
            )
                .into_response(),
        );
    }

    let covers_dir = crate::covers::covers_dir(&state.config.database_path);
    let lock_path = rescan_lock_path(&state.config.database_path);
    let Some(lock_permit) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let _file_lock = match tokio::task::spawn_blocking(move || {
        let _lock_permit = lock_permit;
        try_rescan_file_lock(&lock_path)
    })
    .await
    {
        Ok(Ok(file)) => file,
        Ok(Err(RescanFailure::Busy)) => return status_private_no_store(StatusCode::CONFLICT),
        Ok(Err(RescanFailure::Capacity)) => return service_unavailable(),
        Ok(Err(RescanFailure::Failed)) | Err(_) => return internal_error("rescan failed"),
    };

    match run_rescan_courses(&state, &courses, &covers_dir).await {
        Ok(summary) => private_no_store(Json(summary).into_response()),
        Err(RescanFailure::Busy) => status_private_no_store(StatusCode::CONFLICT),
        Err(RescanFailure::Capacity) => service_unavailable(),
        Err(RescanFailure::Failed) => internal_error("rescan failed"),
    }
}

/// Scans each registered course outside the database lock and acquires it only per course write.
async fn run_rescan_courses(
    state: &AppState,
    courses: &[RegisteredCourse],
    covers_dir: &Path,
) -> Result<RescanResponse, RescanFailure> {
    let mut aggregate = RescanSummary {
        complete: true,
        ..RescanSummary::default()
    };
    let mut failed_courses = 0usize;

    for course in courses {
        let scan_permit = state
            .try_acquire_blocking_task()
            .ok_or(RescanFailure::Capacity)?;
        let course_path = course.root.as_path().to_path_buf();
        let scanned = tokio::task::spawn_blocking(move || {
            let _scan_permit = scan_permit;
            match CourseRoot::resolve(&course_path) {
                Ok(root) => {
                    let outcome = learnvault_core::scan(root.as_path());
                    Some((Some(root), outcome))
                }
                Err(CourseRootError::Missing) if course_entry_is_absent(&course_path) => Some((
                    None,
                    Ok(ScanOutcome {
                        files: Vec::new(),
                        inaccessible: Vec::new(),
                    }),
                )),
                Err(_) => None,
            }
        })
        .await
        .map_err(|_| RescanFailure::Failed)?;

        let Some((root, outcome)) = scanned else {
            failed_courses += 1;
            aggregate.complete = false;
            continue;
        };

        let mut database = state.database.clone().lock_owned().await;
        let db_permit = state
            .try_acquire_blocking_task()
            .ok_or(RescanFailure::Capacity)?;
        let course_id = course.id.clone();
        let registered_root = course.root.clone();
        let covers_dir = covers_dir.to_path_buf();
        let applied = tokio::task::spawn_blocking(move || {
            let _db_permit = db_permit;
            let result = learnvault_core::apply_rescan_result(
                &mut database,
                &course_id,
                outcome,
                root.as_ref().unwrap_or(&registered_root).as_path(),
            );
            if let (Ok(_), Some(root)) = (&result, root) {
                let _ = crate::covers::auto_detect_course_cover(
                    &covers_dir,
                    &course_id,
                    root.as_path(),
                );
            }
            result
        })
        .await
        .map_err(|_| RescanFailure::Failed)?;

        match applied {
            Ok(summary) => {
                aggregate.scan_id = aggregate.scan_id.max(summary.scan_id);
                aggregate.complete = aggregate.complete && summary.complete;
                aggregate.inserted += summary.inserted;
                aggregate.updated += summary.updated;
                aggregate.marked_missing += summary.marked_missing;
                aggregate.preserved += summary.preserved;
                aggregate.ambiguous.extend(summary.ambiguous);
            }
            Err(_) => {
                failed_courses += 1;
                aggregate.complete = false;
            }
        }
    }

    let mut response = RescanResponse::from(aggregate);
    response.courses = courses.len();
    response.failed_courses = failed_courses;
    Ok(response)
}

/// An absent entry under a readable parent is covered by the scan. An inaccessible
/// parent or a parent that now resolves elsewhere cannot establish that absence.
fn course_entry_is_absent(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    CourseRoot::resolve(parent).is_ok_and(|root| root.as_path() == parent)
        && std::fs::symlink_metadata(path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

pub(crate) async fn authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<learnvault_core::AuthSession>, StateError> {
    let Some(token) = request_token(headers) else {
        return Ok(None);
    };
    state.session_user(hash_session_token(&token)).await
}

pub(crate) fn internal_error(message: &'static str) -> Response {
    private_no_store(
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: message }),
        )
            .into_response(),
    )
}

pub(crate) fn service_unavailable() -> Response {
    private_no_store(
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "server busy",
            }),
        )
            .into_response(),
    )
}

pub(crate) fn private_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

pub(crate) fn status_private_no_store(status: StatusCode) -> Response {
    private_no_store(status.into_response())
}

pub(crate) fn credentials_required() -> Response {
    private_no_store(
        (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "credentials_change_required",
            }),
        )
            .into_response(),
    )
}

pub(crate) fn csrf_failed() -> Response {
    private_no_store(
        (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "csrf_failed",
            }),
        )
            .into_response(),
    )
}

fn rescan_lock_path(database_path: &Path) -> PathBuf {
    let mut path = database_path.as_os_str().to_os_string();
    path.push(".rescan.lock");
    path.into()
}

fn try_rescan_file_lock(path: &Path) -> Result<File, RescanFailure> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| RescanFailure::Failed)?;
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => RescanFailure::Busy,
        std::fs::TryLockError::Error(_) => RescanFailure::Failed,
    })?;
    Ok(file)
}

impl From<CatalogNode> for CatalogNodeResponse {
    fn from(node: CatalogNode) -> Self {
        Self {
            title: node.title,
            path: node.path,
            role: node_role(node.role),
            course_id: None,
            cover_version: None,
            source_folder: None,
            structure: None,
            remote: false,
            files: node.files.into_iter().map(Into::into).collect(),
            loose_files: node.loose_files.into_iter().map(Into::into).collect(),
            children: node.children.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CatalogFile> for CatalogFileResponse {
    fn from(file: CatalogFile) -> Self {
        Self {
            media_id: file.media_id,
            name: file.name,
            path: file.path,
            size: file.size,
            mtime_seconds: file.mtime_seconds,
            kind: media_kind(file.kind),
            availability: availability(file.availability),
            unverified: file.unverified,
        }
    }
}

impl From<RescanSummary> for RescanResponse {
    fn from(summary: RescanSummary) -> Self {
        Self {
            scan_id: summary.scan_id,
            complete: summary.complete,
            // A summary describes a single course; the aggregation of several courses replaces the
            // two counts after the conversion, like the desktop's DTO.
            courses: 1,
            failed_courses: 0,
            inserted: summary.inserted,
            updated: summary.updated,
            marked_missing: summary.marked_missing,
            preserved: summary.preserved,
            // Never serializes the keys: the response exposes only the count, like the desktop.
            ambiguous_count: summary.ambiguous.len(),
        }
    }
}

const fn node_role(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Category => "category",
        NodeRole::Course => "course",
        NodeRole::Module => "module",
        NodeRole::Lesson => "lesson",
    }
}

const fn media_kind(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Media => "media",
        MediaKind::Subtitle => "subtitle",
        MediaKind::Material => "material",
    }
}

const fn availability(value: Availability) -> &'static str {
    match value {
        Availability::Available => "available",
        Availability::Missing => "missing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescan_response_exposes_only_the_ambiguous_count() {
        // The summary carries relative paths in the ambiguous keys; the response must never
        // serialize them, exposing only the count (like the desktop's DTO).
        let summary = RescanSummary {
            ambiguous: vec!["Cursos secretos/aula.mp4".to_owned()],
            ..RescanSummary::default()
        };

        let json = serde_json::to_value(RescanResponse::from(summary)).unwrap();

        assert_eq!(json["ambiguous_count"], 1);
        assert!(
            json.get("ambiguous").is_none(),
            "a resposta não pode expor a lista de caminhos: {json}"
        );
        let rendered = json.to_string();
        assert!(
            !rendered.contains("Cursos secretos") && !rendered.contains("aula.mp4"),
            "nenhum caminho relativo pode cruzar a API: {rendered}"
        );
    }

    #[test]
    fn catalog_node_response_serializes_source_folder_when_present() {
        let node = CatalogNodeResponse {
            title: "Curso Teste".to_owned(),
            path: String::new(),
            role: "course",
            course_id: Some("c1".to_owned()),
            cover_version: None,
            source_folder: Some("Cursos 2026".to_owned()),
            structure: None,
            remote: false,
            files: Vec::new(),
            loose_files: Vec::new(),
            children: Vec::new(),
        };

        let json = serde_json::to_value(node).unwrap();
        assert_eq!(json["source_folder"], "Cursos 2026");

        let without = CatalogNodeResponse {
            title: "Curso Sem Pasta".to_owned(),
            path: String::new(),
            role: "course",
            course_id: Some("c2".to_owned()),
            cover_version: None,
            source_folder: None,
            structure: None,
            remote: false,
            files: Vec::new(),
            loose_files: Vec::new(),
            children: Vec::new(),
        };

        let json_without = serde_json::to_value(without).unwrap();
        assert!(json_without.get("source_folder").is_none());
        // A local course carries no `remote` key at all; only a remote one says so.
        assert!(json_without.get("remote").is_none());
        assert!(json.get("remote").is_none());
    }

    #[test]
    fn a_remote_course_node_says_it_is_remote() {
        let node = CatalogNodeResponse {
            title: "Redes".to_owned(),
            path: String::new(),
            role: "course",
            course_id: Some("r1".to_owned()),
            cover_version: None,
            source_folder: None,
            structure: None,
            remote: true,
            files: Vec::new(),
            loose_files: Vec::new(),
            children: Vec::new(),
        };
        assert_eq!(serde_json::to_value(node).unwrap()["remote"], true);
    }

    #[test]
    fn course_structure_serializes_confidence_and_concerns() {
        use learnvault_core::structure::{Concern, Confidence, StructureAssessment};
        let response = structure_response(StructureAssessment {
            confidence: Confidence::Medium,
            concerns: vec![Concern::LessonsWithoutNumbering],
        });
        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["confidence"], "medium");
        assert_eq!(json["concerns"][0], "lessons_without_numbering");
    }
}

#[cfg(test)]
mod title_tests {
    use super::root_titles;
    use learnvault_core::SourceRoot;

    fn roots(prefixes: &[&str]) -> Vec<SourceRoot> {
        prefixes
            .iter()
            .enumerate()
            .map(|(index, prefix)| SourceRoot {
                library_id: index.to_string(),
                prefix: (*prefix).to_owned(),
            })
            .collect()
    }

    #[test]
    fn a_release_style_folder_name_reads_as_words() {
        assert_eq!(
            root_titles(&roots(&[
                "Cursos/Go.Golang.Do.zero",
                "Cursos/Quimica_Para_Disc"
            ])),
            ["Go Golang Do zero", "Quimica Para Disc"]
        );
    }

    #[test]
    fn a_unique_name_is_the_folder_name() {
        assert_eq!(
            root_titles(&roots(&["Cursos/Rust", "Cursos/Go"])),
            ["Rust", "Go"]
        );
    }

    #[test]
    fn equal_names_get_as_many_parents_as_needed() {
        assert_eq!(
            root_titles(&roots(&["A/Redes", "B/Redes", "Solo"])),
            ["Redes (A)", "Redes (B)", "Solo"]
        );
        assert_eq!(
            root_titles(&roots(&["A/x/Redes", "B/x/Redes"])),
            ["Redes (A/x)", "Redes (B/x)"]
        );
        // Three roots with one name: the shortest title that no other root shares.
        assert_eq!(
            root_titles(&roots(&["A/x/Redes", "B/x/Redes", "B/y/Redes"])),
            ["Redes (A/x)", "Redes (B/x)", "Redes (y)"]
        );
    }

    #[test]
    fn a_root_alone_is_never_decorated() {
        assert_eq!(root_titles(&roots(&["A/B/Redes"])), ["Redes"]);
        assert!(root_titles(&[]).is_empty());
    }
}
