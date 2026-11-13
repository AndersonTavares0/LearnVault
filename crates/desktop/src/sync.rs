//! Offline queue and synchronization client of the desktop.
//!
//! Every syncable write stores in the local database and enters the outbox
//! (`sync_outbox`) in the same transaction, so a crash never leaves the local write
//! without its corresponding operation. The operation keeps the same `op_id` between
//! attempts: resending what the server already accepted is a duplicate and does not repeat
//! the effect.
//!
//! The client talks only to the configured server: `POST /api/login`,
//! `POST /api/logout`, `POST /api/sync/push` and `GET /api/sync/pull`. The session is the
//! `learnvault_session` cookie, stored in `sync.json` in the application data directory
//! with mode `0600` on Unix. The cookie is never serialized to the webview, never appears
//! in a log nor in an error message.
//!
//! The server URL is validated before any request: only `http`/`https`, with a host,
//! without embedded credentials, without query, without fragment and without path.
//! Redirects are disabled so the cookie never follows to another origin. Each request has
//! a timeout; transport errors and 5xx are retried with a growing, bounded delay, while 401
//! and 403 (`credentials_change_required`) fail immediately.
//!
//! A sync pushes the outbox in pages, only acknowledges after the server accepts, and then
//! pulls from the cursor until `has_more` is false. The cursor and the effects enter the
//! same commit (`apply_operations`), so an interrupted run resumes from the previous point
//! without duplicating or losing operations. Two runs never overlap: a `SyncFlight`
//! guarantees a single run.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use learnvault_core::{
    Database, MAX_SYNC_BATCH, PushOutcome, SyncBatch, SyncOperation, SyncOperationInput, SyncState,
};
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_TYPE, COOKIE, ORIGIN, SET_COOKIE};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};

use crate::dto::SyncResultDto;
use crate::error::IpcError;
use crate::study::LOCAL_OWNER_ID;

/// Name of the configuration and session file inside the data directory.
pub const SYNC_FILE_NAME: &str = "sync.json";

/// Version of the sync file format.
const SYNC_FILE_VERSION: u32 = 1;

/// Budget of each HTTP request. Being offline cannot block the interface.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Delay between transport/5xx attempts (the first attempt does not wait).
const RETRY_DELAYS_MS: [u64; 3] = [0, 250, 750];

/// Name of the server session cookie (`auth::COOKIE_NAME`).
const SESSION_COOKIE_NAME: &str = "learnvault_session";

/// Read page of the remote journal.
const PULL_LIMIT: usize = 200;

/// Stable sync failure, translated to a short code in the summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncFailure {
    /// Server or session not configured yet.
    NotConfigured,
    /// The session expired (401); sign in again.
    Unauthorized,
    /// The server requires a credential change (403).
    CredentialsRequired,
    /// The server refused the request (other 4xx).
    Rejected,
    /// Transport failure, timeout or 5xx after the attempts.
    Unavailable,
    /// Local database failure.
    Local,
}

impl SyncFailure {
    /// Stable code consumed by the interface.
    pub fn code(self) -> &'static str {
        match self {
            Self::NotConfigured => "not_configured",
            Self::Unauthorized => "session_expired",
            Self::CredentialsRequired => "credentials_change_required",
            Self::Rejected => "sync_rejected",
            Self::Unavailable | Self::Local => "sync_unavailable",
        }
    }
}

/// Persisted configuration: server and session.
///
/// `Debug` is manual so the session never appears in diagnostics.
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SyncSettings {
    /// File version, to discard an unknown format.
    pub version: u32,
    /// Server base URL, already normalized.
    pub server_url: String,
    /// Session cookie in `name=value` format; empty when there is no session.
    pub session: String,
}

impl std::fmt::Debug for SyncSettings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SyncSettings")
            .field("server_url", &self.server_url)
            .field("signed_in", &self.signed_in())
            .finish_non_exhaustive()
    }
}

impl SyncSettings {
    /// Indicates a server has been chosen.
    pub fn configured(&self) -> bool {
        !self.server_url.trim().is_empty()
    }

    /// Indicates a session is stored.
    pub fn signed_in(&self) -> bool {
        !self.session.is_empty()
    }
}

