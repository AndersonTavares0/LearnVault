//! Primitivas de autenticação do servidor.

use crate::{AppState, StateError, catalog::service_unavailable};
use argon2::{
    Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version,
    password_hash::SaltString,
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const ARGON_MEMORY_KIB: u32 = 64 * 1024;
const ARGON_ITERATIONS: u32 = 2;
const ARGON_LANES: u32 = 1;
const ARGON_OUTPUT_BYTES: usize = 32;
const SESSION_TOKEN_BYTES: usize = 32;

/// Erros internos das primitivas de autenticação.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    /// O nome de usuário não atende aos limites do produto.
    #[error("invalid username")]
    InvalidUsername,
    /// A senha inicial não atende aos limites do produto.
    #[error("invalid password")]
    InvalidPassword,
    /// O hashing Argon2id falhou.
    #[error("password hashing failed")]
    PasswordHashing,
    /// A geração criptograficamente segura do token falhou.
    #[error("secure session token generation failed")]
    TokenGeneration,
}

/// Gera um hash PHC Argon2id compatível com os dados legados do Go.
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    validate_password(password)?;
    let params = Params::new(
        ARGON_MEMORY_KIB,
        ARGON_ITERATIONS,
        ARGON_LANES,
        Some(ARGON_OUTPUT_BYTES),
    )
    .map_err(|_| AuthError::PasswordHashing)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut rng = OsRng;
    let salt = SaltString::generate(&mut rng);
    argon
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHashing)
}

/// Verifica uma senha sem diferenciar senha errada de hash persistido inválido.
pub fn verify_password(password: &str, encoded_hash: &str) -> Result<bool, AuthError> {
    validate_password(password)?;
    let Ok(parsed) = PasswordHash::new(encoded_hash) else {
        return Ok(false);
    };
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Valida o limite público do nome de usuário sem revelar dados da configuração.
pub fn validate_username(username: &str) -> Result<(), AuthError> {
    let length = username.chars().count();
    if !(2..=64).contains(&length)
        || username.trim() != username
        || username.chars().any(char::is_control)
    {
        return Err(AuthError::InvalidUsername);
    }
    Ok(())
}

/// Valida o limite público da senha inicial ou de login.
pub fn validate_password(password: &str) -> Result<(), AuthError> {
    let length = password.chars().count();
    if !(12..=1024).contains(&length) {
        return Err(AuthError::InvalidPassword);
    }
    Ok(())
}

/// Gera um token de sessão opaco, codificado em hexadecimal.
pub fn generate_session_token() -> Result<String, AuthError> {
    let mut token = [0u8; SESSION_TOKEN_BYTES];
    getrandom::fill(&mut token).map_err(|_| AuthError::TokenGeneration)?;
    Ok(hex_encode(&token))
}

/// Calcula o hash SHA-256 que será persistido, nunca o token em claro.
pub fn hash_session_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    hex_encode(&digest)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

const COOKIE_NAME: &str = "learnvault_session";

#[derive(Debug, Deserialize)]
pub(crate) struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    user_id: String,
    username: String,
    is_admin: bool,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

pub(crate) async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> impl IntoResponse {
    if !state.allow_login_attempt(&request.username).await {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse {
                error: "try again later",
            }),
        )
            .into_response();
    }
    if !same_origin(&headers)
        || validate_username(&request.username).is_err()
        || validate_password(&request.password).is_err()
    {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "invalid credentials",
            }),
        )
            .into_response();
    }
    let user = match state.user(request.username).await {
        Ok(user) => user,
        Err(StateError::Capacity) => return service_unavailable(),
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "invalid credentials",
                }),
            )
                .into_response();
        }
    };
    let Some(user) = user else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "invalid credentials",
            }),
        )
            .into_response();
    };
    let password = request.password;
    let hash = user.password_hash.clone();
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let valid = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        verify_password(&password, &hash).unwrap_or(false)
    })
    .await
    .unwrap_or(false);
    if !valid {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "invalid credentials",
            }),
        )
            .into_response();
    }
    let Ok(token) = generate_session_token() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    match state
        .create_session(hash_session_token(&token), user.user_id.clone())
        .await
    {
        Ok(()) => {}
        Err(StateError::Capacity) => return service_unavailable(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let mut response = Json(UserResponse {
        user_id: user.user_id,
        username: user.username,
        is_admin: user.is_admin,
    })
    .into_response();
    response
        .headers_mut()
        .insert("set-cookie", session_cookie(&state, &token, false));
    response
}

pub(crate) async fn me(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let Some(token) = request_token(&headers) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let user = match state.session_user(hash_session_token(&token)).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return service_unavailable(),
        Ok(None) | Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let mut response = Json(UserResponse {
        user_id: user.user_id,
        username: user.username,
        is_admin: user.is_admin,
    })
    .into_response();
    response.headers_mut().insert(
        "cache-control",
        HeaderValue::from_static("private, no-store"),
    );
    response
}

pub(crate) async fn logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(token) = request_token(&headers)
        && matches!(
            state.delete_session(hash_session_token(&token)).await,
            Err(StateError::Capacity)
        )
    {
        return service_unavailable();
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert("set-cookie", session_cookie(&state, "", true));
    response
}

pub(crate) fn request_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == COOKIE_NAME && !value.is_empty()).then(|| value.to_owned())
        })
}

