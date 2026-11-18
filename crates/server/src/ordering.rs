//! Manual ordering of the catalog's modules and lessons (issue #239).
//!
//! The course library is **read-only**: the order chosen by the user lives in a
//! versioned file next to the database (the same directory as the course registry and the
//! covers), never inside the course folders. Each course has its own preference,
//! identified by the course's opaque identity; the entries point only to already-catalogued
//! relative paths.
//!
//! The automatic order (natural, deterministic) stays the default: the preference is
//! applied as an overlay in [`learnvault_core::apply_order`] before the tree is
//! served. That is why removing the preference restores the automatic order without rewriting anything.

use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use axum::{
    Json,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{
    CatalogOrder, build_catalog, shape_course_catalog, sibling_paths, validate_siblings,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, StateError,
    auth::same_origin,
    catalog::{
        authenticated_user, credentials_required, csrf_failed, internal_error, private_no_store,
        service_unavailable, status_private_no_store,
    },
    courses::replace_file,
};

/// Name of the ordering file inside the server's data directory.
pub const ORDER_FILE_NAME: &str = "catalog-order.json";

/// Version of the ordering file's format.
pub const ORDER_VERSION: u32 = 1;

/// Ordering preferences of all courses, by the course's opaque identity.
pub type CourseOrders = BTreeMap<String, CatalogOrder>;

/// Serialized format of the ordering file.
#[derive(Debug, Serialize, Deserialize)]
struct OrderFile {
    version: u32,
    /// Course -> (parent path -> desired order of the children).
    courses: BTreeMap<String, BTreeMap<String, Vec<String>>>,
}

/// Path of the ordering file for a database. It sits next to the database, in the data directory,
/// like the course registry and the covers.
pub(crate) fn order_path(database_path: &Path) -> PathBuf {
    let parent = database_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join(ORDER_FILE_NAME)
}

/// Loads the persisted preferences.
///
/// A missing, corrupted or unknown-version file results in no preference
/// (automatic order); no message carries a path.
pub(crate) fn load_orders(path: &Path) -> CourseOrders {
    let Ok(raw) = std::fs::read(path) else {
        return CourseOrders::new();
    };
    let Ok(stored) = serde_json::from_slice::<OrderFile>(&raw) else {
        return CourseOrders::new();
    };
    if stored.version != ORDER_VERSION {
        return CourseOrders::new();
    }
    stored
        .courses
        .into_iter()
        .map(|(course_id, parents)| (course_id, CatalogOrder::from_map(parents)))
        .filter(|(_, order)| !order.is_empty())
        .collect()
}

/// Writes the preferences in a serialized, atomic way.
///
/// The temporary file is created exclusively (`create_new`) and swapped by `rename` via
/// [`replace_file`]. Serialization is the responsibility of the state's write lock.
pub(crate) fn store_orders(path: &Path, orders: &CourseOrders) -> Result<(), OrderError> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(directory).map_err(|_| OrderError::WriteFailed)?;

    let stored = OrderFile {
        version: ORDER_VERSION,
        courses: orders
            .iter()
            .filter(|(_, order)| !order.is_empty())
            .map(|(course_id, order)| (course_id.clone(), order.as_map().clone()))
            .collect(),
    };
    let payload = serde_json::to_vec(&stored).map_err(|_| OrderError::WriteFailed)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("catalog-order");
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
        let _ = std::fs::remove_file(&temporary);
        return Err(OrderError::WriteFailed);
    }
    if replace_file(&temporary, path).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err(OrderError::WriteFailed);
    }
    Ok(())
}

/// Stable failure of ordering persistence. It never carries a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrderError {
    /// The ordering could not be written safely.
    WriteFailed,
}

/// Failure of the ordering layer, before translation to HTTP.
enum OrderFailure {
    /// The parent's path does not exist in the course's catalog.
    UnknownParent,
    /// The list is not an exact permutation of the siblings.
    InvalidOrder,
    /// Reading the catalog failed.
    Core,
    /// Writing the preference failed.
    WriteFailed,
}

/// Body of the ordering request: the parent's path and the desired order of its children.
#[derive(Debug, Deserialize)]
pub(crate) struct OrderRequest {
    /// Relative path of the parent node; empty for the top-level list (the course's modules).
    parent: String,
    /// Paths of the children, in the desired order. Must be an exact permutation.
    children: Vec<String>,
}

/// Restore query: a specific parent, or all of them when absent.
#[derive(Debug, Deserialize)]
pub(crate) struct ResetQuery {
    parent: Option<String>,
}

#[derive(Serialize)]
struct OrderResponse {
    parent: String,
    children: Vec<String>,
}

