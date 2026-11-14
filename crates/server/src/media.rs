//! Entrega autenticada e somente leitura de mídia.

use std::{io::SeekFrom, time::Duration};

use axum::{
    body::Body,
    extract::{Json, Path as AxumPath, RawQuery, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_stream::StreamExt;

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token, same_origin},
    catalog::{
        authenticated_user, credentials_required, csrf_failed, internal_error, private_no_store,
        service_unavailable, status_private_no_store,
    },
    sources::{RemoteClient, RemoteSource},
    stream::idle_timeout_stream,
};

#[derive(Debug, Deserialize)]
pub(crate) struct MediaQuery {
    /// Identidade do curso registrado que contém a mídia.
    course: String,
    /// Caminho relativo dentro da pasta do curso.
    path: String,
}

const MEDIA_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MEDIA_OPEN_TIMEOUT: Duration = Duration::from_secs(5);
/// A subtitle is fetched whole by the player before it becomes cues, so it needs a cap.
const MAX_SUBTITLE_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) async fn media(
    State(state): State<AppState>,
    raw_query: RawQuery,
    headers: HeaderMap,
    method: Method,
) -> Response {
    let Some(token) = request_token(&headers) else {
        return error_private_no_store(StatusCode::UNAUTHORIZED);
    };
    let user = match state.session_user(hash_session_token(&token)).await {
        Ok(Some(user)) if !user.must_change_credentials => user,
        Ok(Some(_)) => return error_credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return error_private_no_store(StatusCode::UNAUTHORIZED),
    };
    let query = match raw_query.0.as_deref() {
        Some(raw) => match serde_urlencoded::from_str::<MediaQuery>(raw) {
            Ok(query) => query,
            Err(_) => return error_private_no_store(StatusCode::BAD_REQUEST),
        },
        None => return error_private_no_store(StatusCode::BAD_REQUEST),
    };

    let Some(course) = state
        .courses_snapshot()
        .into_iter()
        .find(|course| course.id == query.course)
    else {
        // A mesma rota serve os cursos locais e as fontes remotas: a identidade opaca do
        // curso decide qual adaptador entrega os bytes.
        if let Some(source) = state
            .sources_snapshot()
            .into_iter()
            .find(|source| source.id == query.course)
        {
            let library_id = source.id.clone();
            return remote_media(state, user, source, library_id, query.path, headers, method)
                .await;
        }
        // A root of a split source is its own course; its bytes come from the source.
        match source_of_root(&state, &query.course).await {
            Ok(Some(source)) => {
                return remote_media(
                    state,
                    user,
                    source,
                    query.course,
                    query.path,
                    headers,
                    method,
                )
                .await;
            }
            Ok(None) => {}
            Err(response) => return *response,
        }
        return error_private_no_store(StatusCode::NOT_FOUND);
    };
    let root = course.root;
    let path = query.path;
    let lookup_path = path.clone();
    let library_id = course.id.clone();
    // Evita segurar recursos só para devolver 503 sob saturação.
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    // A validação de registro no banco ocorre em escopo restrito e libera o lock do
    // SQLite imediatamente, antes de qualquer operação no sistema de arquivos.
    let reference = {
        let database = state.database.clone().lock_owned().await;
        let Some(blocking_task) = state.try_acquire_blocking_task() else {
            return service_unavailable();
        };
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            if !learnvault_core::can_user_access_course(
                &database,
                &user.user_id,
                user.is_admin,
                &library_id,
            )? {
                return Ok(None);
            }
            learnvault_core::find_media_by_path(&database, &library_id, &lookup_path)
        })
        .await
    };
    let reference = match reference {
        Ok(Ok(Some(reference))) => reference,
        Ok(Ok(None)) => return error_private_no_store(StatusCode::NOT_FOUND),
        Ok(Err(error)) => return media_error_response(&error),
        // A tarefa abortada é uma indisponibilidade operacional, não ausência de mídia.
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

    // Abertura do arquivo físico FORA do lock do banco, com timeout delimitado para
    // que discos lentos ou placeholders de nuvem não bloqueiem o processo indefinidamente.
    let opened = match tokio::time::timeout(
        MEDIA_OPEN_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            learnvault_core::open_media_file(&reference, root.as_path())
        }),
    )
    .await
    {
        Ok(Ok(Ok(opened))) => opened,
        Ok(Ok(Err(error))) => return media_error_response(&error),
        Ok(Err(_join_err)) => return service_unavailable(),
        Err(_timeout_err) => return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if opened.kind == learnvault_core::MediaKind::Subtitle && opened.size > MAX_SUBTITLE_BYTES {
        return error_private_no_store(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let Some((start, end)) = parse_range(headers.get(header::RANGE), opened.size) else {
        return range_not_satisfiable(opened.size);
    };
    let partial = headers.get(header::RANGE).is_some();
    let length = if opened.size == 0 { 0 } else { end - start + 1 };
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
    if let Some(disposition) = content_disposition(&path) {
        response_headers.insert(header::CONTENT_DISPOSITION, disposition);
    }
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

/// The configured source that owns `course` when it is a root, or `None`. The media lookup
/// that follows only knows the files of that root, so a path outside its folder is not found.
async fn source_of_root(
    state: &AppState,
    course: &str,
) -> Result<Option<RemoteSource>, Box<Response>> {
    if !state.has_blocking_capacity() {
        return Err(Box::new(service_unavailable()));
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return Err(Box::new(service_unavailable()));
    };
    let course = course.to_owned();
    let owner = match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        learnvault_core::root_source(&database, &course)
    })
    .await
    {
        Ok(Ok(owner)) => owner,
        _ => return Err(Box::new(service_unavailable())),
    };
    Ok(owner.and_then(|owner| {
        state
            .sources_snapshot()
            .into_iter()
            .find(|source| source.id == owner)
    }))
}

/// Entrega uma faixa de uma mídia de uma fonte WebDAV.
///
/// Só caminhos já catalogados no banco são repassados ao servidor remoto: o cliente HTTP
/// nunca escolhe livremente um caminho no WebDAV. O corpo é transmitido em streaming, sem
/// buffer do arquivo inteiro.
async fn remote_media(
    state: AppState,
    user: learnvault_core::AuthSession,
    source: RemoteSource,
    library_id: String,
    path: String,
    headers: HeaderMap,
    method: Method,
) -> Response {
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let lookup_path = path;
    let lookup = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        if !learnvault_core::can_user_access_course(
            &database,
            &user.user_id,
            user.is_admin,
            &library_id,
        )? {
            return Ok(None);
        }
        learnvault_core::find_media_by_path(&database, &library_id, &lookup_path)
    })
    .await;
    let reference = match lookup {
        Ok(Ok(Some(reference))) => reference,
        Ok(Ok(None)) => return error_private_no_store(StatusCode::NOT_FOUND),
        Ok(Err(error)) => return media_error_response(&error),
        Err(_) => return service_unavailable(),
    };

    let size = reference.size();
    let Some((start, end)) = parse_range(headers.get(header::RANGE), size) else {
        return range_not_satisfiable(size);
    };
    let partial = headers.get(header::RANGE).is_some();
    if size == 0 {
        return empty_remote_response(&reference.relative_path, partial, size);
    }
    let length = end - start + 1;

    let client = match RemoteClient::new(&source) {
        Ok(client) => client,
        Err(_) => return remote_media_error(StatusCode::BAD_GATEWAY, "source_unavailable"),
    };

    let permit = if method == Method::HEAD {
        None
    } else {
        match state.try_acquire_media_stream() {
            Some(permit) => Some(permit),
            None => return service_unavailable(),
        }
    };
    let body = match client {
        RemoteClient::WebDav(dav) => {
            let upstream = match dav.open_range(&reference.relative_path, start, end).await {
                Ok(upstream) => upstream,
                Err(learnvault_core::SourceError::NotFound) => {
                    return error_private_no_store(StatusCode::NOT_FOUND);
                }
                Err(learnvault_core::SourceError::Denied) => {
                    return remote_media_error(StatusCode::BAD_GATEWAY, "source_denied");
                }
                Err(_) => {
                    return remote_media_error(StatusCode::BAD_GATEWAY, "source_unavailable");
                }
            };
            if upstream
                .content_length()
                .is_some_and(|upstream_length| upstream_length != length)
            {
                return remote_media_error(StatusCode::BAD_GATEWAY, "source_range_mismatch");
            }
            match permit {
                Some(permit) => Body::from_stream(upstream.bytes_stream().map(move |chunk| {
                    // O permit vive enquanto o corpo for consumido e é liberado ao final.
                    let _ = &permit;
                    chunk.map_err(std::io::Error::other)
                })),
                None => Body::empty(),
            }
        }
        RemoteClient::Smb(smb) => {
            let upstream = match smb
                .open_stream(&reference.relative_path, start, length)
                .await
            {
                Ok(upstream) => upstream,
                Err(learnvault_core::SourceError::NotFound) => {
                    return error_private_no_store(StatusCode::NOT_FOUND);
                }
                Err(learnvault_core::SourceError::Denied) => {
                    return remote_media_error(StatusCode::BAD_GATEWAY, "source_denied");
                }
                Err(_) => {
                    return remote_media_error(StatusCode::BAD_GATEWAY, "source_unavailable");
                }
            };
            match permit {
                Some(permit) => Body::from_stream(upstream.map(move |chunk| {
                    let _ = &permit;
                    chunk
                })),
                None => Body::empty(),
            }
        }
    };
    let mut response = Response::new(body);
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
        HeaderValue::from_static(content_type(&reference.relative_path)),
    );
    if let Some(disposition) = content_disposition(&reference.relative_path) {
        response_headers.insert(header::CONTENT_DISPOSITION, disposition);
    }
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if partial {
        let Ok(content_range) = HeaderValue::from_str(&format!("bytes {start}-{end}/{size}"))
        else {
            return error_private_no_store(StatusCode::INTERNAL_SERVER_ERROR);
        };
        response_headers.insert(header::CONTENT_RANGE, content_range);
    }
    response
}