fn session_cookie(state: &AppState, token: &str, clear: bool) -> HeaderValue {
    let mut value = format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        if clear { 0 } else { 43_200 }
    );
    if state.config.cookie_secure {
        value.push_str("; Secure");
    }
    HeaderValue::from_str(&value).expect("cookie attributes are valid ASCII")
}

pub(crate) fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get("origin").and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let Some(host) = headers.get("host").and_then(|value| value.to_str().ok()) else {
        return origin == "http://localhost"
            || origin == "https://localhost"
            || origin
                .strip_prefix("http://localhost:")
                .is_some_and(|port| port.parse::<u16>().is_ok())
            || origin
                .strip_prefix("https://localhost:")
                .is_some_and(|port| port.parse::<u16>().is_ok());
    };
    origin == format!("http://{host}") || origin == format!("https://{host}")
}

#[cfg(test)]
mod tests {
    use argon2::{Argon2, PasswordHash, password_hash::PasswordVerifier};
    use base64::{Engine, engine::general_purpose::STANDARD_NO_PAD};

    use super::{
        generate_session_token, hash_password, hash_session_token, validate_password,
        validate_username, verify_password,
    };

    #[test]
    fn password_hash_uses_the_legacy_compatible_argon2id_parameters() {
        let encoded = hash_password("uma senha local muito segura").unwrap();
        let parsed = PasswordHash::new(&encoded).unwrap();

        assert_eq!(parsed.algorithm.as_str(), "argon2id");
        assert_eq!(parsed.params.get_decimal("m").unwrap(), 65_536);
        assert_eq!(parsed.params.get_decimal("t").unwrap(), 2);
        assert_eq!(parsed.params.get_decimal("p").unwrap(), 1);
        assert_eq!(
            STANDARD_NO_PAD
                .decode(parsed.salt.unwrap().as_str())
                .unwrap()
                .len(),
            16
        );
        assert_eq!(parsed.hash.unwrap().as_bytes().len(), 32);
        assert!(
            Argon2::default()
                .verify_password("uma senha local muito segura".as_bytes(), &parsed)
                .is_ok()
        );
    }

    #[test]
    fn password_verification_is_boolean_and_does_not_leak_hash_details() {
        let encoded = hash_password("uma senha local muito segura").unwrap();

        assert!(verify_password("uma senha local muito segura", &encoded).unwrap());
        assert!(!verify_password("senha errada", &encoded).unwrap());
        assert!(!verify_password("qualquer senha", "formato inválido").unwrap());
    }

    #[test]
    fn credentials_have_explicit_bounds() {
        assert!(validate_username("ab").is_ok());
        assert!(validate_username(&"a".repeat(64)).is_ok());
        assert!(validate_username("a").is_err());
        assert!(validate_username(&"a".repeat(65)).is_err());
        assert!(validate_password(&"a".repeat(12)).is_ok());
        assert!(validate_password(&"a".repeat(1024)).is_ok());
        assert!(validate_password(&"a".repeat(11)).is_err());
        assert!(validate_password(&"a".repeat(1025)).is_err());
    }

    #[test]
    fn session_tokens_are_random_and_only_their_hash_is_persisted() {
        let first = generate_session_token().unwrap();
        let second = generate_session_token().unwrap();

        assert_ne!(first, second);
        assert_eq!(first.len(), 64);
        assert_eq!(second.len(), 64);
        assert_ne!(first, hash_session_token(&first));
        assert_eq!(hash_session_token(&first), hash_session_token(&first));
    }
}
