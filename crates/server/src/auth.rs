//! Authentication primitives of the server.

use crate::{
    AppState, StateError,
    catalog::{authenticated_user, credentials_required, internal_error, service_unavailable},
};
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use axum::{
    Json,
    extract::{Extension, Path as AxumPath, State, connect_info::ConnectInfo},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    net::{IpAddr, SocketAddr},
    sync::OnceLock,
};
use thiserror::Error;

const ARGON_MEMORY_KIB: u32 = 64 * 1024;
const ARGON_ITERATIONS: u32 = 2;
const ARGON_LANES: u32 = 1;
const ARGON_OUTPUT_BYTES: usize = 32;
const SESSION_TOKEN_BYTES: usize = 32;
static TRUSTED_PROXY_IPS: OnceLock<Vec<IpAddr>> = OnceLock::new();

/// Internal errors of the authentication primitives.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    /// The user name does not meet the product's limits.
    #[error("invalid username")]
    InvalidUsername,
    /// The initial password does not meet the product's limits.
    #[error("invalid password")]
    InvalidPassword,
    /// The Argon2id hashing failed.
    #[error("password hashing failed")]
    PasswordHashing,
    /// The cryptographically secure generation of the token failed.
    #[error("secure session token generation failed")]
    TokenGeneration,
}

/// Generates an Argon2id PHC hash compatible with the Go legacy data.
pub fn hash_password(password: &str) -> Result<String, AuthError> {
    validate_password(password)?;
    hash_password_unchecked(password)
}

pub(crate) fn hash_password_unchecked(password: &str) -> Result<String, AuthError> {
    let params = Params::new(
        ARGON_MEMORY_KIB,
        ARGON_ITERATIONS,
        ARGON_LANES,
        Some(ARGON_OUTPUT_BYTES),
    )
    .map_err(|_| AuthError::PasswordHashing)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    argon
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHashing)
}