fn empty_remote_response(path: &str, partial: bool, size: u64) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = if partial {
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };
    let headers = response.headers_mut();
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    headers.insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type(path)),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if partial && let Ok(content_range) = HeaderValue::from_str(&format!("bytes 0-0/{size}")) {
        headers.insert(header::CONTENT_RANGE, content_range);
    }
    response
}

fn remote_media_error(status: StatusCode, error: &'static str) -> Response {
    error_response_private_no_store(
        (status, axum::Json(serde_json::json!({ "error": error }))).into_response(),
    )
}

fn error_credentials_required() -> Response {
    error_response_private_no_store(
        (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({"error":"credentials_change_required"})),
        )
            .into_response(),
    )
}

fn media_error_response(error: &learnvault_core::Error) -> Response {
    error_private_no_store(media_error_status(error))
}

/// Erros da superfície autenticada nunca devem ser armazenados por proxies.
fn error_private_no_store(status: StatusCode) -> Response {
    error_response_private_no_store(status.into_response())
}

fn error_response_private_no_store(mut response: Response) -> Response {
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
    let extension = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    playable_media_type(&extension).unwrap_or("application/octet-stream")
}

fn playable_media_type(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mkv" => "video/x-matroska",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "ts" => "video/mp2t",
        "m3u8" => "application/vnd.apple.mpegurl",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "opus" => "audio/opus",
        "weba" => "audio/webm",
        "wma" => "audio/x-ms-wma",
        "vtt" => "text/vtt",
        "srt" => "application/x-subrip",
        "ass" => "text/x-ass",
        "ssa" => "text/x-ssa",
        "sub" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

fn content_disposition(path: &str) -> Option<HeaderValue> {
    let extension = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    if playable_media_type(&extension).is_some() {
        return None;
    }

    let basename = path.rsplit(['/', '\\']).next().unwrap_or_default();
    let filename: String = basename
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let filename = if filename.is_empty() || filename == "." || filename == ".." {
        "download"
    } else {
        &filename
    };
    HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).ok()
}

