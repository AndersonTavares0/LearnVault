//! Remote course sources (WebDAV) and the administrative endpoints that maintain them.
//!
//! The configuration lives in `sources.json`, next to the database, like the local course
//! registry. The password is never stored in clear text: each source keeps
//! `v1:<ciphertext>` with the key from [`crate::secret`]. The API returns only identity,
//! name, URL, username and the presence of a password; the secret appears in no response,
//! log or error message.
//!
//! The source is read-only: the WebDAV client only issues `PROPFIND` and `GET`.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{
    Json,
    extract::{Path as AxumPath, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{CourseSource, SourceEntry, SourceError, SourceResult, scan_source};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    AppState,
    auth::same_origin,
    catalog::{
        authenticated_user, credentials_required, csrf_failed, internal_error, private_no_store,
        service_unavailable, status_private_no_store,
    },
    secret::SecretKey,
    smb::SmbSource,
    webdav::{SourceUrlError, WebDavClient, parse_source_url},
};

/// Name of the source configuration file inside the data directory.
pub(crate) const SOURCES_FILE_NAME: &str = "sources.json";
/// Version of the configuration file format.
const SOURCES_VERSION: u32 = 1;
/// Character limit of the displayed name.
const MAX_SOURCE_NAME_CHARS: usize = 120;
/// Maximum time to walk a remote source.
const SOURCE_SCAN_TIMEOUT: Duration = Duration::from_secs(120);
/// Maximum time of the connection test.
pub(crate) const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Remote course source loaded in memory.
#[derive(Clone)]
pub(crate) struct RemoteSource {
    /// Opaque identity used as `library_id` and in the endpoints.
    pub(crate) id: String,
    /// Name displayed in the catalog.
    pub(crate) name: String,
    /// Normalized WebDAV base URL.
    pub(crate) url: Url,
    /// Basic authentication username, when there is one.
    pub(crate) username: Option<String>,
    /// Clear-text password only in memory, never serialized.
    password: Option<String>,
    /// Original cipher text, preserved so the secret is not lost when the key does not load.
    stored_secret: Option<String>,
    /// True when there is a stored secret that could not be decrypted.
    pub(crate) credential_error: bool,
}

impl std::fmt::Debug for RemoteSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RemoteSource")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("url", &self.url)
            .field("username", &self.username)
            .field("has_password", &self.password.is_some())
            .finish()
    }
}

impl RemoteSource {
    /// Clear-text password to build the authentication header.
    pub(crate) fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// True when the source has a usable username and password.
    pub(crate) fn has_password(&self) -> bool {
        self.password.is_some()
    }
}

/// Adapter chosen by the URL scheme of the source.
pub(crate) enum RemoteClient {
    WebDav(WebDavClient),
    Smb(SmbSource),
}

impl RemoteClient {
    /// Creates the client of the matching transport.
    pub(crate) fn new(source: &RemoteSource) -> Result<Self, SourceError> {
        if source.url.scheme() == "smb" {
            return Ok(Self::Smb(SmbSource::new(
                source.url.clone(),
                source.username.as_deref(),
                source.password(),
            )?));
        }
        Ok(Self::WebDav(WebDavClient::new(
            source.url.clone(),
            source.username.as_deref(),
            source.password(),
        )?))
    }

    /// Confirms reachability, authentication and root directory read.
    pub(crate) async fn probe(&self) -> SourceResult<usize> {
        match self {
            Self::WebDav(client) => client.probe().await,
            Self::Smb(client) => client.probe().await,
        }
    }
}

impl CourseSource for RemoteClient {
    fn list<'a>(
        &'a self,
        directory: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = SourceResult<Vec<SourceEntry>>> + Send + 'a>,
    > {
        match self {
            Self::WebDav(client) => client.list(directory),
            Self::Smb(client) => client.list(directory),
        }
    }

    fn read_range<'a>(
        &'a self,
        path: &'a str,
        offset: u64,
        length: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = SourceResult<Vec<u8>>> + Send + 'a>>
    {
        match self {
            Self::WebDav(client) => client.read_range(path, offset, length),
            Self::Smb(client) => client.read_range(path, offset, length),
        }
    }
}