/// Verifies a password without telling a wrong password from an invalid persisted hash.
pub fn verify_password(password: &str, encoded_hash: &str) -> Result<bool, AuthError> {
    let Ok(parsed) = PasswordHash::new(encoded_hash) else {
        return Ok(false);
    };
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Validates the public limit of the user name without revealing configuration data.
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

/// Validates the public limit of the initial or login password.
pub fn validate_password(password: &str) -> Result<(), AuthError> {
    let length = password.chars().count();
    if !(12..=1024).contains(&length) {
        return Err(AuthError::InvalidPassword);
    }
    Ok(())
}

/// Validates the public limit of the current password given when changing credentials.
///
/// Mirrors the Go service's check (`1..=1024` runes) so that an empty
/// password or one above the limit is refused before reaching Argon2.
pub fn validate_current_password(password: &str) -> Result<(), AuthError> {
    let length = password.chars().count();
    if !(1..=1024).contains(&length) {
        return Err(AuthError::InvalidPassword);
    }
    Ok(())
}

/// Generates an opaque session token, hexadecimal-encoded.
pub fn generate_session_token() -> Result<String, AuthError> {
    let mut token = [0u8; SESSION_TOKEN_BYTES];
    getrandom::fill(&mut token).map_err(|_| AuthError::TokenGeneration)?;
    Ok(hex_encode(&token))
}

/// Computes the SHA-256 hash that will be persisted, never the token in the clear.
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
    must_change_credentials: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CredentialsRequest {
    /// Password currently in use, required to prevent a change by a stolen session.
    current_password: String,
    username: String,
    password: String,
}

pub(crate) async fn credentials(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<CredentialsRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let Some(token) = request_token(&headers) else {
        return private_no_store(StatusCode::UNAUTHORIZED.into_response());
    };
    let current_token_hash = hash_session_token(&token);
    let user = match state.session_user(current_token_hash.clone()).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return private_no_store(service_unavailable()),
        Ok(None) | Err(_) => return private_no_store(StatusCode::UNAUTHORIZED.into_response()),
    };
    if validate_current_password(&request.current_password).is_err()
        || validate_username(&request.username).is_err()
        || validate_password(&request.password).is_err()
    {
        return private_no_store(
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "invalid_credentials_format",
                }),
            )
                .into_response(),
        );
    }
    let source = login_source(
        peer.map(|Extension(ConnectInfo(address))| address),
        &headers,
        trusted_proxy_ips(),
    );
    let throttle_key = format!("{}\x00{source}", user.user_id);
    if !state.allow_credential_attempt(&throttle_key).await {
        return private_no_store(
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: "try again later",
                }),
            )
                .into_response(),
        );
    }
    let current_password = request.current_password.clone();
    let current_hash = user.password_hash.clone();
    let Some(permit) = state.try_acquire_blocking_task() else {
        return private_no_store(service_unavailable());
    };
    let current_valid = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        verify_password(&current_password, &current_hash).unwrap_or(false)
    })
    .await
    .unwrap_or(false);
    if !current_valid {
        return private_no_store(invalid_credentials());
    }
    let Some(permit) = state.try_acquire_blocking_task() else {
        return private_no_store(service_unavailable());
    };
    let password = request.password;
    let Ok(hash) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash_password(&password)
    })
    .await
    .unwrap_or(Err(AuthError::PasswordHashing)) else {
        return private_no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };
    let Ok(new_token) = generate_session_token() else {
        return private_no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };
    match state
        .update_credentials_and_session(
            current_token_hash,
            user.password_hash,
            request.username.clone(),
            hash,
            hash_session_token(&new_token),
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => return private_no_store(invalid_credentials()),
        Err(StateError::Capacity) => return private_no_store(service_unavailable()),
        Err(_) => return private_no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
    let mut response = Json(UserResponse {
        user_id: user.user_id,
        username: request.username,
        is_admin: user.is_admin,
        must_change_credentials: false,
    })
    .into_response();
    let Some(cookie) = session_cookie(&state, &new_token, false) else {
        return private_no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };
    response.headers_mut().insert("set-cookie", cookie);
    private_no_store(response)
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: &'static str,
}

fn invalid_credentials() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse {
            error: "invalid credentials",
        }),
    )
        .into_response()
}

pub(crate) async fn login(
    State(state): State<AppState>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let source = login_source(
        peer.map(|Extension(ConnectInfo(address))| address),
        &headers,
        trusted_proxy_ips(),
    );
    if !state.allow_login_attempt(&source).await {
        return no_store(
            (
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    error: "try again later",
                }),
            )
                .into_response(),
        );
    }
    if validate_username(&request.username).is_err() || request.password.is_empty() {
        return no_store(invalid_credentials());
    }
    let user = match state.user(request.username).await {
        Ok(user) => user,
        Err(StateError::Capacity) => return no_store(service_unavailable()),
        Err(_) => return no_store(invalid_credentials()),
    };
    let Some(user) = user else {
        return no_store(invalid_credentials());
    };
    let password = request.password;
    let hash = user.password_hash.clone();
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return no_store(service_unavailable());
    };
    let valid = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        verify_password(&password, &hash).unwrap_or(false)
    })
    .await
    .unwrap_or(false);
    if !valid {
        return no_store(invalid_credentials());
    }
    let Ok(token) = generate_session_token() else {
        return no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };
    match state
        .create_session(
            hash_session_token(&token),
            user.user_id.clone(),
            user.password_hash,
        )
        .await
    {
        Ok(true) => {}
        Ok(false) => return no_store(invalid_credentials()),
        Err(StateError::Capacity) => return no_store(service_unavailable()),
        Err(_) => return no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    }
    let mut response = Json(UserResponse {
        user_id: user.user_id,
        username: user.username,
        is_admin: user.is_admin,
        must_change_credentials: user.must_change_credentials,
    })
    .into_response();
    let Some(cookie) = session_cookie(&state, &token, false) else {
        return no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };
    response.headers_mut().insert("set-cookie", cookie);
    no_store(response)
}