#[derive(Serialize)]
struct ResetResponse {
    removed: bool,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

/// `PUT /api/admin/courses/{id}/order` — sets the order of the siblings of a course node.
///
/// Administrator only, with credentials set and same origin. The course must be
/// registered; an unknown parent path, a list that is not an exact permutation
/// of the siblings or an unknown course answer `404`/`422` without revealing physical paths.
pub(crate) async fn put_order(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    headers: HeaderMap,
    Json(request): Json<OrderRequest>,
) -> Response {
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
    if !course_registered(&state, &course_id) {
        return status_private_no_store(StatusCode::NOT_FOUND);
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let database = state.database.clone().lock_owned().await;
    let orders = state.orders.clone();
    let orders_file = state.orders_file.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        set_course_order(
            &database,
            &orders,
            &orders_file,
            &course_id,
            &request.parent,
            &request.children,
        )
    })
    .await;

    match outcome {
        Ok(Ok(response)) => private_no_store(Json(response).into_response()),
        Ok(Err(OrderFailure::UnknownParent)) | Ok(Err(OrderFailure::InvalidOrder)) => {
            json_error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_order")
        }
        Ok(Err(OrderFailure::WriteFailed)) | Err(_) => internal_error("course_order_unavailable"),
        Ok(Err(OrderFailure::Core)) => internal_error("catalog unavailable"),
    }
}

/// `DELETE /api/admin/courses/{id}/order` — restores the automatic order.
///
/// Without `parent`, it removes the course's whole preference; with `parent`, it removes the preference
/// of that node. It is idempotent: restoring what was already automatic answers
/// `removed: false`. The same authentication and same-origin requirements as the `PUT`.
pub(crate) async fn reset_order(
    State(state): State<AppState>,
    AxumPath(course_id): AxumPath<String>,
    Query(query): Query<ResetQuery>,
    headers: HeaderMap,
) -> Response {
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
    if !course_registered(&state, &course_id) {
        return status_private_no_store(StatusCode::NOT_FOUND);
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let orders = state.orders.clone();
    let orders_file = state.orders_file.clone();
    let parent = query.parent;
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        clear_course_order(&orders, &orders_file, &course_id, parent.as_deref())
    })
    .await;

    match outcome {
        Ok(Ok(removed)) => private_no_store(Json(ResetResponse { removed }).into_response()),
        Ok(Err(_)) | Err(_) => internal_error("course_order_unavailable"),
    }
}

/// Validates the list against the parent's siblings and persists the preference.
fn set_course_order(
    database: &learnvault_core::Database,
    orders: &std::sync::RwLock<CourseOrders>,
    orders_file: &Path,
    course_id: &str,
    parent: &str,
    children: &[String],
) -> Result<OrderResponse, OrderFailure> {
    let catalog = build_catalog(database, course_id).map_err(|_| OrderFailure::Core)?;
    let (shaped, _) = shape_course_catalog(catalog);
    let siblings = sibling_paths(&shaped, parent).ok_or(OrderFailure::UnknownParent)?;
    if siblings.is_empty() || !validate_siblings(&siblings, children) {
        return Err(OrderFailure::InvalidOrder);
    }

    let mut guard = orders.write().map_err(|_| OrderFailure::WriteFailed)?;
    let mut next = guard.clone();
    next.entry(course_id.to_owned())
        .or_default()
        .set_children(parent, children.to_vec());
    store_orders(orders_file, &next).map_err(|_| OrderFailure::WriteFailed)?;
    *guard = next;

    Ok(OrderResponse {
        parent: parent.to_owned(),
        children: children.to_vec(),
    })
}

/// Removes a parent's preference (or the whole course's) and persists the change.
fn clear_course_order(
    orders: &std::sync::RwLock<CourseOrders>,
    orders_file: &Path,
    course_id: &str,
    parent: Option<&str>,
) -> Result<bool, OrderError> {
    let mut guard = orders.write().map_err(|_| OrderError::WriteFailed)?;
    let mut next = guard.clone();
    let removed = match parent {
        Some(parent) => next
            .get_mut(course_id)
            .is_some_and(|order| order.remove(parent)),
        None => next.remove(course_id).is_some(),
    };
    if !removed {
        return Ok(false);
    }
    if next.get(course_id).is_some_and(CatalogOrder::is_empty) {
        next.remove(course_id);
    }
    store_orders(orders_file, &next)?;
    *guard = next;
    Ok(true)
}

/// True when the identity matches a registered course.
fn course_registered(state: &AppState, course_id: &str) -> bool {
    state
        .courses_snapshot()
        .iter()
        .any(|course| course.id == course_id)
}

/// Stable JSON error, with no additional cache policy.
fn json_error(status: StatusCode, message: &'static str) -> Response {
    private_no_store((status, Json(ErrorBody { error: message })).into_response())
}
