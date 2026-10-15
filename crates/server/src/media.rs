//! Entrega autenticada e somente leitura de mídia.

use std::io::SeekFrom;

use axum::{
    body::Body,
    extract::{RawQuery, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token},
    catalog::service_unavailable,
};

#[derive(Debug, Deserialize)]
pub(crate) struct MediaQuery {
    path: String,
}

pub(crate) async fn media(
    State(state): State<AppState>,
    raw_query: RawQuery,
    headers: HeaderMap,
    method: Method,
) -> Response {
    let Some(token) = request_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    match state.session_user(hash_session_token(&token)).await {
        Ok(Some(_)) => {}
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    }
    let query = match raw_query.0.as_deref() {
        Some(raw) => match serde_urlencoded::from_str::<MediaQuery>(raw) {
            Ok(query) => query,
            Err(_) => return StatusCode::BAD_REQUEST.into_response(),
        },
        None => return StatusCode::BAD_REQUEST.into_response(),
    };

    let root = state.config.library_root.clone();
    let path = query.path;
    let lookup_path = path.clone();
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let database = state.database.clone().lock_owned().await;
    let opened = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let Some(reference) = learnvault_core::find_media_by_path(&database, &lookup_path)? else {
            return Ok(None);
        };
        learnvault_core::open_media(&database, &reference.library_id, &reference.media_id, &root)
            .map(Some)
    })
    .await;
    let opened = match opened {
        Ok(Ok(Some(opened))) => opened,
        Ok(Ok(None)) => return StatusCode::NOT_FOUND.into_response(),
        Ok(Err(error)) => return media_error_response(&error),
        // A tarefa abortada é uma indisponibilidade operacional, não ausência de mídia.
        Err(_) => return service_unavailable(),
    };

    let Some((start, end)) = parse_range(headers.get(header::RANGE), opened.size) else {
        return range_not_satisfiable(opened.size);
    };
    let partial = headers.get(header::RANGE).is_some();
    let length = if opened.size == 0 { 0 } else { end - start + 1 };
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

fn media_error_response(error: &learnvault_core::Error) -> Response {
    media_error_status(error).into_response()
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

fn parse_range(value: Option<&HeaderValue>, size: u64) -> Option<(u64, u64)> {
    let Some(value) = value else {
        return Some((0, size.saturating_sub(1)));
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

#[cfg(test)]
mod tests {
    use super::{media_error_status, parse_range};
    use axum::http::HeaderValue;
    use learnvault_core::Error;

    #[test]
    fn empty_files_have_a_valid_full_response_but_ranges_do_not() {
        assert_eq!(parse_range(None, 0), Some((0, 0)));
        assert_eq!(
            parse_range(Some(&HeaderValue::from_static("bytes=0-")), 0),
            None
        );
    }

    #[test]
    fn media_errors_map_only_legitimate_absence_to_not_found() {
        for error in [
            Error::MediaNotFound {
                media_id: "media-1".to_owned(),
            },
            Error::MediaUnavailable {
                media_id: "media-1".to_owned(),
            },
            Error::MediaMissingOnDisk {
                media_id: "media-1".to_owned(),
            },
            Error::MediaRemoved {
                media_id: "media-1".to_owned(),
            },
        ] {
            assert_eq!(
                media_error_status(&error),
                axum::http::StatusCode::NOT_FOUND
            );
        }
    }

    #[test]
    fn media_integrity_and_operational_errors_map_to_internal_server_error() {
        for error in [
            Error::InvalidRelativePath {
                path: "../outside".to_owned(),
            },
            Error::MediaOutsideLibrary {
                media_id: "media-1".to_owned(),
            },
            Error::MediaIsSymlink {
                media_id: "media-1".to_owned(),
            },
            Error::MediaNotAFile {
                media_id: "media-1".to_owned(),
            },
            Error::MediaIdentityMismatch {
                media_id: "media-1".to_owned(),
            },
            Error::MediaUnreadable {
                media_id: "media-1".to_owned(),
                source: std::io::Error::other("unreadable"),
            },
            Error::LibraryRootMissing {
                path: "missing-library".into(),
            },
        ] {
            assert_eq!(
                media_error_status(&error),
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            );
        }
    }
}
