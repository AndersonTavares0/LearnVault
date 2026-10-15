use std::{
    fs::File,
    path::{Component, Path, PathBuf},
    pin::Pin,
    task::{Context, Poll},
};

use axum::{
    body::Body,
    extract::State,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, ReadBuf},
    sync::OwnedSemaphorePermit,
};
use tokio_util::io::ReaderStream;

use crate::AppState;

/// Arquivos estáticos permanecem pequenos para que uma raiz web mal configurada não esgote
/// recursos. Após validar os metadados, o arquivo é transmitido sem carregá-lo em memória.
const MAX_SPA_FILE_SIZE: u64 = 16 * 1024 * 1024;

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

    match open_file(&state, root, &target, false).await {
        Ok(response) => return response,
        Err(FileError::TooLarge) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(FileError::Busy) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(FileError::Missing) => {}
    }
    if Path::new(path).extension().is_some() {
        return StatusCode::NOT_FOUND.into_response();
    }

    let shell = root.join("200.html");
    match open_file(&state, root, &shell, true).await {
        Ok(response) => response,
        Err(FileError::TooLarge) => StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(FileError::Missing | FileError::Busy) => {
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
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

/// Abre e valida o arquivo antes de transmitir *o mesmo handle*.
///
/// A raiz web configurada (`--web-root`) é canonicalizada na inicialização e deve permanecer um
/// limite de deploy confiável e somente leitura. Cada requisição rejeita symlinks (e reparse
/// points no Windows) no caminho,
/// então valida o handle aberto. Isso impede escapes óbvios por links e não reabre o caminho após
/// a validação. APIs seguras e portáveis de Rust não oferecem contenção atômica contra um processo
/// hostil substituindo diretórios concorrentemente; portanto, o deploy não pode conceder escrita
/// não confiável sobre a raiz web.
async fn open_file(
    state: &AppState,
    root: &Path,
    target: &Path,
    shell: bool,
) -> Result<Response, FileError> {
    let Some(stream_permit) = state.try_acquire_spa_stream() else {
        return Err(FileError::Busy);
    };
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

    let stream = ReaderStream::new(PermitReader {
        reader: tokio::fs::File::from_std(opened.file).take(opened.size),
        _permit: stream_permit,
    });
    Ok(file_response(
        Body::from_stream(stream),
        &root,
        &opened.path,
        shell,
        opened.size,
    ))
}

/// Mantém a reserva de streaming viva enquanto `ReaderStream` ainda puder ler o corpo.
struct PermitReader<R> {
    reader: R,
    _permit: OwnedSemaphorePermit,
}

impl<R: AsyncRead + Unpin> AsyncRead for PermitReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().reader).poll_read(cx, buf)
    }
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
