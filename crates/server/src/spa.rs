use std::{
    fs::File,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use tokio::io::AsyncReadExt;
use tokio_util::io::StreamReader;

use crate::{AppState, stream::idle_timeout_stream};

/// Static files stay small so that a misconfigured web root does not exhaust
/// resources. After validating the metadata, the file is streamed without loading it into memory.
const MAX_SPA_FILE_SIZE: u64 = 16 * 1024 * 1024;
const SPA_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// Delivers SPA files without letting the request leave the configured root.
pub(crate) async fn serve(State(state): State<AppState>, uri: Uri) -> Response {
    let Some(root) = state.config.web_root.as_deref() else {
        return error_no_store(StatusCode::NOT_FOUND);
    };
    let path = uri.path().trim_start_matches('/');
    if path == "api" || path.starts_with("api/") {
        return error_no_store(StatusCode::NOT_FOUND);
    }

    let target = match requested_path(root, path) {
        Some(path) => path,
        None => return error_no_store(StatusCode::NOT_FOUND),
    };

    if target == root.join("200.html") {
        return serve_shell(&state)
            .await
            .unwrap_or_else(|error| match error {
                FileError::TooLarge => error_no_store(StatusCode::PAYLOAD_TOO_LARGE),
                FileError::Missing | FileError::Busy => {
                    error_no_store(StatusCode::SERVICE_UNAVAILABLE)
                }
            });
    }

    // Extensionless routes may depend on the fallback to the shell. Check its limit before
    // reserving capacity for a possible asset, so as not to mask an invalid configuration
    // (413) as operational saturation (503).
    if Path::new(path).extension().is_none() && state.spa_shell_is_too_large() {
        return error_no_store(StatusCode::PAYLOAD_TOO_LARGE);
    }

    match open_file(&state, root, &target, false).await {
        Ok(response) => return response,
        Err(FileError::TooLarge) => return error_no_store(StatusCode::PAYLOAD_TOO_LARGE),
        Err(FileError::Busy) => return error_no_store(StatusCode::SERVICE_UNAVAILABLE),
        Err(FileError::Missing) => {}
    }
    if Path::new(path).extension().is_some() {
        return error_no_store(StatusCode::NOT_FOUND);
    }

    match serve_shell(&state).await {
        Ok(response) => response,
        Err(FileError::TooLarge) => error_no_store(StatusCode::PAYLOAD_TOO_LARGE),
        Err(FileError::Missing | FileError::Busy) => {
            error_no_store(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

/// Static errors must never be stored by proxies: a stale 503 or 404
/// would prolong an unavailability that has already been resolved.
fn error_no_store(status: StatusCode) -> Response {
    let mut response = status.into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn serve_shell(state: &AppState) -> Result<Response, FileError> {
    if state.spa_shell_is_too_large() {
        return Err(FileError::TooLarge);
    }
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return Err(FileError::Busy);
    };
    let shell_state = state.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        shell_state.current_spa_shell()
    })
    .await
    .map_err(|_| FileError::Busy)?
    .ok_or(FileError::Missing)?;
    let size = bytes.len() as u64;
    let Some(stream_permit) = state.try_acquire_spa_stream(true) else {
        return Err(FileError::Busy);
    };
    // The shell was already checked against the hash cache above. Recreating a reader from those
    // bytes keeps the reservation until the body is consumed, dropped or goes idle, without reopening the
    // path and without introducing a second TOCTOU window.
    let reader = StreamReader::new(tokio_stream::once(Ok::<_, std::io::Error>(
        Bytes::copy_from_slice(&bytes),
    )));
    let Some(web_root) = state.config.web_root.as_deref() else {
        // `serve` only calls this function with `web_root` configured; the absence here
        // indicates an inconsistent state, treated as a missing shell, without `panic`.
        return Err(FileError::Missing);
    };
    Ok(file_response(
        Body::from_stream(idle_timeout_stream(
            reader,
            stream_permit,
            SPA_STREAM_IDLE_TIMEOUT,
        )),
        web_root,
        Path::new("200.html"),
        true,
        size,
    ))
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

/// Opens and validates the file before streaming *the same handle*.
///
/// The configured web root (`--web-root`) is canonicalized at startup and must remain a trusted,
/// read-only deploy boundary. Each request rejects symlinks (and reparse
/// points on Windows) in the path,
/// then validates the open handle. This prevents obvious escapes through links and does not reopen the path after
/// validation. Safe, portable Rust APIs do not offer atomic containment against a hostile process
/// replacing directories concurrently; therefore, the deploy must not grant untrusted write access
/// to the web root.
async fn open_file(
    state: &AppState,
    root: &Path,
    target: &Path,
    shell: bool,
) -> Result<Response, FileError> {
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return Err(FileError::Busy);
    };
    let root = root.to_owned();
    let open_root = root.clone();
    let target = target.to_owned();
    let opened = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        open_checked_file(&open_root, &target)
    })
    .await
    .map_err(|_| FileError::Busy)??;
    // Only reserve a stream after confirming that the file exists. Extensionless SPA routes
    // first check for a possible asset and then fall back to the shell; consuming the
    // asset reservation in that check would make slow assets block the fallback.
    let Some(stream_permit) = state.try_acquire_spa_stream(shell) else {
        return Err(FileError::Busy);
    };

    let stream = idle_timeout_stream(
        tokio::fs::File::from_std(opened.file).take(opened.size),
        stream_permit,
        SPA_STREAM_IDLE_TIMEOUT,
    );
    Ok(file_response(
        Body::from_stream(stream),
        &root,
        &opened.path,
        shell,
        opened.size,
    ))
}

struct OpenedFile {
    file: File,
    path: PathBuf,
    size: u64,
}

enum FileError {
    Missing,
    TooLarge,
    Busy,
}

fn open_checked_file(root: &Path, target: &Path) -> Result<OpenedFile, FileError> {
    reject_linked_components(root, target)?;
    let resolved = target.canonicalize().map_err(|_| FileError::Missing)?;
    if !resolved.starts_with(root) || is_link(&resolved)? {
        return Err(FileError::Missing);
    }

    let file = File::open(&resolved).map_err(|_| FileError::Missing)?;
    let metadata = file.metadata().map_err(|_| FileError::Missing)?;
    if !metadata.is_file() {
        return Err(FileError::Missing);
    }
    if metadata.len() > MAX_SPA_FILE_SIZE {
        return Err(FileError::TooLarge);
    }
    Ok(OpenedFile {
        file,
        path: resolved,
        size: metadata.len(),
    })
}

fn reject_linked_components(root: &Path, target: &Path) -> Result<(), FileError> {
    let relative = target.strip_prefix(root).map_err(|_| FileError::Missing)?;
    let mut current = root.to_owned();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(FileError::Missing);
        };
        current.push(component);
        if is_link(&current)? {
            return Err(FileError::Missing);
        }
    }
    Ok(())
}

fn is_link(path: &Path) -> Result<bool, FileError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| FileError::Missing)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        Ok(metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }
    #[cfg(not(windows))]
    {
        Ok(metadata.file_type().is_symlink())
    }
}

fn file_response(body: Body, root: &Path, path: &Path, shell: bool, size: u64) -> Response {
    let mut response = Response::new(body);
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_LENGTH, header::HeaderValue::from(size));
    headers.insert(header::CONTENT_TYPE, mime(path));
    headers.insert(
        header::CACHE_CONTROL,
        if shell {
            header::HeaderValue::from_static("no-cache")
        } else if is_immutable_asset(root, path) {
            header::HeaderValue::from_static("public, max-age=31536000, immutable")
        } else {
            header::HeaderValue::from_static("no-cache")
        },
    );
    response
}

fn is_immutable_asset(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .is_ok_and(|relative| relative.starts_with(Path::new("_app").join("immutable")))
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
