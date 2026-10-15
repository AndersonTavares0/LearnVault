use std::path::{Component, Path, PathBuf};

use axum::{
    body::Body,
    extract::State,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};

use crate::AppState;

/// Entrega arquivos da SPA sem permitir que a requisição saia da raiz configurada.
pub(crate) async fn serve(State(state): State<AppState>, uri: Uri) -> Response {
    let Some(root) = state.config.web_root.as_deref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    let target = match requested_path(root, path) {
        Some(path) => path,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    if let Some(response) = read_file(root, &target, false).await {
        return response;
    }
    if Path::new(path).extension().is_some() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let shell = root.join("200.html");
    read_file(root, &shell, true)
        .await
        .unwrap_or_else(|| StatusCode::SERVICE_UNAVAILABLE.into_response())
}

fn requested_path(root: &Path, path: &str) -> Option<PathBuf> {
    // URI paths are percent encoded. Rejecting encoded paths prevents ambiguous filesystem
    // normalization while keeping the configured root as the sole authority.
    if path.contains('%') {
        return None;
    }
    let relative = Path::new(path);
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(relative))
}

async fn read_file(root: &Path, target: &Path, shell: bool) -> Option<Response> {
    let root = root.to_owned();
    let target = target.to_owned();
    tokio::task::spawn_blocking(move || {
        let resolved = target.canonicalize().ok()?;
        if !resolved.starts_with(&root) || !resolved.is_file() {
            return None;
        }
        let bytes = std::fs::read(&resolved).ok()?;
        Some(file_response(bytes, &resolved, shell))
    })
    .await
    .ok()
    .flatten()
}

fn file_response(bytes: Vec<u8>, path: &Path, shell: bool) -> Response {
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, mime(path));
    headers.insert(
        header::CACHE_CONTROL,
        if shell {
            header::HeaderValue::from_static("no-cache")
        } else if is_immutable_asset(path) {
            header::HeaderValue::from_static("public, max-age=31536000, immutable")
        } else {
            header::HeaderValue::from_static("no-cache")
        },
    );
    response
}

fn is_immutable_asset(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('\\', "/")
        .contains("/_app/immutable/")
}

fn mime(path: &Path) -> header::HeaderValue {
    let value = match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css",
        Some("js" | "mjs") => "text/javascript",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    };
    header::HeaderValue::from_static(value)
}