#[derive(Serialize)]
pub(crate) struct MediaImpactResponse {
    pub(crate) media_id: String,
    pub(crate) relative_path: String,
    pub(crate) availability: &'static str,
    pub(crate) progress_owners: usize,
    pub(crate) already_removed: bool,
}

#[derive(Deserialize)]
pub(crate) struct RemoveMediaRequest {
    #[serde(default)]
    pub(crate) confirm_progress: bool,
}

#[derive(Serialize)]
pub(crate) struct RemoveMediaOutcomeResponse {
    pub(crate) media_id: String,
    pub(crate) removed_now: bool,
    pub(crate) preserved_owners: usize,
}

#[derive(Serialize)]
struct RemovalNeedsConfirmationResponse {
    error: &'static str,
    media_id: String,
    progress_owners: usize,
}

/// `GET /api/admin/media/{id}/impact` — consulta o impacto da remoção de uma mídia.
pub(crate) async fn impact(
    State(state): State<AppState>,
    AxumPath(media_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !user.is_admin {
        return status_private_no_store(StatusCode::FORBIDDEN);
    }
    if user.must_change_credentials {
        return credentials_required();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let lookup_id = media_id.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let library_id = learnvault_core::find_media_library_id(&database, &lookup_id)?;
        let Some(library_id) = library_id else {
            return Err(learnvault_core::Error::MediaNotFound {
                media_id: lookup_id,
            });
        };
        learnvault_core::media_removal_impact(&database, &library_id, &lookup_id)
    })
    .await;

    match outcome {
        Ok(Ok(impact)) => {
            let availability_str = match impact.availability {
                learnvault_core::Availability::Available => "available",
                learnvault_core::Availability::Missing => "missing",
            };
            private_no_store(
                Json(MediaImpactResponse {
                    media_id: impact.media_id,
                    relative_path: impact.relative_path,
                    availability: availability_str,
                    progress_owners: impact.progress_owners,
                    already_removed: impact.already_removed,
                })
                .into_response(),
            )
        }
        Ok(Err(learnvault_core::Error::MediaNotFound { .. })) => {
            status_private_no_store(StatusCode::NOT_FOUND)
        }
        Ok(Err(_)) | Err(_) => internal_error("media_impact_unavailable"),
    }
}