/// Persisted entry. The secret is the only encrypted field.
#[derive(Debug, Serialize, Deserialize)]
struct StoredSource {
    id: String,
    name: String,
    url: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    secret: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SourcesFile {
    version: u32,
    sources: Vec<StoredSource>,
}

/// Path of the sources file for a database.
pub(crate) fn sources_path(database_path: &Path) -> PathBuf {
    let parent = database_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join(SOURCES_FILE_NAME)
}

/// Loads the persisted sources.
///
/// A missing, corrupted or unknown-version file results in an empty list, as in the course
/// registry. Entries whose URL is invalid are discarded. A secret that could not be
/// decrypted keeps the source, marked so the interface explains the failure instead of
/// trying to authenticate without a password.
pub(crate) fn load_sources(path: &Path, key_path: &Path) -> Vec<RemoteSource> {
    let Ok(raw) = fs::read(path) else {
        return Vec::new();
    };
    let Ok(stored) = serde_json::from_slice::<SourcesFile>(&raw) else {
        return Vec::new();
    };
    if stored.version != SOURCES_VERSION {
        return Vec::new();
    }
    let key = SecretKey::load(key_path).ok();
    let mut sources = Vec::with_capacity(stored.sources.len());
    for entry in stored.sources {
        let Ok(url) = parse_source_url(&entry.url) else {
            continue;
        };
        let mut credential_error = false;
        let password = match entry.secret.as_deref() {
            Some(secret) => match key.as_ref().and_then(|key| key.decrypt(secret).ok()) {
                Some(password) => Some(password),
                None => {
                    credential_error = true;
                    None
                }
            },
            None => None,
        };
        sources.push(RemoteSource {
            id: entry.id,
            name: entry.name,
            url,
            username: entry.username.filter(|username| !username.is_empty()),
            password,
            stored_secret: entry.secret,
            credential_error,
        });
    }
    sources
}

/// Stores the sources serially and atomically.
///
/// The temporary file is created exclusively and swapped by `rename`. Clear-text secrets
/// are encrypted before writing; a secret that was already stored and could not be
/// decrypted keeps the original cipher text.
pub(crate) fn store_sources(
    path: &Path,
    key_path: &Path,
    sources: &[RemoteSource],
) -> Result<(), SourcesError> {
    let key = if sources.iter().any(|source| source.password.is_some()) {
        // Creating a new key while another source still has an orphan cipher text would
        // make that cipher text unrecoverable. The administrator must restore the key or
        // remove the broken source before storing a new password.
        if sources.iter().any(|source| source.credential_error) {
            return Err(SourcesError::WriteFailed);
        }
        Some(SecretKey::load_or_create(key_path).map_err(|_| SourcesError::WriteFailed)?)
    } else {
        None
    };
    let mut stored = Vec::with_capacity(sources.len());
    for source in sources {
        let secret = match (source.password.as_deref(), source.stored_secret.as_deref()) {
            (Some(password), _) => Some(
                key.as_ref()
                    .ok_or(SourcesError::WriteFailed)?
                    .encrypt(password)
                    .map_err(|_| SourcesError::WriteFailed)?,
            ),
            (None, secret) => secret.map(str::to_owned),
        };
        stored.push(StoredSource {
            id: source.id.clone(),
            name: source.name.clone(),
            url: source.url.to_string(),
            username: source.username.clone(),
            secret,
        });
    }

    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory).map_err(|_| SourcesError::WriteFailed)?;
    let payload = serde_json::to_vec(&SourcesFile {
        version: SOURCES_VERSION,
        sources: stored,
    })
    .map_err(|_| SourcesError::WriteFailed)?;
    let temporary = path.with_extension("json.tmp");
    let _ = fs::remove_file(&temporary);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| SourcesError::WriteFailed)?;
    file.write_all(&payload)
        .map_err(|_| SourcesError::WriteFailed)?;
    file.sync_all().map_err(|_| SourcesError::WriteFailed)?;
    drop(file);
    #[cfg(windows)]
    let _ = fs::remove_file(path);
    fs::rename(&temporary, path).map_err(|_| {
        let _ = fs::remove_file(&temporary);
        SourcesError::WriteFailed
    })
}

/// Sanitized error of the source configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourcesError {
    /// Writing the file failed.
    WriteFailed,
}

#[derive(Serialize)]
struct SourceResponse {
    id: String,
    name: String,
    url: String,
    username: Option<String>,
    has_password: bool,
    credential_error: bool,
}

impl From<&RemoteSource> for SourceResponse {
    fn from(source: &RemoteSource) -> Self {
        Self {
            id: source.id.clone(),
            name: source.name.clone(),
            url: source.url.to_string(),
            username: source.username.clone(),
            has_password: source.has_password(),
            credential_error: source.credential_error,
        }
    }
}

#[derive(Serialize)]
struct SourcesResponse {
    sources: Vec<SourceResponse>,
}