fn trusted_proxy_ips() -> &'static [IpAddr] {
    TRUSTED_PROXY_IPS
        .get_or_init(|| {
            std::env::var("TRUSTED_PROXY_IPS")
                .unwrap_or_default()
                .split(',')
                .filter_map(|value| value.trim().parse().ok())
                .collect()
        })
        .as_slice()
}

fn login_source(
    peer: Option<SocketAddr>,
    headers: &HeaderMap,
    trusted_proxies: &[IpAddr],
) -> String {
    let Some(peer) = peer else {
        return "unbound-test-client".to_owned();
    };
    if trusted_proxies.contains(&peer.ip())
        && let Some(client_ip) = forwarded_client_ip(headers, trusted_proxies)
    {
        return client_ip.to_string();
    }
    peer.ip().to_string()
}

/// Resolves a client address from the canonical forwarding header. Trusted proxy hops
/// are skipped from the right so a shared upstream proxy does not become every client's key.
fn forwarded_client_ip(headers: &HeaderMap, trusted_proxies: &[IpAddr]) -> Option<IpAddr> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .rev()
        .find_map(|value| {
            value
                .to_str()
                .ok()?
                .split(',')
                .rev()
                .filter_map(parse_ip)
                .find(|address| !trusted_proxies.contains(address))
        })
}

fn parse_ip(value: &str) -> Option<IpAddr> {
    value.trim().trim_matches(['[', ']']).parse().ok()
}

#[cfg(test)]
mod login_rate_limit_tests {
    use super::login_source;
    use axum::http::{HeaderMap, HeaderValue};
    use std::net::{IpAddr, SocketAddr};

    #[test]
    fn client_ip_headers_are_used_only_from_explicitly_trusted_proxies() {
        let peer: SocketAddr = "192.0.2.1:8090".parse().unwrap();
        let client: IpAddr = "203.0.113.9".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.7"));
        headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.9"));

        assert_eq!(login_source(Some(peer), &headers, &[]), "192.0.2.1");
        assert_eq!(
            login_source(Some(peer), &headers, &[peer.ip()]),
            client.to_string()
        );
    }

    #[test]
    fn proxy_that_strips_forwarded_headers_uses_the_socket_peer() {
        let peer: SocketAddr = "192.0.2.1:8090".parse().unwrap();
        let headers = HeaderMap::new();

        assert_eq!(
            login_source(Some(peer), &headers, &[peer.ip()]),
            "192.0.2.1"
        );
    }

    #[test]
    fn sanitized_forwarded_for_is_canonical_even_if_real_ip_is_present() {
        let peer: SocketAddr = "192.0.2.1:8090".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.250"));
        headers.insert("x-forwarded-for", HeaderValue::from_static("198.51.100.7"));

        assert_eq!(
            login_source(Some(peer), &headers, &[peer.ip()]),
            "198.51.100.7"
        );
    }

    #[test]
    fn multi_hop_forwarded_for_skips_trusted_proxies_from_the_right() {
        let peer: SocketAddr = "192.0.2.1:8090".parse().unwrap();
        let trusted_upstream: IpAddr = "192.0.2.2".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("198.51.100.7, 192.0.2.2"),
        );

        assert_eq!(
            login_source(Some(peer), &headers, &[peer.ip(), trusted_upstream]),
            "198.51.100.7"
        );
    }

    #[test]
    fn forwarded_for_uses_the_hop_appended_by_the_trusted_proxy() {
        let peer: SocketAddr = "192.0.2.1:8090".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.9, 198.51.100.7, 198.51.100.8"),
        );

        assert_eq!(
            login_source(Some(peer), &headers, &[peer.ip()]),
            "198.51.100.8"
        );
        assert_eq!(login_source(Some(peer), &headers, &[]), "192.0.2.1");
    }

    #[test]
    fn forwarded_for_accepts_bracketed_and_unknown_values() {
        let peer: SocketAddr = "192.0.2.1:8090".parse().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_static("[2001:db8::7]"));
        assert_eq!(
            login_source(Some(peer), &headers, &[peer.ip()]),
            "2001:db8::7"
        );

        let mut unknown = HeaderMap::new();
        unknown.insert("x-forwarded-for", HeaderValue::from_static("unknown"));
        assert_eq!(
            login_source(Some(peer), &unknown, &[peer.ip()]),
            "192.0.2.1"
        );
    }
}

