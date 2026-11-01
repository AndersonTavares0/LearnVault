//! Avatares personalizados de usuários: leitura, upload e remoção por dono autenticado.
//!
//! Cada usuário autenticado pode gravar um único arquivo de avatar, armazenado no diretório
//! de dados do servidor em `<diretório de dados>/avatars/<user_id>.avatar`, sem tocar a
//! biblioteca de cursos (somente-leitura). O formato é validado estritamente por magic bytes
//! (PNG, JPEG ou WebP) e limitado a 2 MiB ([`MAX_AVATAR_BYTES`]). Nenhum caminho interno é
//! exposto na API.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use tokio_stream::StreamExt as _;

use crate::{
    AppState, StateError,
    auth::same_origin,
    catalog::{
        authenticated_user, credentials_required, csrf_failed, internal_error, private_no_store,
        service_unavailable, status_private_no_store,
    },
    courses::replace_file,
    covers::{ImageType, detect_image_type, modified_seconds},
};

fn json_error(status: StatusCode, error: &'static str) -> Response {
    private_no_store((status, Json(serde_json::json!({ "error": error }))).into_response())
}

/// Tamanho máximo aceito para o avatar de um usuário (2 MiB).
pub(crate) const MAX_AVATAR_BYTES: usize = 2 * 1024 * 1024;

/// Nome do diretório de avatares dentro da pasta de dados do servidor.
pub(crate) const AVATARS_DIR_NAME: &str = "avatars";

/// Sufixo do arquivo de avatar dentro do diretório de avatares.
const AVATAR_FILE_SUFFIX: &str = ".avatar";

/// Comprimento máximo da identidade de usuário aceita para caminhos seguros.
const MAX_USER_ID_LEN: usize = 128;

/// Falha nas operações de persistência do avatar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AvatarError {
    Rejected,
    WriteFailed,
    Unreadable,
}

#[derive(Serialize)]
pub(crate) struct AvatarResponse {
    pub(crate) version: i64,
}

/// Caminho do diretório de avatares, ao lado do banco de dados.
pub(crate) fn avatars_dir(database_path: &Path) -> PathBuf {
    let parent = database_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    parent.join(AVATARS_DIR_NAME)
}

/// Valida se o identificador do usuário é seguro como segmento de arquivo.
pub(crate) fn valid_user_id(user_id: &str) -> bool {
    if user_id.is_empty() || user_id.len() > MAX_USER_ID_LEN {
        return false;
    }
    user_id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn avatar_file(avatars_dir: &Path, user_id: &str) -> PathBuf {
    avatars_dir.join(format!("{user_id}{AVATAR_FILE_SUFFIX}"))
}

/// Grava o avatar de forma atômica no disco e devolve a versão resultante (mtime em segundos).
pub(crate) fn store_avatar(
    avatars_dir: &Path,
    user_id: &str,
    bytes: &[u8],
) -> Result<i64, AvatarError> {
    if !valid_user_id(user_id)
        || bytes.len() > MAX_AVATAR_BYTES
        || detect_image_type(bytes).is_none()
    {
        return Err(AvatarError::Rejected);
    }
    fs::create_dir_all(avatars_dir).map_err(|_| AvatarError::WriteFailed)?;

    let destination = avatar_file(avatars_dir, user_id);
    static ATOMIC_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let counter = ATOMIC_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let temporary = avatars_dir.join(format!(".{user_id}.{}.{counter}.tmp", std::process::id()));

    let write_result = (|| -> std::io::Result<i64> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);

        replace_file(&temporary, &destination)?;
        let metadata = fs::symlink_metadata(&destination)?;
        modified_seconds(&metadata)
            .ok_or_else(|| std::io::Error::other("tempo de modificação inválido"))
    })();

    match write_result {
        Ok(version) => Ok(version),
        Err(_) => {
            let _ = fs::remove_file(&temporary);
            Err(AvatarError::WriteFailed)
        }
    }
}