/// Reads the configuration from the data directory; missing, corrupted or in an unknown
/// version counts as not configured.
pub fn load_settings(data_dir: &Path) -> SyncSettings {
    fs::read(data_dir.join(SYNC_FILE_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SyncSettings>(&bytes).ok())
        .filter(|settings| settings.version == SYNC_FILE_VERSION)
        .unwrap_or_default()
}

/// Stores the configuration atomically and with mode `0600` on Unix.
///
/// A write failure does not change the previous file. On Windows there is no POSIX mode;
/// the file inherits the ACL of the application data directory.
pub fn store_settings(path: &Path, settings: &SyncSettings) -> Result<(), IpcError> {
    let mut stored = settings.clone();
    stored.version = SYNC_FILE_VERSION;
    let payload = serde_json::to_vec(&stored).map_err(|_| IpcError::sync_unavailable())?;

    let temporary = path.with_extension(format!("tmp.{}", std::process::id()));
    let _ = fs::remove_file(&temporary);
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|_| IpcError::sync_unavailable())?;
    if file
        .write_all(&payload)
        .and_then(|()| file.sync_all())
        .is_err()
    {
        let _ = fs::remove_file(&temporary);
        return Err(IpcError::sync_unavailable());
    }
    drop(file);
    #[cfg(windows)]
    let _ = fs::remove_file(path);
    if fs::rename(&temporary, path).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err(IpcError::sync_unavailable());
    }
    Ok(())
}

/// Validates and normalizes the server base URL.
///
/// Only `http`/`https`, with an explicit host, without embedded credentials, query,
/// fragment or path. Normalization removes the trailing slash to concatenate `/api/...`.
pub fn validate_server_url(raw: &str) -> Result<String, IpcError> {
    let mut url = Url::parse(raw.trim()).map_err(|_| IpcError::invalid_sync_server())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(IpcError::invalid_sync_server());
    }
    url.set_path("");
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// Authenticates on the server and returns the session cookie for the caller to store.
///
/// The password only exists in this call; the returned cookie is never logged.
pub fn login(server_url: &str, username: &str, password: &str) -> Result<String, IpcError> {
    let base = validate_server_url(server_url)?;
    let origin = origin_of(&base)?;
    let client = http_client().map_err(|_| IpcError::sync_unavailable())?;
    let body = serde_json::json!({ "username": username, "password": password }).to_string();
    let url = format!("{base}/api/login");
    let response = with_retry(|| {
        send(
            &client,
            reqwest::Method::POST,
            &url,
            &origin,
            None,
            Some(body.clone()),
        )
    })
    .map_err(|failure| match failure {
        SyncFailure::Unavailable => IpcError::sync_unavailable(),
        _ => IpcError::sync_login_failed(),
    })?;
    if !(200..=299).contains(&response.status) {
        return Err(if response.status == 401 || response.status == 403 {
            IpcError::sync_login_failed()
        } else {
            IpcError::sync_unavailable()
        });
    }
    response.session.ok_or_else(IpcError::sync_login_failed)
}

/// Ends the session on the server. A network failure is ignored: the caller deletes the
/// local copy anyway.
pub fn logout(server_url: &str, session: &str) {
    let (Ok(base), Ok(origin), Ok(client)) = (
        validate_server_url(server_url),
        origin_of(server_url),
        http_client(),
    ) else {
        return;
    };
    let _ = send(
        &client,
        reqwest::Method::POST,
        &format!("{base}/api/logout"),
        &origin,
        Some(session),
        None,
    );
}

