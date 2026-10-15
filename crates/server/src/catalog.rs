//! Endpoints HTTP do catálogo e do rescan administrativo.

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
    Availability, Catalog, CatalogFile, CatalogNode, MediaKind, NodeRole, RescanSummary,
};
use serde::Serialize;

use crate::{
    AppState, LIBRARY_ID, StateError,
    auth::{hash_session_token, request_token, same_origin},
};

enum RescanFailure {
    Busy,
    Failed,
    RootChanged,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Serialize)]
struct CatalogResponse {
    complete: bool,
    loose_files: Vec<CatalogFileResponse>,
    categories: Vec<CatalogNodeResponse>,
}

#[derive(Serialize)]
struct CatalogNodeResponse {
    title: String,
    path: String,
    role: &'static str,
    files: Vec<CatalogFileResponse>,
    loose_files: Vec<CatalogFileResponse>,
    children: Vec<CatalogNodeResponse>,
}

#[derive(Serialize)]
struct CatalogFileResponse {
    media_id: String,
    name: String,
    path: String,
    size: u64,
    kind: &'static str,
    availability: &'static str,
    unverified: bool,
}

#[derive(Serialize)]
struct RescanResponse {
    scan_id: i64,
    complete: bool,
    inserted: usize,
    updated: usize,
    marked_missing: usize,
    preserved: usize,
    ambiguous: Vec<String>,
}

pub(crate) async fn catalog(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => {}
        Ok(Some(_)) => return credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    }

    // Evita segurar o mutex do banco só para devolver 503 sob saturação.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        learnvault_core::build_catalog(&database, LIBRARY_ID)
    })
    .await
    {
        Ok(Ok(catalog)) => private_no_store(Json(CatalogResponse::from(catalog)).into_response()),
        Ok(Err(_)) | Err(_) => internal_error("catalog unavailable"),
    }
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
    // Evita segurar locks só para devolver 503 sob saturação.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let Ok(local_lock) = state.rescan_lock.clone().try_lock_owned() else {
        return status_private_no_store(StatusCode::CONFLICT);
    };

    let database = state.database.clone();
    let root = state.config.library_root.clone();
    let lock_path = rescan_lock_path(&state.config.database_path);
    let mut database = database.lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    match tokio::task::spawn_blocking(move || {
        // Os guards e o permit pertencem deliberadamente à tarefa blocking. Tokio continua
        // executando-a depois que o JoinHandle é descartado, então um timeout HTTP não libera
        // a capacidade nem os locks enquanto o scan ainda usa o banco.
        let _blocking_task = blocking_task;
        let _local_lock = local_lock;
        let _file_lock = try_rescan_file_lock(&lock_path)?;
        if root.canonicalize().ok().as_deref() != Some(root.as_path()) {
            return Err(RescanFailure::RootChanged);
        }
        learnvault_core::rescan(&mut database, LIBRARY_ID, &root).map_err(|_| RescanFailure::Failed)
    })
    .await
    {
        Ok(Ok(summary)) => private_no_store(Json(RescanResponse::from(summary)).into_response()),
        Ok(Err(RescanFailure::Busy)) => status_private_no_store(StatusCode::CONFLICT),
        Ok(Err(RescanFailure::RootChanged)) => internal_error("library root changed"),
        Ok(Err(RescanFailure::Failed)) | Err(_) => internal_error("rescan failed"),
    }
}

async fn authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<learnvault_core::AuthSession>, StateError> {
    let Some(token) = request_token(headers) else {
        return Ok(None);
    };
    state.session_user(hash_session_token(&token)).await
}

fn internal_error(message: &'static str) -> Response {
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

fn private_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

fn status_private_no_store(status: StatusCode) -> Response {
    private_no_store(status.into_response())
}

fn credentials_required() -> Response {
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

fn csrf_failed() -> Response {
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

impl From<Catalog> for CatalogResponse {
    fn from(catalog: Catalog) -> Self {
        Self {
            complete: catalog.complete,
            loose_files: catalog.loose_files.into_iter().map(Into::into).collect(),
            categories: catalog.categories.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<CatalogNode> for CatalogNodeResponse {
    fn from(node: CatalogNode) -> Self {
        Self {
            title: node.title,
            path: node.path,
            role: node_role(node.role),
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
            inserted: summary.inserted,
            updated: summary.updated,
            marked_missing: summary.marked_missing,
            preserved: summary.preserved,
            ambiguous: summary.ambiguous,
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