/// Lê o avatar de um usuário do disco, retornando bytes, tipo detectado e mtime.
pub(crate) fn read_avatar(
    avatars_dir: &Path,
    user_id: &str,
) -> Result<Option<(Vec<u8>, ImageType, i64)>, AvatarError> {
    if !valid_user_id(user_id) {
        return Ok(None);
    }
    let path = avatar_file(avatars_dir, user_id);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(AvatarError::Unreadable),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    let mtime = modified_seconds(&metadata).ok_or(AvatarError::Unreadable)?;
    let bytes = fs::read(&path).map_err(|_| AvatarError::Unreadable)?;
    let Some(image_type) = detect_image_type(&bytes) else {
        return Err(AvatarError::Unreadable);
    };
    Ok(Some((bytes, image_type, mtime)))
}

/// Remove o avatar de um usuário do disco. Retorna se o arquivo existia.
pub(crate) fn remove_avatar(avatars_dir: &Path, user_id: &str) -> Result<bool, AvatarError> {
    if !valid_user_id(user_id) {
        return Ok(false);
    }
    let path = avatar_file(avatars_dir, user_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(AvatarError::WriteFailed),
    }
}

enum AvatarBodyError {
    TooLarge,
    Unreadable,
}

async fn read_avatar_body(body: Body) -> Result<Vec<u8>, AvatarBodyError> {
    let mut stream = body.into_data_stream();
    let mut bytes = Vec::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|_| AvatarBodyError::Unreadable)?;
        if bytes.len().saturating_add(chunk.len()) > MAX_AVATAR_BYTES {
            return Err(AvatarBodyError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// `GET /api/user/avatar` — serve a imagem do avatar do usuário logado.
pub(crate) async fn get_avatar(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => user,
        Ok(Some(_)) => return credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let avatars = avatars_dir(&state.config.database_path);
    let user_id = user.user_id;

    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        read_avatar(&avatars, &user_id)
    })
    .await;

    match outcome {
        Ok(Ok(Some((bytes, image_type, version)))) => {
            if headers
                .get(header::IF_NONE_MATCH)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|if_none_match| if_none_match.trim_matches('"') == version.to_string())
            {
                return StatusCode::NOT_MODIFIED.into_response();
            }
            let mut response = Body::from(bytes).into_response();
            *response.status_mut() = StatusCode::OK;
            let response_headers = response.headers_mut();
            response_headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(image_type.content_type()),
            );
            response_headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=3600"),
            );
            if let Ok(etag) = HeaderValue::from_str(&format!("\"{version}\"")) {
                response_headers.insert(header::ETAG, etag);
            }
            response
        }
        Ok(Ok(None)) => json_error(StatusCode::NOT_FOUND, "avatar_not_found"),
        Ok(Err(_)) | Err(_) => internal_error("avatar_read_failed"),
    }
}

/// `PUT /api/user/avatar` — envia e atualiza o avatar do usuário logado.
pub(crate) async fn put_avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => user,
        Ok(Some(_)) => return credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_AVATAR_BYTES as u64)
    {
        return json_error(StatusCode::PAYLOAD_TOO_LARGE, "avatar_too_large");
    }

    let bytes = match read_avatar_body(body).await {
        Ok(bytes) => bytes,
        Err(AvatarBodyError::TooLarge) => {
            return json_error(StatusCode::PAYLOAD_TOO_LARGE, "avatar_too_large");
        }
        Err(AvatarBodyError::Unreadable) => {
            return json_error(StatusCode::BAD_REQUEST, "avatar_unreadable");
        }
    };

    if detect_image_type(&bytes).is_none() {
        return json_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_avatar_type",
        );
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let avatars = avatars_dir(&state.config.database_path);
    let user_id = user.user_id;

    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        store_avatar(&avatars, &user_id, &bytes)
    })
    .await;

    match outcome {
        Ok(Ok(version)) => private_no_store(Json(AvatarResponse { version }).into_response()),
        Ok(Err(AvatarError::Rejected)) => json_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_avatar_type",
        ),
        Ok(Err(_)) | Err(_) => internal_error("avatar_write_failed"),
    }
}