#[derive(Deserialize)]
pub(crate) struct CreateSourceRequest {
    name: String,
    url: String,
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

#[derive(Serialize)]
struct TestResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    entries: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

#[derive(Serialize)]
struct RemoveResponse {
    removed: bool,
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
}

/// `GET /api/admin/sources` — lists sources without ever returning the secret.
pub(crate) async fn list(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(failure) = require_admin(&state, &headers).await {
        return admin_failure_response(failure);
    }
    private_no_store(
        Json(SourcesResponse {
            sources: state
                .sources_snapshot()
                .iter()
                .map(SourceResponse::from)
                .collect(),
        })
        .into_response(),
    )
}

/// `POST /api/admin/sources` — registers a remote source.
pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateSourceRequest>,
) -> Response {
    if let Err(failure) = require_admin(&state, &headers).await {
        return admin_failure_response(failure);
    }
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let Some(name) = validate_name(&request.name) else {
        return json_error(StatusCode::BAD_REQUEST, "invalid_source_name");
    };
    let url = match parse_source_url(&request.url) {
        Ok(url) => url,
        Err(error) => return json_error(StatusCode::BAD_REQUEST, source_url_code(error)),
    };
    let username = request.username.filter(|value| !value.trim().is_empty());
    let password = request.password.filter(|value| !value.is_empty());
    if password.is_some() && username.is_none() {
        return json_error(StatusCode::BAD_REQUEST, "username_required");
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let sources = Arc::clone(&state.sources);
    let sources_file = state.sources_file.clone();
    let key_path = state.source_key_path.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let mut guard = sources.write().map_err(|_| SourcesError::WriteFailed)?;
        let source = RemoteSource {
            id: format!("src-{}", uuid::Uuid::new_v4().simple()),
            name,
            url,
            username,
            password,
            stored_secret: None,
            credential_error: false,
        };
        let mut next = guard.clone();
        next.push(source.clone());
        store_sources(&sources_file, &key_path, &next)?;
        *guard = next;
        Ok::<_, SourcesError>(source)
    })
    .await;

    match outcome {
        Ok(Ok(source)) => private_no_store(
            (StatusCode::CREATED, Json(SourceResponse::from(&source))).into_response(),
        ),
        Ok(Err(_)) | Err(_) => internal_error("source_registry_unavailable"),
    }
}

/// `DELETE /api/admin/sources/{id}` — removes the configuration without touching the
/// remote server.
pub(crate) async fn remove(
    State(state): State<AppState>,
    AxumPath(source_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(failure) = require_admin(&state, &headers).await {
        return admin_failure_response(failure);
    }
    if !same_origin(&headers) {
        return csrf_failed();
    }
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let sources = Arc::clone(&state.sources);
    let sources_file = state.sources_file.clone();
    let key_path = state.source_key_path.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let mut guard = sources.write().map_err(|_| SourcesError::WriteFailed)?;
        let next: Vec<RemoteSource> = guard
            .iter()
            .filter(|source| source.id != source_id)
            .cloned()
            .collect();
        let removed = next.len() != guard.len();
        if removed {
            store_sources(&sources_file, &key_path, &next)?;
            *guard = next;
        }
        Ok::<_, SourcesError>(removed)
    })
    .await;

    match outcome {
        Ok(Ok(removed)) => private_no_store(Json(RemoveResponse { removed }).into_response()),
        Ok(Err(_)) | Err(_) => internal_error("source_registry_unavailable"),
    }
}

