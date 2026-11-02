//! Transcodificação e streaming sob demanda para múltiplos codecs e contêineres de vídeo.
//!
//! Permite a reprodução direta no navegador de formatos que normalmente não são suportados
//! nativamente pela tag `<video>` (como MKV, HEVC sem aceleração, áudio AC3, etc.) através
//! do remuxing ou recodificação sob demanda usando FFmpeg para Fragmented MP4 em pipe.

use std::{
    path::PathBuf,
    pin::Pin,
    sync::OnceLock,
    task::{Context, Poll},
    time::Duration,
};

use axum::{
    body::Body,
    extract::{Json, RawQuery, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, ReadBuf};

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token},
    catalog::service_unavailable,
    stream::idle_timeout_stream,
};

const TRANSCODE_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MEDIA_OPEN_TIMEOUT: Duration = Duration::from_secs(5);

static FFMPEG_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Retorna o executável do ffmpeg a ser utilizado.
pub(crate) fn ffmpeg_path() -> PathBuf {
    if let Ok(path) = std::env::var("FFMPEG_PATH") {
        PathBuf::from(path)
    } else {
        PathBuf::from("ffmpeg")
    }
}

/// Verifica se o FFmpeg está instalado e acessível no ambiente.
pub(crate) fn is_ffmpeg_available() -> bool {
    *FFMPEG_AVAILABLE.get_or_init(|| {
        let binary = ffmpeg_path();
        std::process::Command::new(binary)
            .arg("-version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct TranscodeQuery {
    /// Identidade do curso.
    course: String,
    /// Caminho relativo dentro do curso.
    path: String,
    /// Ponto inicial para seek em segundos.
    #[serde(default)]
    start: Option<f64>,
    /// Forçar recodificação completa do fluxo de vídeo para H.264.
    #[serde(default)]
    reencode: Option<bool>,
}

#[derive(Serialize)]
pub(crate) struct MediaCapabilitiesResponse {
    pub(crate) transcoding: bool,
    pub(crate) supported_codecs: Vec<&'static str>,
}

/// `GET /api/media/capabilities` — reporta as capacidades de reprodução e transcodificação do servidor.
pub(crate) async fn media_capabilities(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = request_token(&headers) else {
        return error_private_no_store(StatusCode::UNAUTHORIZED);
    };
    match state.session_user(hash_session_token(&token)).await {
        Ok(Some(user)) if !user.must_change_credentials => {}
        Ok(Some(_)) => return error_credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return error_private_no_store(StatusCode::UNAUTHORIZED),
    }

    let response = MediaCapabilitiesResponse {
        transcoding: is_ffmpeg_available(),
        supported_codecs: vec![
            "h264", "hevc", "vp9", "av1", "aac", "ac3", "eac3", "mp3", "opus", "mkv", "mp4", "webm",
        ],
    };

    (
        StatusCode::OK,
        [
            (header::CACHE_CONTROL, "private, no-store"),
            (header::CONTENT_TYPE, "application/json"),
        ],
        Json(response),
    )
        .into_response()
}

/// Stream que encapsula o stdout do processo e o processo filho com `kill_on_drop(true)`
/// e deadline de leitura contra travamento de processos filhos.
struct ProcessStdoutStream {
    stdout: tokio::process::ChildStdout,
    _child: tokio::process::Child,
    deadline: Option<Pin<Box<tokio::time::Sleep>>>,
    read_timeout: Duration,
}

impl AsyncRead for ProcessStdoutStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let timeout = self.read_timeout;
        if self.deadline.is_none() {
            self.deadline = Some(Box::pin(tokio::time::sleep(timeout)));
        }

        match Pin::new(&mut self.stdout).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                self.deadline = None;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => {
                self.deadline = None;
                Poll::Ready(Err(e))
            }
            Poll::Pending => {
                if let Some(deadline) = &mut self.deadline
                    && std::future::Future::poll(deadline.as_mut(), cx).is_ready()
                {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "ffmpeg transcode stream stalled",
                    )));
                }
                Poll::Pending
            }
        }
    }
}