pub(crate) async fn me(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(token) = request_token(&headers) else {
        return private_no_store(StatusCode::UNAUTHORIZED.into_response());
    };
    let user = match state.session_user(hash_session_token(&token)).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return private_no_store(service_unavailable()),
        Ok(None) | Err(_) => return private_no_store(StatusCode::UNAUTHORIZED.into_response()),
    };
    let response = Json(UserResponse {
        user_id: user.user_id,
        username: user.username,
        is_admin: user.is_admin,
        must_change_credentials: user.must_change_credentials,
    })
    .into_response();
    private_no_store(response)
}

pub(crate) async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if let Some(token) = request_token(&headers)
        && matches!(
            state.delete_session(hash_session_token(&token)).await,
            Err(StateError::Capacity)
        )
    {
        return no_store(service_unavailable());
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    let Some(cookie) = session_cookie(&state, "", true) else {
        return no_store(StatusCode::INTERNAL_SERVER_ERROR.into_response());
    };
    response.headers_mut().insert("set-cookie", cookie);
    no_store(response)
}

#[derive(Debug, Serialize)]
pub(crate) struct LocalUserResponse {
    user_id: String,
    username: String,
    is_admin: bool,
    created_at: i64,
    allowed_course_ids: Option<Vec<String>>,
}

impl From<learnvault_core::LocalUserSummary> for LocalUserResponse {
    fn from(summary: learnvault_core::LocalUserSummary) -> Self {
        Self {
            user_id: summary.user_id,
            username: summary.username,
            is_admin: summary.is_admin,
            created_at: summary.created_at,
            allowed_course_ids: summary.allowed_course_ids,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AdminUsersResponse {
    items: Vec<LocalUserResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateUserRequest {
    username: String,
    password: String,
    #[serde(default)]
    allowed_course_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateUserCoursesRequest {
    allowed_course_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateUserCoursesResponse {
    user_id: String,
    allowed_course_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DeleteUserResponse {
    deleted: bool,
}

fn generate_user_id() -> Result<String, AuthError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| AuthError::TokenGeneration)?;
    Ok(format!("usr-{}", hex_encode(&bytes)))
}

async fn require_admin(state: &AppState, headers: &HeaderMap) -> Option<Response> {
    let user = match authenticated_user(state, headers).await {
        Ok(Some(user)) => user,
        Err(StateError::Capacity) => return Some(private_no_store(service_unavailable())),
        Ok(None) | Err(_) => {
            return Some(private_no_store(StatusCode::UNAUTHORIZED.into_response()));
        }
    };
    if !user.is_admin {
        return Some(private_no_store(StatusCode::FORBIDDEN.into_response()));
    }
    if user.must_change_credentials {
        return Some(credentials_required());
    }
    None
}

pub(crate) async fn list_users(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(response) = require_admin(&state, &headers).await {
        return response;
    }
    if !state.has_blocking_capacity() {
        return private_no_store(service_unavailable());
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return private_no_store(service_unavailable());
    };
    match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        learnvault_core::list_users(&database)
    })
    .await
    {
        Ok(Ok(items)) => private_no_store(
            Json(AdminUsersResponse {
                items: items.into_iter().map(Into::into).collect(),
            })
            .into_response(),
        ),
        Ok(Err(_)) | Err(_) => internal_error("users_unavailable"),
    }
}

pub(crate) async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateUserRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if let Some(response) = require_admin(&state, &headers).await {
        return response;
    }
    if validate_username(&request.username).is_err()
        || validate_password(&request.password).is_err()
    {
        return private_no_store(
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(ErrorResponse {
                    error: "invalid_credentials_format",
                }),
            )
                .into_response(),
        );
    }
    if !state.has_blocking_capacity() {
        return private_no_store(service_unavailable());
    }
    let Ok(user_id) = generate_user_id() else {
        return internal_error("user_create_failed");
    };
    let password = request.password;
    let Some(permit) = state.try_acquire_blocking_task() else {
        return private_no_store(service_unavailable());
    };
    let Ok(password_hash) = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        hash_password(&password)
    })
    .await
    .unwrap_or(Err(AuthError::PasswordHashing)) else {
        return internal_error("user_create_failed");
    };

    let mut database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return private_no_store(service_unavailable());
    };
    let username = request.username;
    let allowed_course_ids = request.allowed_course_ids;
    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        learnvault_core::create_user(
            &mut database,
            &user_id,
            &username,
            &password_hash,
            created_at,
            allowed_course_ids.as_deref(),
        )
    })
    .await
    {
        Ok(Ok(created)) => private_no_store(
            (StatusCode::CREATED, Json(LocalUserResponse::from(created))).into_response(),
        ),
        Ok(Err(learnvault_core::Error::UsernameTaken)) => private_no_store(
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "username_taken",
                }),
            )
                .into_response(),
        ),
        Ok(Err(_)) | Err(_) => internal_error("user_create_failed"),
    }
}