/// `POST /api/admin/sources/{id}/test` — validates reachability and authentication.
pub(crate) async fn test(
    State(state): State<AppState>,
    AxumPath(source_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(failure) = require_admin(&state, &headers).await {
        return admin_failure_response(failure);
    }
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let Some(source) = state
        .sources_snapshot()
        .into_iter()
        .find(|source| source.id == source_id)
    else {
        return status_private_no_store(StatusCode::NOT_FOUND);
    };
    if source.credential_error {
        return private_no_store(
            Json(TestResponse::failure("credentials_unavailable")).into_response(),
        );
    }
    let client = match RemoteClient::new(&source) {
        Ok(client) => client,
        Err(_) => {
            return private_no_store(Json(TestResponse::failure("invalid_source")).into_response());
        }
    };
    match tokio::time::timeout(PROBE_TIMEOUT, client.probe()).await {
        Ok(Ok(entries)) => private_no_store(
            Json(TestResponse {
                ok: true,
                entries: Some(entries),
                error: None,
            })
            .into_response(),
        ),
        Ok(Err(error)) => {
            private_no_store(Json(TestResponse::failure(source_error_code(error))).into_response())
        }
        Err(_) => private_no_store(Json(TestResponse::failure("timeout")).into_response()),
    }
}

/// `POST /api/admin/sources/{id}/scan` — discovers the remote tree and applies it to the
/// catalog.
pub(crate) async fn scan(
    State(state): State<AppState>,
    AxumPath(source_id): AxumPath<String>,
    headers: HeaderMap,
) -> Response {
    if let Err(failure) = require_admin(&state, &headers).await {
        return admin_failure_response(failure);
    }
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let Ok(_local_lock) = state.rescan_lock.clone().try_lock_owned() else {
        return status_private_no_store(StatusCode::CONFLICT);
    };
    let Some(source) = state
        .sources_snapshot()
        .into_iter()
        .find(|source| source.id == source_id)
    else {
        return status_private_no_store(StatusCode::NOT_FOUND);
    };
    if source.credential_error {
        return json_error(StatusCode::BAD_REQUEST, "credentials_unavailable");
    }
    let client = match RemoteClient::new(&source) {
        Ok(client) => client,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "invalid_source"),
    };

    // The network happens before the database lock: a slow source does not block the other
    // catalog read operations.
    let outcome = match tokio::time::timeout(SOURCE_SCAN_TIMEOUT, scan_source(&client)).await {
        Ok(outcome) => outcome,
        Err(_) => return json_error(StatusCode::GATEWAY_TIMEOUT, "source_scan_timeout"),
    };

    let failure = outcome
        .as_ref()
        .err()
        .map(|error| source_error_code(*error));
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let database = state.database.clone().lock_owned().await;
    let Some(blocking_task) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    let library_id = source.id.clone();
    let blocking = tokio::task::spawn_blocking(move || {
        let _blocking_task = blocking_task;
        let mut database = database;
        crate::source_roots::apply_scan_by_roots(&mut database, &library_id, outcome)
    })
    .await;

    match blocking {
        Ok(Ok(summary)) => {
            private_no_store(Json(crate::catalog::RescanResponse::from(summary)).into_response())
        }
        // The source failure has already been recorded in the history by
        // `apply_source_scan`; the response returns the sanitized reason instead of a
        // generic 500.
        Ok(Err(_)) => match failure {
            Some(error) => json_error(StatusCode::BAD_GATEWAY, error),
            None => internal_error("source_scan_failed"),
        },
        Err(_) => internal_error("source_scan_failed"),
    }
}

/// Rejection reason of an administrative session, translated only in the handler.
pub(crate) enum AdminFailure {
    Unauthenticated,
    Forbidden,
    CredentialsRequired,
    Capacity,
}

pub(crate) async fn require_admin(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<learnvault_core::AuthSession, AdminFailure> {
    match authenticated_user(state, headers).await {
        Ok(Some(user)) if user.is_admin && !user.must_change_credentials => Ok(user),
        Ok(Some(user)) if user.is_admin => Err(AdminFailure::CredentialsRequired),
        Ok(Some(_)) => Err(AdminFailure::Forbidden),
        Err(crate::StateError::Capacity) => Err(AdminFailure::Capacity),
        Ok(None) | Err(_) => Err(AdminFailure::Unauthenticated),
    }
}

pub(crate) fn admin_failure_response(failure: AdminFailure) -> Response {
    match failure {
        AdminFailure::Unauthenticated => status_private_no_store(StatusCode::UNAUTHORIZED),
        AdminFailure::Forbidden => status_private_no_store(StatusCode::FORBIDDEN),
        AdminFailure::CredentialsRequired => credentials_required(),
        AdminFailure::Capacity => service_unavailable(),
    }
}

impl TestResponse {
    fn failure(error: &'static str) -> Self {
        Self {
            ok: false,
            entries: None,
            error: Some(error),
        }
    }
}

fn validate_name(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty()
        || name.chars().count() > MAX_SOURCE_NAME_CHARS
        || name.chars().any(char::is_control)
    {
        return None;
    }
    Some(name.to_owned())
}

fn source_url_code(error: SourceUrlError) -> &'static str {
    match error {
        SourceUrlError::UnsupportedScheme => "unsupported_source_scheme",
        SourceUrlError::InvalidUrl
        | SourceUrlError::MissingHost
        | SourceUrlError::MissingShare
        | SourceUrlError::Traversal => "invalid_source_url",
        SourceUrlError::CredentialsInUrl => "source_url_contains_credentials",
        SourceUrlError::UnsafeTarget => "unsafe_source_target",
        SourceUrlError::QueryOrFragment => "invalid_source_url",
    }
}

pub(crate) fn source_error_code(error: SourceError) -> &'static str {
    match error {
        SourceError::Unreachable => "unreachable",
        SourceError::Denied => "denied",
        SourceError::NotFound => "not_found",
        SourceError::InvalidResponse => "invalid_response",
        SourceError::Limit => "limit_exceeded",
    }
}

