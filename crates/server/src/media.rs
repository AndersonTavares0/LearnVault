//! Entrega autenticada e somente leitura de mídia.

use std::{io::SeekFrom, time::Duration};

use axum::{
    body::Body,
    extract::{RawQuery, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token},
    catalog::service_unavailable,
    stream::idle_timeout_stream,
};

#[derive(Debug, Deserialize)]
pub(crate) struct MediaQuery {
    path: String,
}

const MEDIA_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn media(
    State(state): State<AppState>,
    raw_query: RawQuery,
    headers: HeaderMap,
    method: Method,
) -> Response {
    let Some(token) = request_token(&headers) else {
        return error_private_no_store(StatusCode::UNAUTHORIZED);
    };
    match state.session_user(hash_session_token(&token)).await {
        Ok(Some(_)) => {}
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return error_private_no_store(StatusCode::UNAUTHORIZED),
    }
    let query = match raw_query.0.as_deref() {
        Some(raw) => match serde_urlencoded::from_str::<MediaQuery>(raw) {
            Ok(query) => query,
            Err(_) => return error_private_no_store(StatusCode::BAD_REQUEST),
        },
        None => return error_private_no_store(StatusCode::BAD_REQUEST),
    };

    let root = state.config.library_root.clone();
    let path = query.path;
    let lookup_path = path.clone();
    // Evita segurar o mutex do banco só para devolver 503 sob saturação.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let reference = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        learnvault_core::find_media_by_path(&database, &lookup_path)
    })
    .await;
    let reference = match reference {
        Ok(Ok(Some(reference))) => reference,
        Ok(Ok(None)) => return error_private_no_store(StatusCode::NOT_FOUND),
        Ok(Err(error)) => return media_error_response(&error),
        // A tarefa abortada é uma indisponibilidade operacional, não ausência de mídia.
        Err(_) => return service_unavailable(),
    };
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let opened = match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        learnvault_core::open_media_reference(&reference, &root)
    })
    .await
    {
        Ok(Ok(opened)) => opened,
        Ok(Err(error)) => return media_error_response(&error),
        Err(_) => return service_unavailable(),
    };

    let Some((start, end)) = parse_range(headers.get(header::RANGE), opened.size) else {
        return range_not_satisfiable(opened.size);
    };
    let partial = headers.get(header::RANGE).is_some();
    let length = if opened.size == 0 { 0 } else { end - start + 1 };
    let stream_permit = if method == Method::HEAD {
        None
    } else {
        match state.try_acquire_media_stream() {
            Some(permit) => Some(permit),
            None => return service_unavailable(),
        }
    };
    let mut file = tokio::fs::File::from_std(opened.file);
    if file.seek(SeekFrom::Start(start)).await.is_err() {
        return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR);
    }
    let stream = stream_permit.map(|stream_permit| {
        idle_timeout_stream(file.take(length), stream_permit, MEDIA_STREAM_IDLE_TIMEOUT)
    });
    let mut response = Response::new(if method == Method::HEAD {
        Body::empty()
    } else {
        let Some(stream) = stream else {
            // Só é `None` para `HEAD`; um `GET` sem stream indica estado
            // interno inconsistente, nunca ausência de mídia.
            return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR);
        };
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
        let Ok(content_range) =
            HeaderValue::from_str(&format!("bytes {start}-{end}/{}", opened.size))
        else {
            return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR);
        };
        response_headers.insert(header::CONTENT_RANGE, content_range);
    }
    response
}

fn media_error_response(error: &learnvault_core::Error) -> Response {
    error_private_no_store(media_error_status(error))
}

/// Erros da superfície autenticada nunca devem ser armazenados por proxies.
fn error_private_no_store(status: StatusCode) -> Response {
    let mut response = status.into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
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
    let mut response = error_private_no_store(StatusCode::RANGE_NOT_SATISFIABLE);
    if let Ok(content_range) = HeaderValue::from_str(&format!("bytes */{size}")) {
        response
            .headers_mut()
            .insert(header::CONTENT_RANGE, content_range);
    }
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
