//! Entrega autenticada e somente leitura de mídia.

use std::io::SeekFrom;

use axum::{
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::{
    AppState,
    auth::{hash_session_token, request_token},
};

#[derive(Debug, Deserialize)]
pub(crate) struct MediaQuery {
    path: String,
}

pub(crate) async fn media(
    State(state): State<AppState>,
    Query(query): Query<MediaQuery>,
    headers: HeaderMap,
    method: Method,
) -> Response {
    let Some(token) = request_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    if state
        .session_user(hash_session_token(&token))
        .await
        .ok()
        .flatten()
        .is_none()
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }

    let root = state.config.library_root.clone();
    let path = query.path;
    let lookup_path = path.clone();
    let database = state.database.clone();
    let opened = tokio::task::spawn_blocking(move || {
        let database = database.blocking_lock();
        let reference = learnvault_core::find_media_by_path(&database, &lookup_path)
            .ok()
            .flatten()?;
        learnvault_core::open_media(&database, &reference.library_id, &reference.media_id, &root)
            .ok()
    })
    .await;
    let Ok(Some(opened)) = opened else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let Some((start, end)) = parse_range(headers.get(header::RANGE), opened.size) else {
        return range_not_satisfiable(opened.size);
    };
    let partial = headers.get(header::RANGE).is_some();
    let length = end - start + 1;
    let mut file = tokio::fs::File::from_std(opened.file);
    if file.seek(SeekFrom::Start(start)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let stream = ReaderStream::new(file.take(length));
    let mut response = Response::new(if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(stream)
    });
    *response.status_mut() = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let response_headers = response.headers_mut();
    response_headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    response_headers.insert(header::CONTENT_LENGTH, HeaderValue::from(length));
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(&path)),
    );
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if partial {
        response_headers.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{}", opened.size)).unwrap(),
        );
    }
    response
}

fn parse_range(value: Option<&HeaderValue>, size: u64) -> Option<(u64, u64)> {
    let Some(value) = value else {
        return (size > 0).then_some((0, size - 1));
    };
    let text = value.to_str().ok()?;
    let spec = text.strip_prefix("bytes=")?;
    if spec.contains(',') || size == 0 {
        return None;
    }
    let (start, end) = spec.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?;
        if suffix == 0 {
            return None;
        }
        let length = suffix.min(size);
        return Some((size - length, size - 1));
    }
    let start = start.parse::<u64>().ok()?;
    if start >= size {
        return None;
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().ok()?.min(size - 1)
    };
    (start <= end).then_some((start, end))
}

fn range_not_satisfiable(size: u64) -> Response {
    let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    response.headers_mut().insert(
        header::CONTENT_RANGE,
        HeaderValue::from_str(&format!("bytes */{size}")).unwrap(),
    );
    response
}

fn content_type(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "m3u8" => "application/vnd.apple.mpegurl",
        "ts" => "video/mp2t",
        "vtt" => "text/vtt",
        "srt" => "application/x-subrip",
        _ => "application/octet-stream",
    }
}