pub(crate) fn json_error(status: StatusCode, error: &'static str) -> Response {
    private_no_store((status, Json(ErrorBody { error })).into_response())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        RemoteClient, RemoteSource, SourceResponse, SourcesError, SourcesResponse, load_sources,
        sources_path, store_sources,
    };
    use crate::secret::SecretKey;
    use crate::webdav::parse_source_url;

    fn source(password: Option<&str>) -> RemoteSource {
        RemoteSource {
            id: "src-test".to_owned(),
            name: "Curso remoto".to_owned(),
            url: parse_source_url("davs://example.test/dav/").unwrap(),
            username: password.map(|_| "learner".to_owned()),
            password: password.map(str::to_owned),
            stored_secret: None,
            credential_error: false,
        }
    }

    #[test]
    fn the_secret_is_encrypted_at_rest_and_round_trips() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("learnvault.sqlite");
        let path = sources_path(&database);
        let key_path = SecretKey::path_for(&database);
        store_sources(&path, &key_path, &[source(Some("synthetic-secret"))]).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("synthetic-secret"));
        assert!(raw.contains("v1:"));

        let loaded = load_sources(&path, &key_path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].password(), Some("synthetic-secret"));
        assert!(!loaded[0].credential_error);
    }

    #[test]
    fn responses_and_debug_never_contain_the_password() {
        let source = source(Some("synthetic-secret"));
        let json = serde_json::to_string(&SourcesResponse {
            sources: vec![SourceResponse::from(&source)],
        })
        .unwrap();
        assert!(!json.contains("synthetic-secret"));
        assert!(!json.contains("secret"));
        assert!(json.contains("\"has_password\":true"));

        let debug = format!("{source:?}");
        assert!(!debug.contains("synthetic-secret"));
        assert!(debug.contains("has_password: true"));
    }

    #[test]
    fn a_missing_key_marks_the_source_without_exposing_or_dropping_the_secret() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("learnvault.sqlite");
        let path = sources_path(&database);
        let key_path = SecretKey::path_for(&database);
        store_sources(&path, &key_path, &[source(Some("synthetic-secret"))]).unwrap();
        fs::remove_file(&key_path).unwrap();

        let loaded = load_sources(&path, &key_path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].password(), None);
        assert!(loaded[0].credential_error);

        // Removing another source cannot erase the cipher text that was not decrypted yet.
        store_sources(&path, &key_path, &loaded).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("v1:"));

        // Storing a new password would create another key and lose the orphan cipher text.
        assert_eq!(
            store_sources(&path, &key_path, &[loaded[0].clone(), source(Some("nova"))]),
            Err(SourcesError::WriteFailed)
        );
    }

    #[test]
    fn an_invalid_or_corrupt_config_is_ignored() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("learnvault.sqlite");
        let path = sources_path(&database);
        let key_path = SecretKey::path_for(&database);

        fs::write(&path, b"{ not json").unwrap();
        assert!(load_sources(&path, &key_path).is_empty());

        fs::write(
            &path,
            br#"{"version":1,"sources":[{"id":"src-1","name":"x","url":"ftp://host/x"}]}"#,
        )
        .unwrap();
        assert!(load_sources(&path, &key_path).is_empty());
    }

    #[test]
    fn source_url_is_normalized_before_persisting() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("learnvault.sqlite");
        let path = sources_path(&database);
        let key_path = SecretKey::path_for(&database);
        let mut source = source(None);
        source.url = parse_source_url("dav://host/dav").unwrap();
        store_sources(&path, &key_path, &[source]).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("http://host/dav/"));
    }

    #[test]
    fn an_smb_source_keeps_its_url_and_never_persists_the_password() {
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("learnvault.sqlite");
        let path = sources_path(&database);
        let key_path = SecretKey::path_for(&database);
        let mut source = source(Some("synthetic-secret"));
        source.url = parse_source_url("smb://nas/Cursos/Modulo%201").unwrap();

        assert!(matches!(
            RemoteClient::new(&source).unwrap(),
            RemoteClient::Smb(_)
        ));
        store_sources(&path, &key_path, &[source]).unwrap();

        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("smb://nas/Cursos/Modulo%201"));
        assert!(!raw.contains("synthetic-secret"));
        assert!(raw.contains("v1:"));

        let loaded = load_sources(&path, &key_path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].url.as_str(), "smb://nas/Cursos/Modulo%201");
        assert_eq!(loaded[0].password(), Some("synthetic-secret"));
    }
}