/// Runs a complete sync: pushes the outbox, confirms the acceptance and pulls from the
/// cursor until the last page.
pub fn run(
    database: &Database,
    server_url: &str,
    session: &str,
) -> Result<SyncResultDto, SyncFailure> {
    if server_url.trim().is_empty() || session.is_empty() {
        return Err(SyncFailure::NotConfigured);
    }
    let base = validate_server_url(server_url).map_err(|_| SyncFailure::NotConfigured)?;
    let origin = origin_of(&base).map_err(|_| SyncFailure::NotConfigured)?;
    let client = http_client()?;
    let state =
        learnvault_core::sync_state(database, LOCAL_OWNER_ID).map_err(|_| SyncFailure::Local)?;

    let mut pushed = 0_usize;
    loop {
        let operations =
            learnvault_core::pending_operations(database, LOCAL_OWNER_ID, MAX_SYNC_BATCH)
                .map_err(|_| SyncFailure::Local)?;
        if operations.is_empty() {
            break;
        }
        let full_page = operations.len() == MAX_SYNC_BATCH;
        let outcome = push(
            &client,
            &base,
            &origin,
            session,
            &state.device_id,
            &operations,
        )?;
        let op_ids: Vec<String> = operations
            .iter()
            .map(|operation| operation.op_id.clone())
            .collect();
        learnvault_core::acknowledge_operations(database, LOCAL_OWNER_ID, &op_ids)
            .map_err(|_| SyncFailure::Local)?;
        pushed += outcome.applied + outcome.duplicated;
        if !full_page {
            break;
        }
    }

    let mut cursor = state.pull_cursor;
    let mut pulled = 0_usize;
    loop {
        let batch = pull(&client, &base, &origin, session, &state.device_id, cursor)?;
        learnvault_core::apply_operations(database, LOCAL_OWNER_ID, &batch.items, batch.cursor)
            .map_err(|_| SyncFailure::Local)?;
        pulled += batch.items.len();
        let advanced = batch.cursor > cursor;
        cursor = batch.cursor;
        // A server that repeats the same page cannot trap the client in a loop.
        if !batch.has_more || !advanced {
            break;
        }
    }

    let pending =
        learnvault_core::pending_count(database, LOCAL_OWNER_ID).map_err(|_| SyncFailure::Local)?;
    Ok(SyncResultDto {
        pushed,
        pulled,
        pending,
        error: None,
    })
}

/// Local state of the owner: device and already applied cursor.
pub fn state(database: &Database) -> Result<SyncState, IpcError> {
    learnvault_core::sync_state(database, LOCAL_OWNER_ID).map_err(|_| IpcError::sync_unavailable())
}

/// Stable identity of this device.
pub(crate) fn device(database: &Database) -> Result<String, IpcError> {
    state(database).map(|state| state.device_id)
}

/// Outbox operations not confirmed yet, from the oldest to the newest.
pub fn pending(database: &Database, limit: usize) -> Result<Vec<SyncOperationInput>, IpcError> {
    learnvault_core::pending_operations(database, LOCAL_OWNER_ID, limit)
        .map_err(|_| IpcError::sync_unavailable())
}

/// Number of operations still pending.
pub fn pending_count(database: &Database) -> Result<i64, IpcError> {
    learnvault_core::pending_count(database, LOCAL_OWNER_ID)
        .map_err(|_| IpcError::sync_unavailable())
}

/// Removes from the outbox the operations the server already accepted.
pub fn acknowledge(database: &Database, op_ids: &[String]) -> Result<usize, IpcError> {
    learnvault_core::acknowledge_operations(database, LOCAL_OWNER_ID, op_ids)
        .map_err(|_| IpcError::sync_unavailable())
}

/// Applies the received operations and stores the cursor in the same transaction.
pub fn apply(
    database: &Database,
    operations: &[SyncOperation],
    cursor: i64,
) -> Result<usize, IpcError> {
    learnvault_core::apply_operations(database, LOCAL_OWNER_ID, operations, cursor)
        .map_err(|_| IpcError::sync_unavailable())
}

/// Guarantees a single run: the second call returns `None` while the first lives.
#[derive(Debug, Default)]
pub struct SyncFlight {
    running: AtomicBool,
}

impl SyncFlight {
    /// Marks a run as in progress.
    pub fn try_begin(&self) -> Option<SyncGuard<'_>> {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| SyncGuard(&self.running))
    }

    /// Indicates whether a run is in progress.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Run in progress; releases the `SyncFlight` when dropped.
pub struct SyncGuard<'a>(&'a AtomicBool);

impl Drop for SyncGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

struct HttpResponse {
    status: u16,
    body: String,
    session: Option<String>,
}