/// `DELETE /api/user/avatar` — remove o avatar do usuário logado.
pub(crate) async fn delete_avatar(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match authenticated_user(&state, &headers).await {
        Ok(Some(user)) if !user.must_change_credentials => user,
        Ok(Some(_)) => return credentials_required(),
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return status_private_no_store(StatusCode::UNAUTHORIZED),
    };
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }

    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let avatars = avatars_dir(&state.config.database_path);
    let user_id = user.user_id;

    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        remove_avatar(&avatars, &user_id)
    })
    .await;

    match outcome {
        Ok(Ok(_)) => status_private_no_store(StatusCode::NO_CONTENT),
        Ok(Err(_)) | Err(_) => internal_error("avatar_write_failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_png() -> Vec<u8> {
        let mut bytes = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        bytes.extend_from_slice(&[0; 16]);
        bytes
    }

    fn sample_webp() -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&[4, 0, 0, 0]);
        bytes.extend_from_slice(b"WEBP");
        bytes
    }

    #[test]
    fn user_id_validation_prevents_escape_and_invalid_characters() {
        assert!(valid_user_id("user-1"));
        assert!(valid_user_id("user_abc-123"));
        assert!(valid_user_id("018f26a1-0000-7000-8000-000000000001"));
        assert!(!valid_user_id(""));
        assert!(!valid_user_id(".."));
        assert!(!valid_user_id("../user"));
        assert!(!valid_user_id("user/1"));
        assert!(!valid_user_id("user\\1"));
        assert!(!valid_user_id("user\0"));
        assert!(!valid_user_id(&"a".repeat(129)));
    }

    #[test]
    fn avatar_lifecycle_and_atomic_persistence() {
        let temp = tempfile::tempdir().unwrap();
        let avatars = avatars_dir(&temp.path().join("learnvault.sqlite"));

        // Inicialmente ausente
        assert_eq!(read_avatar(&avatars, "user-1").unwrap(), None);

        // Gravação bem-sucedida
        let png = sample_png();
        let v1 = store_avatar(&avatars, "user-1", &png).unwrap();
        assert!(v1 > 0);

        let (loaded, img_type, v_read) = read_avatar(&avatars, "user-1").unwrap().unwrap();
        assert_eq!(loaded, png);
        assert_eq!(img_type, ImageType::Png);
        assert_eq!(v_read, v1);

        // Atualização com WebP
        let webp = sample_webp();
        store_avatar(&avatars, "user-1", &webp).unwrap();
        let (loaded2, img_type2, _) = read_avatar(&avatars, "user-1").unwrap().unwrap();
        assert_eq!(loaded2, webp);
        assert_eq!(img_type2, ImageType::Webp);

        // Remoção
        assert!(remove_avatar(&avatars, "user-1").unwrap());
        assert!(
            !remove_avatar(&avatars, "user-1").unwrap(),
            "segunda remoção idempotente"
        );
        assert_eq!(read_avatar(&avatars, "user-1").unwrap(), None);
    }

    #[test]
    fn avatar_rejects_corrupted_or_oversized_payload() {
        let temp = tempfile::tempdir().unwrap();
        let avatars = avatars_dir(&temp.path().join("learnvault.sqlite"));

        assert_eq!(
            store_avatar(&avatars, "user-1", b"not-image"),
            Err(AvatarError::Rejected)
        );
        let oversized = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut big = oversized;
        big.resize(MAX_AVATAR_BYTES + 1, 0);
        assert_eq!(
            store_avatar(&avatars, "user-1", &big),
            Err(AvatarError::Rejected)
        );
    }
}
