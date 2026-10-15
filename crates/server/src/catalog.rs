//! Endpoints HTTP do catálogo e do rescan administrativo.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{
    Availability, Catalog, CatalogFile, CatalogNode, MediaKind, NodeRole, RescanSummary,
};
use serde::Serialize;

use crate::{
    AppState,
    auth::{hash_session_token, request_token, same_origin},
};

const LIBRARY_ID: &str = "library-1";

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
    if !authenticated(&state, &headers).await {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let database = state.database.clone();
    match tokio::task::spawn_blocking(move || {
        let database = database.blocking_lock();
        learnvault_core::build_catalog(&database, LIBRARY_ID)
    })
    .await
    {
        Ok(Ok(catalog)) => Json(CatalogResponse::from(catalog)).into_response(),
        Ok(Err(_)) | Err(_) => internal_error("catalog unavailable"),
    }
}

pub(crate) async fn rescan(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(user) = authenticated_user(&state, &headers).await else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if !user.is_admin || !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(_rescan_guard) = state.rescan_lock.try_lock() else {
        return StatusCode::CONFLICT.into_response();
    };

    let database = state.database.clone();
    let root = state.config.library_root.clone();
    match tokio::task::spawn_blocking(move || {
        let mut database = database.blocking_lock();
        learnvault_core::rescan(&mut database, LIBRARY_ID, &root)
    })
    .await
    {
        Ok(Ok(summary)) => Json(RescanResponse::from(summary)).into_response(),
        Ok(Err(_)) | Err(_) => internal_error("rescan failed"),
    }
}

async fn authenticated(state: &AppState, headers: &HeaderMap) -> bool {
    authenticated_user(state, headers).await.is_some()
}

async fn authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Option<learnvault_core::AuthSession> {
    let token = request_token(headers)?;
    state
        .session_user(hash_session_token(&token))
        .await
        .ok()
        .flatten()
}

fn internal_error(message: &'static str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
        .into_response()
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