fn http_client() -> Result<Client, SyncFailure> {
    static PROVIDER: Once = Once::new();
    PROVIDER.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    Client::builder()
        .timeout(REQUEST_TIMEOUT)
        // Without redirects: the session cookie never follows to another origin.
        .redirect(Policy::none())
        .build()
        .map_err(|_| SyncFailure::Unavailable)
}

fn origin_of(server_url: &str) -> Result<String, IpcError> {
    let url = Url::parse(server_url).map_err(|_| IpcError::invalid_sync_server())?;
    Ok(url.origin().ascii_serialization())
}

fn send(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    origin: &str,
    session: Option<&str>,
    body: Option<String>,
) -> Result<HttpResponse, SyncFailure> {
    let mut request = client.request(method, url).header(ORIGIN, origin);
    if let Some(body) = body {
        request = request.header(CONTENT_TYPE, "application/json").body(body);
    }
    if let Some(session) = session {
        request = request.header(COOKIE, session);
    }
    let response = request.send().map_err(|_| SyncFailure::Unavailable)?;
    let status = response.status().as_u16();
    let session = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .find_map(session_cookie);
    let body = response.text().map_err(|_| SyncFailure::Unavailable)?;
    Ok(HttpResponse {
        status,
        body,
        session,
    })
}

/// Extracts only the `learnvault_session=...` pair from a `Set-Cookie`.
fn session_cookie(value: &reqwest::header::HeaderValue) -> Option<String> {
    let value = value.to_str().ok()?;
    let pair = value.split(';').next()?.trim();
    let (name, token) = pair.split_once('=')?;
    (name == SESSION_COOKIE_NAME && !token.is_empty()).then(|| pair.to_owned())
}

fn with_retry(
    mut attempt: impl FnMut() -> Result<HttpResponse, SyncFailure>,
) -> Result<HttpResponse, SyncFailure> {
    let mut last = SyncFailure::Unavailable;
    for delay in RETRY_DELAYS_MS {
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        match attempt() {
            Ok(response) if response.status >= 500 => last = SyncFailure::Unavailable,
            Ok(response) => return Ok(response),
            Err(failure) => last = failure,
        }
    }
    Err(last)
}

fn ensure_success(response: &HttpResponse) -> Result<(), SyncFailure> {
    match response.status {
        200..=299 => Ok(()),
        401 => Err(SyncFailure::Unauthorized),
        403 if response.body.contains("credentials_change_required") => {
            Err(SyncFailure::CredentialsRequired)
        }
        400..=499 => Err(SyncFailure::Rejected),
        _ => Err(SyncFailure::Unavailable),
    }
}

fn push(
    client: &Client,
    base: &str,
    origin: &str,
    session: &str,
    device_id: &str,
    operations: &[SyncOperationInput],
) -> Result<PushOutcome, SyncFailure> {
    #[derive(Serialize)]
    struct Body<'a> {
        device_id: &'a str,
        operations: &'a [SyncOperationInput],
    }
    let body = serde_json::to_string(&Body {
        device_id,
        operations,
    })
    .map_err(|_| SyncFailure::Rejected)?;
    let url = format!("{base}/api/sync/push");
    let response = with_retry(|| {
        send(
            client,
            reqwest::Method::POST,
            &url,
            origin,
            Some(session),
            Some(body.clone()),
        )
    })?;
    ensure_success(&response)?;
    serde_json::from_str(&response.body).map_err(|_| SyncFailure::Rejected)
}

fn pull(
    client: &Client,
    base: &str,
    origin: &str,
    session: &str,
    device_id: &str,
    cursor: i64,
) -> Result<SyncBatch, SyncFailure> {
    let mut url =
        Url::parse(&format!("{base}/api/sync/pull")).map_err(|_| SyncFailure::Rejected)?;
    url.query_pairs_mut()
        .append_pair("device", device_id)
        .append_pair("cursor", &cursor.to_string())
        .append_pair("limit", &PULL_LIMIT.to_string());
    let response = with_retry(|| {
        send(
            client,
            reqwest::Method::GET,
            url.as_str(),
            origin,
            Some(session),
            None,
        )
    })?;
    ensure_success(&response)?;
    serde_json::from_str(&response.body).map_err(|_| SyncFailure::Rejected)
}

/// Path of the configuration file inside the data directory.
pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SYNC_FILE_NAME)
}