pub(crate) async fn set_user_courses(
    State(state): State<AppState>,
    AxumPath(user_id): AxumPath<String>,
    headers: HeaderMap,
    Json(request): Json<UpdateUserCoursesRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if let Some(response) = require_admin(&state, &headers).await {
        return response;
    }
    if !state.has_blocking_capacity() {
        return private_no_store(service_unavailable());
    }
    let mut database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return private_no_store(service_unavailable());
    };
    let target_user_id = user_id.clone();
    let allowed_course_ids = request.allowed_course_ids;
    match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        learnvault_core::set_user_course_access(
            &mut database,
            &target_user_id,
            allowed_course_ids.as_deref(),
        )
    })
    .await
    {
        Ok(Ok(allowed)) => private_no_store(
            Json(UpdateUserCoursesResponse {
                user_id,
                allowed_course_ids: allowed,
            })
            .into_response(),
        ),
        Ok(Err(learnvault_core::Error::UserNotFound)) => {
            private_no_store(StatusCode::NOT_FOUND.into_response())
        }
        Ok(Err(learnvault_core::Error::CannotModifyAdminUser)) => {
            private_no_store(StatusCode::FORBIDDEN.into_response())
        }
        Ok(Err(_)) | Err(_) => internal_error("user_courses_update_failed"),
    }
}

pub(crate) async fn delete_user(
    State(state): State<AppState>,
    AxumPath(user_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if let Some(response) = require_admin(&state, &headers).await {
        return response;
    }
    if !state.has_blocking_capacity() {
        return private_no_store(service_unavailable());
    }
    let mut database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return private_no_store(service_unavailable());
    };
    match tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        learnvault_core::delete_user(&mut database, &user_id)
    })
    .await
    {
        Ok(Ok(deleted)) => private_no_store(Json(DeleteUserResponse { deleted }).into_response()),
        Ok(Err(learnvault_core::Error::CannotModifyAdminUser)) => {
            private_no_store(StatusCode::FORBIDDEN.into_response())
        }
        Ok(Err(_)) | Err(_) => internal_error("user_delete_failed"),
    }
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
}

fn private_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        "cache-control",
        HeaderValue::from_static("private, no-store"),
    );
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