/// `DELETE /api/admin/media/{id}` — remove uma mídia do catálogo (tombstone persistente).
pub(crate) async fn remove_media(
    State(state): State<AppState>,
    AxumPath(media_id): AxumPath<String>,
    headers: HeaderMap,
    Json(request): Json<RemoveMediaRequest>,
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
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let mut database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let lookup_id = media_id.clone();
    let confirm_progress = request.confirm_progress;
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let library_id = learnvault_core::find_media_library_id(&database, &lookup_id)?;
        let Some(library_id) = library_id else {
            return Err(learnvault_core::Error::MediaNotFound {
                media_id: lookup_id,
            });
        };
        learnvault_core::remove_media(&mut database, &library_id, &lookup_id, confirm_progress)
    })
    .await;

    match outcome {
        Ok(Ok(outcome)) => private_no_store(
            Json(RemoveMediaOutcomeResponse {
                media_id: outcome.media_id,
                removed_now: outcome.removed_now,
                preserved_owners: outcome.preserved_owners,
            })
            .into_response(),
        ),
        Ok(Err(learnvault_core::Error::MediaRemovalNeedsConfirmation {
            media_id,
            progress_owners,
        })) => private_no_store(
            (
                StatusCode::CONFLICT,
                Json(RemovalNeedsConfirmationResponse {
                    error: "media_removal_needs_confirmation",
                    media_id,
                    progress_owners,
                }),
            )
                .into_response(),
        ),
        Ok(Err(learnvault_core::Error::MediaNotFound { .. })) => {
            status_private_no_store(StatusCode::NOT_FOUND)
        }
        Ok(Err(_)) | Err(_) => internal_error("media_removal_unavailable"),
    }
}

#[cfg(test)]
mod tests {
    use super::{content_disposition, content_type, media_error_status, parse_range};
    use axum::http::HeaderValue;
    use learnvault_core::Error;

    #[test]
    fn go_playable_media_types_keep_their_mime_type_and_are_inline() {
        for (path, expected_type) in [
            ("lesson.mp4", "video/mp4"),
            ("lesson.m4v", "video/mp4"),
            ("lesson.mov", "video/quicktime"),
            ("lesson.webm", "video/webm"),
            ("lesson.ogv", "video/ogg"),
            ("lesson.mp3", "audio/mpeg"),
            ("lesson.m4a", "audio/mp4"),
            ("lesson.ogg", "audio/ogg"),
            ("lesson.oga", "audio/ogg"),
            ("lesson.wav", "audio/wav"),
            ("lesson.flac", "audio/flac"),
            ("subtitles.vtt", "text/vtt"),
            ("subtitles.srt", "application/x-subrip"),
            ("subtitles.ass", "text/x-ass"),
            ("subtitles.ssa", "text/x-ssa"),
            ("subtitles.sub", "text/plain; charset=utf-8"),
        ] {
            assert_eq!(content_type(path), expected_type, "{path}");
            assert_eq!(content_disposition(path), None, "{path}");
        }
    }

    #[test]
    fn non_playable_extensions_use_octet_stream_and_safe_attachment_basenames() {
        for path in ["lesson.html", "lesson.svg", "slides.pdf", "archive.zip"] {
            assert_eq!(content_type(path), "application/octet-stream", "{path}");
            assert!(content_disposition(path).is_some(), "{path}");
        }

        for (path, expected_type) in [
            ("lesson.mkv", "video/x-matroska"),
            ("lesson.avi", "video/x-msvideo"),
            ("lesson.wmv", "video/x-ms-wmv"),
            ("lesson.flv", "video/x-flv"),
            ("lesson.ts", "video/mp2t"),
            ("lesson.m3u8", "application/vnd.apple.mpegurl"),
        ] {
            assert_eq!(content_type(path), expected_type, "{path}");
            assert!(content_disposition(path).is_none(), "{path}");
        }

        for (path, expected) in [
            (
                "lessons/unsafe\"name.html",
                "attachment; filename=\"unsafe_name.html\"",
            ),
            (
                "lessons\\unsafe name.svg",
                "attachment; filename=\"unsafe_name.svg\"",
            ),
        ] {
            assert_eq!(
                content_disposition(path).unwrap().to_str().unwrap(),
                expected
            );
        }
    }

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
            // Corrupção de catálogo nunca pode virar 404: isso confundiria ausência
            // legítima com um banco corrompido e criaria um oráculo de existência.
            Error::MediaCorruptSize {
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