/// `GET /api/media/transcode` — entrega streaming de vídeo transcodificado ou remuxado sob demanda em fMP4.
pub(crate) async fn media_transcode(
    State(state): State<AppState>,
    raw_query: RawQuery,
    headers: HeaderMap,
    method: Method,
) -> Response {
    let Some(token) = request_token(&headers) else {
        return error_private_no_store(StatusCode::UNAUTHORIZED);
    };
    match state.session_user(hash_session_token(&token)).await {
        Ok(Some(user)) if !user.must_change_credentials => {}
        Ok(Some(_)) => return error_credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return error_private_no_store(StatusCode::UNAUTHORIZED),
    }

    let query = match raw_query.0.as_deref() {
        Some(raw) => match serde_urlencoded::from_str::<TranscodeQuery>(raw) {
            Ok(query) => query,
            Err(_) => return error_private_no_store(StatusCode::BAD_REQUEST),
        },
        None => return error_private_no_store(StatusCode::BAD_REQUEST),
    };

    let Some(course) = state
        .courses_snapshot()
        .into_iter()
        .find(|c| c.id == query.course)
    else {
        return error_private_no_store(StatusCode::NOT_FOUND);
    };
    let root = course.root;
    let path = query.path;
    let lookup_path = path.clone();
    let library_id = course.id.clone();

    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let reference = {
        let database = state.database.clone().lock_owned().await;
        let Some(blocking_task) = state.try_acquire_blocking_task() else {
            return service_unavailable();
        };
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            learnvault_core::find_media_by_path(&database, &library_id, &lookup_path)
        })
        .await
    };

    let reference = match reference {
        Ok(Ok(Some(reference))) => reference,
        Ok(Ok(None)) => return error_private_no_store(StatusCode::NOT_FOUND),
        Ok(Err(error)) => return media_error_response(&error),
        Err(_) => return service_unavailable(),
    };

    let stream_permit = if method == Method::HEAD {
        None
    } else {
        match state.try_acquire_media_stream() {
            Some(permit) => Some(permit),
            None => return service_unavailable(),
        }
    };

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };

    let open_root = root.clone();
    let open_reference = reference.clone();
    let _opened = match tokio::time::timeout(
        MEDIA_OPEN_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            learnvault_core::open_media_file(&open_reference, open_root.as_path())
        }),
    )
    .await
    {
        Ok(Ok(Ok(opened))) => opened,
        Ok(Ok(Err(error))) => return media_error_response(&error),
        Ok(Err(_join_err)) => return service_unavailable(),
        Err(_timeout_err) => return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if method == Method::HEAD {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::OK;
        let headers = response.headers_mut();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("video/mp4"));
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        );
        headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("none"));
        return response;
    }

    if !is_ffmpeg_available() {
        return (
            StatusCode::NOT_IMPLEMENTED,
            [
                (header::CACHE_CONTROL, "private, no-store"),
                (header::CONTENT_TYPE, "application/json"),
            ],
            Json(serde_json::json!({
                "error": "transcoding_unavailable",
                "message": "FFmpeg não está disponível no servidor para transcodificação sob demanda"
            })),
        )
            .into_response();
    }

    let canonical_root = match root.as_path().canonicalize() {
        Ok(r) => r,
        Err(_) => return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let canonical_path =
        match learnvault_core::paths::resolve_within(&canonical_root, &reference.relative_path) {
            Ok(path) => path,
            Err(_) => return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR),
        };

    let Some(stream_permit) = stream_permit else {
        return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let mut cmd = tokio::process::Command::new(ffmpeg_path());
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .args(["-protocol_whitelist", "file,crypto,data"]);

    let start = query.start.unwrap_or(0.0);
    let start = if start.is_finite() && start > 0.0 {
        start
    } else {
        0.0
    };
    if start > 0.0 {
        cmd.arg("-ss").arg(format!("{start:.3}"));
    }

    cmd.arg("-i").arg(canonical_path);

    let reencode = query.reencode.unwrap_or(false);
    if reencode {
        cmd.args([
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-c:a", "aac", "-b:a", "192k",
        ]);
    } else {
        // Modo inteligente: copia fluxo de vídeo diretamente (zero CPU) e assegura áudio em AAC (compatível com navegadores)
        cmd.args(["-c:v", "copy", "-c:a", "aac", "-b:a", "192k"]);
    }

    cmd.args([
        "-movflags",
        "frag_keyframe+empty_moov+default_base_moof",
        "-f",
        "mp4",
        "pipe:1",
    ]);

    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::null());
    cmd.kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(_) => return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR),
    };

    let process_stream = ProcessStdoutStream {
        stdout,
        _child: child,
        deadline: None,
        read_timeout: TRANSCODE_STREAM_IDLE_TIMEOUT,
    };

    let stream = idle_timeout_stream(process_stream, stream_permit, TRANSCODE_STREAM_IDLE_TIMEOUT);

    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = StatusCode::OK;
    let response_headers = response.headers_mut();
    response_headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("video/mp4"));
    response_headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("none"));
    response
}

fn error_credentials_required() -> Response {
    (
        StatusCode::FORBIDDEN,
        [(header::CACHE_CONTROL, "private, no-store")],
        Json(serde_json::json!({"error": "credentials_change_required"})),
    )
        .into_response()
}

fn media_error_response(error: &learnvault_core::Error) -> Response {
    error_private_no_store(media_error_status(error))
}

fn error_private_no_store(status: StatusCode) -> Response {
    (
        status,
        [(header::CACHE_CONTROL, "private, no-store")],
        Body::empty(),
    )
        .into_response()
}

fn media_error_status(error: &learnvault_core::Error) -> StatusCode {
    use learnvault_core::Error;

    match error {
        Error::MediaNotFound { .. }
        | Error::MediaUnavailable { .. }
        | Error::MediaMissingOnDisk { .. }
        | Error::MediaRemoved { .. } => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