fn session_cookie(state: &AppState, token: &str, clear: bool) -> Option<HeaderValue> {
    let mut value = format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}",
        if clear { 0 } else { 43_200 }
    );
    if state.config.cookie_secure {
        value.push_str("; Secure");
    }
    // The token is hexadecimal by construction, so the conversion is infallible in
    // practice; even so, never `panic` on a response path.
    HeaderValue::from_str(&value).ok()
}

fn forwarded_host(headers: &HeaderMap) -> Option<&str> {
    let raw = headers
        .get("x-forwarded-host")
        .and_then(|value| value.to_str().ok())?;
    let host = raw.split(',').next()?.trim();
    if host.is_empty()
        || host.chars().any(|ch| {
            ch.is_control() || ch.is_whitespace() || matches!(ch, '/' | '\\' | '@' | '?' | '#')
        })
    {
        return None;
    }
    Some(host)
}

pub(crate) fn same_origin(headers: &HeaderMap) -> bool {
    let host = headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("localhost");
    let candidate = headers
        .get("origin")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
        .or_else(|| {
            let referer = headers
                .get("referer")
                .and_then(|value| value.to_str().ok())?;
            let (scheme, remainder) = if let Some(value) = referer.strip_prefix("http://") {
                ("http://", value)
            } else {
                ("https://", referer.strip_prefix("https://")?)
            };
            Some(format!("{scheme}{}", remainder.split('/').next()?))
        });
    let Some(candidate) = candidate else {
        return false;
    };
    if candidate == format!("http://{host}") || candidate == format!("https://{host}") {
        return true;
    }
    forwarded_host(headers).is_some_and(|fwd| {
        candidate == format!("http://{fwd}") || candidate == format!("https://{fwd}")
    })
}

fn csrf_failed() -> Response {
    no_store(
        (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "csrf_failed",
            }),
        )
            .into_response(),
    )
}

#[cfg(test)]
mod tests {
    use argon2::{Argon2, PasswordHash, password_hash::PasswordVerifier};

    use super::{
        generate_session_token, hash_password, hash_session_token, validate_current_password,
        validate_password, validate_username, verify_password,
    };

    #[test]
    fn password_hash_uses_the_legacy_compatible_argon2id_parameters() {
        let encoded = hash_password("uma senha local muito segura").unwrap();
        let parsed = PasswordHash::new(&encoded).unwrap();

        assert_eq!(parsed.algorithm.as_str(), "argon2id");
        assert_eq!(parsed.params.get_decimal("m").unwrap(), 65_536);
        assert_eq!(parsed.params.get_decimal("t").unwrap(), 2);
        assert_eq!(parsed.params.get_decimal("p").unwrap(), 1);
        // `Salt` now stores the decoded bytes; 16 bytes is the recommended salt
        // size and the same one produced by argon2 0.5.
        assert_eq!(parsed.salt.unwrap().as_ref().len(), 16);
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
    fn current_password_matches_the_go_service_bounds() {
        assert!(validate_current_password("a").is_ok());
        assert!(validate_current_password(&"a".repeat(1024)).is_ok());
        assert!(validate_current_password("").is_err());
        assert!(validate_current_password(&"a".repeat(1025)).is_err());
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

    #[test]
    fn same_origin_honors_valid_x_forwarded_host_and_rejects_malformed() {
        use axum::http::{HeaderMap, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert("host", HeaderValue::from_static("learnvault:8090"));
        headers.insert(
            "x-forwarded-host",
            HeaderValue::from_static("vault.example.com"),
        );
        headers.insert(
            "origin",
            HeaderValue::from_static("https://vault.example.com"),
        );
        assert!(super::same_origin(&headers));

        let mut bad = HeaderMap::new();
        bad.insert("host", HeaderValue::from_static("learnvault:8090"));
        bad.insert(
            "x-forwarded-host",
            HeaderValue::from_static("evil.com@good.com"),
        );
        bad.insert(
            "origin",
            HeaderValue::from_static("https://evil.com@good.com"),
        );
        assert!(!super::same_origin(&bad));
    }
}
