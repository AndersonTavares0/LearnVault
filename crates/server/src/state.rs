use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use learnvault_core::{AuthSession, AuthUser, Database, Error as CoreError};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

use crate::{
    Config,
    auth::hash_password,
    courses::{RegisteredCourse, load_registry, registry_path, seeded_courses},
    indexing::Indexer,
    ordering::{CourseOrders, load_orders, order_path},
    secret::SecretKey,
    sources::{RemoteSource, load_sources, sources_path},
    update::{UpdateChecker, github_fetcher},
};

const SESSION_LIFETIME_SECONDS: i64 = 12 * 60 * 60;
const MAX_BLOCKING_TASKS: usize = 4;
const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_LOGIN_ATTEMPT_BUCKETS: usize = 1024;
const MAX_SPA_SHELL_SIZE: u64 = 16 * 1024 * 1024;
const MAX_SPA_ASSET_STREAMS: usize = 3;
const MAX_SPA_SHELL_STREAMS: usize = 1;
const MAX_MEDIA_STREAMS: usize = 4;

/// Minimal shared state of the HTTP application.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Validated configuration used by the server.
    pub config: Config,
    /// Registered courses, in import order. Reading is fast (a snapshot) and
    /// writing happens only on import/removal, off the Tokio threads.
    pub(crate) courses: Arc<RwLock<Vec<RegisteredCourse>>>,
    /// Versioned file of the course registry, next to the database.
    pub(crate) courses_file: PathBuf,
    /// Manual ordering preferences, by the course's opaque identity.
    pub(crate) orders: Arc<RwLock<CourseOrders>>,
    /// Versioned file of the ordering preferences, next to the database.
    pub(crate) orders_file: PathBuf,
    /// Configured remote sources, with secrets only in memory.
    pub(crate) sources: Arc<RwLock<Vec<RemoteSource>>>,
    /// Versioned file of the remote sources, next to the database.
    pub(crate) sources_file: PathBuf,
    /// File of the key that encrypts the sources' secrets.
    pub(crate) source_key_path: PathBuf,
    pub(crate) database: Arc<Mutex<Database>>,
    pub(crate) login_attempts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    pub(crate) credential_attempts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    pub(crate) rescan_lock: Arc<Mutex<()>>,
    /// Automatic indexing job. Inert until [`Indexer::start`] is called by the server.
    pub(crate) indexer: Indexer,
    pub(crate) inline_script_hashes: Arc<[String]>,
    pub(crate) inline_style_hashes: Arc<[String]>,
    /// Product update check, done by the local server.
    pub(crate) update: UpdateChecker,
    spa_shell: Option<Arc<SpaShell>>,
    spa_shell_too_large: bool,
    blocking_tasks: Arc<Semaphore>,
    spa_asset_streams: Arc<Semaphore>,
    spa_shell_streams: Arc<Semaphore>,
    media_streams: Arc<Semaphore>,
}

/// Initialization error that is safe for the server's logs.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    /// Persistence failure of the core.
    #[error("database initialization failed")]
    Database(#[source] CoreError),
    /// The database's parent directory could not be created.
    #[error("database directory initialization failed")]
    DatabaseDirectory,
    /// There is no administrator and no valid credentials were provided.
    #[error("administrator credentials are required for first startup")]
    MissingAdministrator,
    /// The initial password could not be turned into a hash.
    #[error("initial administrator password is invalid")]
    InvalidAdministrator,
    /// The worker thread could not finish.
    #[error("blocking authentication task failed")]
    Blocking(#[source] tokio::task::JoinError),
    /// There is no capacity to start another blocking task.
    #[error("blocking task capacity exhausted")]
    Capacity,
}

impl AppState {
    /// Creates the application's initial state.
    pub fn new(config: Config) -> Self {
        let database = Database::open_in_memory().expect("in-memory database must initialize");
        let spa_shell_too_large = shell_too_large(config.web_root.as_deref());
        let spa_shell = load_spa_shell(config.web_root.as_deref()).map(Arc::new);
        let (inline_script_hashes, inline_style_hashes) = shell_hashes(spa_shell.as_deref());
        let courses_file = registry_path(&config.database_path);
        let update = UpdateChecker::new(config.update_check, github_fetcher());
        Self {
            update,
            inline_script_hashes: inline_script_hashes.into(),
            inline_style_hashes: inline_style_hashes.into(),
            spa_shell,
            spa_shell_too_large,
            courses: Arc::new(RwLock::new(Vec::new())),
            courses_file,
            orders: Arc::new(RwLock::new(CourseOrders::new())),
            orders_file: order_path(&config.database_path),
            sources: Arc::new(RwLock::new(Vec::new())),
            sources_file: sources_path(&config.database_path),
            source_key_path: SecretKey::path_for(&config.database_path),
            config,
            database: Arc::new(Mutex::new(database)),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
            credential_attempts: Arc::new(Mutex::new(HashMap::new())),
            rescan_lock: Arc::new(Mutex::new(())),
            indexer: Indexer::default(),
            blocking_tasks: Arc::new(Semaphore::new(MAX_BLOCKING_TASKS)),
            spa_asset_streams: Arc::new(Semaphore::new(MAX_SPA_ASSET_STREAMS)),
            spa_shell_streams: Arc::new(Semaphore::new(MAX_SPA_SHELL_STREAMS)),
            media_streams: Arc::new(Semaphore::new(MAX_MEDIA_STREAMS)),
        }
    }

    /// Opens the database, loads the course registry and runs the administrative bootstrap
    /// off the Tokio threads.
    pub async fn initialize(config: Config) -> Result<Self, StateError> {
        let setup = config.clone();
        let (
            database,
            spa_shell,
            spa_shell_too_large,
            courses,
            courses_file,
            orders,
            sources,
            sources_file,
            source_key_path,
        ) = tokio::task::spawn_blocking(move || {
            let shell_too_large = shell_too_large(setup.web_root.as_deref());
            let shell = load_spa_shell(setup.web_root.as_deref()).map(Arc::new);
            if let Some(parent) = setup
                .database_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent).map_err(|_| StateError::DatabaseDirectory)?;
            }
            // The registry is loaded next to the database; the seeded roots only come into
            // play when no registry has been persisted yet, and are never rewritten
            // here: an explicit removal of a seeded course does not come back on the next start.
            let courses_file = registry_path(&setup.database_path);
            let mut courses = load_registry(&courses_file);
            if !courses_file.exists()
                && let Some(seed) = setup.library_root.as_deref()
            {
                courses.extend(seeded_courses(seed));
            }
            // The ordering preferences live next to the database, in the ordering
            // file; a course removed from the catalog keeps its preference, which only
            // applies again if the same course is re-imported.
            let orders = load_orders(&order_path(&setup.database_path));
            let sources_file = sources_path(&setup.database_path);
            let source_key_path = SecretKey::path_for(&setup.database_path);
            let sources = load_sources(&sources_file, &source_key_path);
            let mut database =
                Database::open(&setup.database_path).map_err(StateError::Database)?;
            let existing = learnvault_core::has_admin(&database).map_err(StateError::Database)?;
            if existing {
                if !setup.listen_addr.ip().is_loopback()
                    && let Some(admin) = learnvault_core::find_user_by_username(&database, "admin")
                        .map_err(StateError::Database)?
                    && (admin.must_change_credentials
                        || crate::auth::verify_password("admin", &admin.password_hash)
                            .unwrap_or(false))
                {
                    return Err(StateError::MissingAdministrator);
                }
                return Ok((
                    database,
                    shell,
                    shell_too_large,
                    courses,
                    courses_file,
                    orders,
                    sources,
                    sources_file,
                    source_key_path,
                ));
            }
            let using_temporary_default =
                setup.admin_username.is_none() && setup.admin_password.is_none();
            if using_temporary_default && !setup.listen_addr.ip().is_loopback() {
                return Err(StateError::MissingAdministrator);
            }
            let username = setup.admin_username.as_deref().unwrap_or("admin");
            let password = setup.admin_password.as_deref().unwrap_or("admin");
            crate::auth::validate_username(username)
                .map_err(|_| StateError::InvalidAdministrator)?;
            let password_hash = if using_temporary_default {
                crate::auth::hash_password_unchecked(password)
            } else {
                hash_password(password)
            }
            .map_err(|_| StateError::InvalidAdministrator)?;
            learnvault_core::bootstrap_admin_with_change_required(
                &mut database,
                "usr-admin",
                username,
                &password_hash,
                unix_now(),
                using_temporary_default,
            )
            .map_err(StateError::Database)?;
            Ok((
                database,
                shell,
                shell_too_large,
                courses,
                courses_file,
                orders,
                sources,
                sources_file,
                source_key_path,
            ))
        })
        .await
        .map_err(StateError::Blocking)??;
        let (inline_script_hashes, inline_style_hashes) = shell_hashes(spa_shell.as_deref());
        let update = UpdateChecker::new(config.update_check, github_fetcher());
        Ok(Self {
            orders: Arc::new(RwLock::new(orders)),
            orders_file: order_path(&config.database_path),
            config,
            update,
            courses: Arc::new(RwLock::new(courses)),
            courses_file,
            sources: Arc::new(RwLock::new(sources)),
            sources_file,
            source_key_path,
            database: Arc::new(Mutex::new(database)),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
            credential_attempts: Arc::new(Mutex::new(HashMap::new())),
            rescan_lock: Arc::new(Mutex::new(())),
            indexer: Indexer::default(),
            inline_script_hashes: inline_script_hashes.into(),
            inline_style_hashes: inline_style_hashes.into(),
            spa_shell,
            spa_shell_too_large,
            blocking_tasks: Arc::new(Semaphore::new(MAX_BLOCKING_TASKS)),
            spa_asset_streams: Arc::new(Semaphore::new(MAX_SPA_ASSET_STREAMS)),
            spa_shell_streams: Arc::new(Semaphore::new(MAX_SPA_SHELL_STREAMS)),
            media_streams: Arc::new(Semaphore::new(MAX_MEDIA_STREAMS)),
        })
    }

    /// Immutable snapshot of the registered courses.
    ///
    /// A catalog read, rescan or media request uses the snapshot and never holds the write
    /// lock while reading files or the database.
    pub(crate) fn courses_snapshot(&self) -> Vec<RegisteredCourse> {
        self.courses
            .read()
            .map(|courses| courses.clone())
            .unwrap_or_default()
    }

    /// Immutable snapshot of the remote sources.
    ///
    /// It includes the plain-text password only in memory, to build the authentication header;
    /// it is never serialized.
    pub(crate) fn sources_snapshot(&self) -> Vec<RemoteSource> {
        self.sources
            .read()
            .map(|sources| sources.clone())
            .unwrap_or_default()
    }

    /// Immutable snapshot of the ordering preferences.
    ///
    /// A catalog read uses the snapshot and never holds the write lock while
    /// reading the database or assembling the tree.
    pub(crate) fn orders_snapshot(&self) -> CourseOrders {
        self.orders
            .read()
            .map(|orders| orders.clone())
            .unwrap_or_default()
    }

    /// Reserves capacity for a task that may outlive the HTTP timeout.
    ///
    /// The permit must be moved into the `spawn_blocking` closure, so it stays
    /// accounted for until the work actually finishes.
    pub fn try_acquire_blocking_task(&self) -> Option<OwnedSemaphorePermit> {
        self.blocking_tasks.clone().try_acquire_owned().ok()
    }

    /// Cheap capacity peek to avoid holding mutexes when the server
    /// is already saturated. It is only a check (subject to a race); the real
    /// acquisition still goes through `try_acquire_*` after the locks.
    pub(crate) fn has_blocking_capacity(&self) -> bool {
        self.blocking_tasks.available_permits() > 0
    }

    /// Checks the database's readiness without exposing the persistence error to the client.
    ///
    /// The health check uses an independent read-only connection so an active writer
    /// does not make the database appear unavailable.
    pub(crate) async fn database_ready(&self) -> bool {
        let blocking_task = match self.try_acquire_blocking_task() {
            Some(blocking_task) => blocking_task,
            None => return false,
        };
        let database_path = self.config.database_path.clone();
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            learnvault_core::file_has_current_schema(database_path).unwrap_or(false)
        })
        .await
        .unwrap_or(false)
    }

    /// Reserves a SPA connection until the HTTP body is consumed, dropped or goes idle.
    ///
    /// The shell gets a reservation of its own so that slow clients downloading assets do not keep it
    /// from loading an application route.
    pub(crate) fn try_acquire_spa_stream(&self, shell: bool) -> Option<OwnedSemaphorePermit> {
        let streams = if shell {
            &self.spa_shell_streams
        } else {
            &self.spa_asset_streams
        };
        streams.clone().try_acquire_owned().ok()
    }

    /// Waits (without blocking the executor) until a SPA asset-stream reservation
    /// becomes available, returning the permit.
    ///
    /// It is the *awaitable* sibling of [`Self::try_acquire_spa_stream`]: instead of failing
    /// immediately, it registers on the semaphore and resolves exactly when the idle
    /// watchdog returns the reservation. Exposed so that the integration test observes
    /// the watchdog's release of capacity deterministically, instead of
    /// assuming that a fixed advance of the virtual clock is enough for the producer to reach
    /// the deadline after the real read IO (issue #154). `serve`'s admission
    /// still uses `try_acquire_spa_stream`; no backpressure logic changes.
    #[doc(hidden)]
    pub async fn acquire_spa_asset_stream_permit(&self) -> OwnedSemaphorePermit {
        self.spa_asset_streams
            .clone()
            .acquire_owned()
            .await
            .expect("o semáforo de streams de asset da SPA nunca é fechado")
    }

    /// Reserves an authenticated media stream until the body is consumed or dropped.
    pub(crate) fn try_acquire_media_stream(&self) -> Option<OwnedSemaphorePermit> {
        self.media_streams.clone().try_acquire_owned().ok()
    }

    /// Returns the immutable shell loaded at startup only if the configured file still
    /// has the same bytes. This way, a swap of `200.html` cannot be served with old CSP hashes;
    /// the deploy must restart the process to publish a new shell.
    pub(crate) fn current_spa_shell(&self) -> Option<Arc<[u8]>> {
        let shell = self.spa_shell.as_ref()?;
        (load_spa_shell(self.config.web_root.as_deref()).as_ref()? == shell.as_ref())
            .then(|| shell.bytes.clone())
    }

    /// The shell's size is validated at startup, before contending for blocking capacity.
    /// This preserves the 413 for an invalid configuration even under operational saturation.
    pub(crate) fn spa_shell_is_too_large(&self) -> bool {
        self.spa_shell_too_large
    }

    pub(crate) async fn allow_login_attempt(&self, source: &str) -> bool {
        self.allow_login_attempt_at(source, Instant::now()).await
    }

    pub(crate) async fn allow_credential_attempt(&self, key: &str) -> bool {
        self.allow_credential_attempt_at(key, Instant::now()).await
    }

    async fn allow_login_attempt_at(&self, source: &str, now: Instant) -> bool {
        allow_attempt_at(&self.login_attempts, source, now).await
    }

    async fn allow_credential_attempt_at(&self, key: &str, now: Instant) -> bool {
        allow_attempt_at(&self.credential_attempts, key, now).await
    }

    pub(crate) async fn user(&self, username: String) -> Result<Option<AuthUser>, StateError> {
        let database = self.database.clone().lock_owned().await;
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            learnvault_core::find_user_by_username(&database, &username)
        })
        .await
        .map_err(StateError::Blocking)?
        .map_err(StateError::Database)
    }

    pub(crate) async fn session_user(
        &self,
        token_hash: String,
    ) -> Result<Option<AuthSession>, StateError> {
        let database = self.database.clone().lock_owned().await;
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            let now = unix_now();
            let session = learnvault_core::session_user(&database, &token_hash, now)?;
            // Sliding window: renew once half of the lifetime has been consumed, so an
            // active reader keeps the session without extending it on every request.
            if let Some(session) = &session
                && session.expires_at - now <= SESSION_LIFETIME_SECONDS / 2
            {
                learnvault_core::touch_session(
                    &database,
                    &token_hash,
                    now + SESSION_LIFETIME_SECONDS,
                )?;
            }
            Ok(session)
        })
        .await
        .map_err(StateError::Blocking)?
        .map_err(StateError::Database)
    }

    pub(crate) async fn create_session(
        &self,
        token_hash: String,
        user_id: String,
    ) -> Result<(), StateError> {
        let mut database = self.database.clone().lock_owned().await;
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            let now = unix_now();
            // Housekeeping on login keeps the table bounded without a scheduler.
            learnvault_core::prune_expired_sessions(&database, now)?;
            learnvault_core::create_session(
                &mut database,
                &token_hash,
                &user_id,
                now + SESSION_LIFETIME_SECONDS,
            )
        })
        .await
        .map_err(StateError::Blocking)?
        .map_err(StateError::Database)
    }

    pub(crate) async fn delete_session(&self, token_hash: String) -> Result<(), StateError> {
        let mut database = self.database.clone().lock_owned().await;
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            learnvault_core::delete_session(&mut database, &token_hash)
        })
        .await
        .map_err(StateError::Blocking)?
        .map_err(StateError::Database)
    }

    pub(crate) async fn update_credentials_and_session(
        &self,
        user_id: String,
        username: String,
        password_hash: String,
        token_hash: String,
    ) -> Result<(), StateError> {
        let mut database = self.database.clone().lock_owned().await;
        let permit = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            learnvault_core::update_credentials_and_session(
                &mut database,
                &user_id,
                &username,
                &password_hash,
                &token_hash,
                unix_now() + SESSION_LIFETIME_SECONDS,
            )
        })
        .await
        .map_err(StateError::Blocking)?
        .map_err(StateError::Database)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SpaShell {
    bytes: Arc<[u8]>,
    script_hashes: Vec<String>,
    style_hashes: Vec<String>,
}

/// Loads the shell from the same content that is used for the CSP hashes. I/O failures or a
/// missing shell leave the policy with no hashes, preserving the HTTP fallback without revealing local
/// paths. The web root must remain free of untrusted writes: this check detects
/// changes between requests, but does not replace atomic containment against a hostile process.
fn load_spa_shell(web_root: Option<&Path>) -> Option<SpaShell> {
    let root = web_root?;
    let shell = root.join("200.html");
    let metadata = match fs::symlink_metadata(&shell) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
        _ => return None,
    };
    if metadata.len() > MAX_SPA_SHELL_SIZE {
        return None;
    }
    let source = match fs::read(shell) {
        Ok(source) => source,
        Err(_) => return None,
    };
    Some(SpaShell {
        script_hashes: csp_hashes(inline_scripts(&source)),
        style_hashes: csp_hashes(inline_styles(&source)),
        bytes: source.into(),
    })
}

fn shell_too_large(web_root: Option<&Path>) -> bool {
    web_root
        .map(|root| root.join("200.html"))
        .and_then(|shell| fs::symlink_metadata(shell).ok())
        .is_some_and(|metadata| {
            metadata.is_file()
                && !metadata.file_type().is_symlink()
                && metadata.len() > MAX_SPA_SHELL_SIZE
        })
}

fn shell_hashes(shell: Option<&SpaShell>) -> (Vec<String>, Vec<String>) {
    shell.map_or_else(
        || (Vec::new(), Vec::new()),
        |shell| (shell.script_hashes.clone(), shell.style_hashes.clone()),
    )
}

fn csp_hashes(values: Vec<&[u8]>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| format!("sha256-{}", STANDARD.encode(Sha256::digest(value))))
        .collect()
}

fn inline_scripts(source: &[u8]) -> Vec<&[u8]> {
    let mut scripts = Vec::new();
    let mut offset = 0;
    while let Some(start) = find_ascii_case_insensitive(source, b"<script", offset) {
        let name_end = start + b"<script".len();
        let valid_name_end = source
            .get(name_end)
            .is_some_and(|byte| *byte == b'>' || *byte == b'/' || byte.is_ascii_whitespace());
        if !valid_name_end {
            offset = name_end;
            continue;
        }
        let Some(open_end) = tag_end(source, name_end) else {
            break;
        };
        let Some(close_start) = find_closing_tag(source, b"script", open_end + 1) else {
            break;
        };
        let Some(close_end) = tag_end(source, close_start + b"</script".len()) else {
            break;
        };
        if !has_src_attribute(&source[name_end..=open_end]) {
            scripts.push(&source[open_end + 1..close_start]);
        }
        offset = close_end + 1;
    }
    scripts
}

fn inline_styles(source: &[u8]) -> Vec<&[u8]> {
    let mut styles = inline_style_attributes(source);
    let mut offset = 0;
    while let Some(start) = find_ascii_case_insensitive(source, b"<style", offset) {
        let name_end = start + b"<style".len();
        let valid_name_end = source
            .get(name_end)
            .is_some_and(|byte| *byte == b'>' || *byte == b'/' || byte.is_ascii_whitespace());
        if !valid_name_end {
            offset = name_end;
            continue;
        }
        let Some(open_end) = tag_end(source, name_end) else {
            break;
        };
        let Some(close_start) = find_closing_tag(source, b"style", open_end + 1) else {
            break;
        };
        styles.push(&source[open_end + 1..close_start]);
        let Some(close_end) = tag_end(source, close_start + b"</style".len()) else {
            break;
        };
        offset = close_end + 1;
    }
    styles
}

fn inline_style_attributes(source: &[u8]) -> Vec<&[u8]> {
    let mut styles = Vec::new();
    let mut offset = 0;
    while let Some(start) = source[offset..].iter().position(|byte| *byte == b'<') {
        let start = offset + start;
        let tag_start = start + 1;
        let Some(&first) = source.get(tag_start) else {
            break;
        };
        if matches!(first, b'/' | b'!' | b'?') {
            offset = tag_start;
            continue;
        }
        let Some(end) = tag_end(source, tag_start) else {
            break;
        };
        let tag = &source[tag_start..end];
        let name_end = tag
            .iter()
            .position(|byte| byte.is_ascii_whitespace() || *byte == b'/')
            .unwrap_or(tag.len());
        if tag[..name_end].eq_ignore_ascii_case(b"script") {
            let Some(close_start) = find_closing_tag(source, b"script", end + 1) else {
                break;
            };
            let Some(close_end) = tag_end(source, close_start + b"</script".len()) else {
                break;
            };
            offset = close_end + 1;
            continue;
        }
        styles.extend(attribute_values(tag, b"style"));
        offset = end + 1;
    }
    styles
}

fn attribute_values<'a>(tag: &'a [u8], name: &[u8]) -> Vec<&'a [u8]> {
    let mut values = Vec::new();
    let mut offset = tag
        .iter()
        .position(|byte| byte.is_ascii_whitespace() || *byte == b'/')
        .unwrap_or(tag.len());
    while offset < tag.len() {
        while matches!(tag.get(offset), Some(byte) if byte.is_ascii_whitespace() || *byte == b'/') {
            offset += 1;
        }
        let attribute_start = offset;
        while matches!(tag.get(offset), Some(byte) if byte.is_ascii_alphanumeric()
            || matches!(*byte, b'-' | b':'))
        {
            offset += 1;
        }
        if attribute_start == offset {
            offset += 1;
            continue;
        }
        let matches_name = tag[attribute_start..offset].eq_ignore_ascii_case(name);
        while matches!(tag.get(offset), Some(byte) if byte.is_ascii_whitespace()) {
            offset += 1;
        }
        if tag.get(offset) != Some(&b'=') {
            // The name and every separator that follows it have already been consumed. Keeping `offset` at that
            // point guarantees that boolean attributes (for example `hidden` and `defer`) advance
            // to the next attribute instead of reprocessing the current name.
            continue;
        }
        offset += 1;
        while matches!(tag.get(offset), Some(byte) if byte.is_ascii_whitespace()) {
            offset += 1;
        }
        let value_start = offset;
        let value_end = match tag.get(offset) {
            Some(b'\'' | b'"') => {
                let delimiter = tag[offset];
                offset += 1;
                let value_start = offset;
                while tag.get(offset).is_some_and(|byte| *byte != delimiter) {
                    offset += 1;
                }
                let value_end = offset;
                offset += usize::from(tag.get(offset) == Some(&delimiter));
                if matches_name {
                    values.push(&tag[value_start..value_end]);
                }
                continue;
            }
            Some(_) => {
                while matches!(tag.get(offset), Some(byte) if !byte.is_ascii_whitespace()
                    && *byte != b'/')
                {
                    offset += 1;
                }
                offset
            }
            None => break,
        };
        if matches_name {
            values.push(&tag[value_start..value_end]);
        }
    }
    values
}

fn find_ascii_case_insensitive(source: &[u8], needle: &[u8], offset: usize) -> Option<usize> {
    source[offset..]
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
        .map(|index| offset + index)
}

fn find_closing_tag(source: &[u8], name: &[u8], mut offset: usize) -> Option<usize> {
    let mut needle = Vec::with_capacity(name.len() + 2);
    needle.extend_from_slice(b"</");
    needle.extend_from_slice(name);
    while let Some(start) = find_ascii_case_insensitive(source, &needle, offset) {
        let name_end = start + needle.len();
        if source
            .get(name_end)
            .is_some_and(|byte| *byte == b'>' || *byte == b'/' || byte.is_ascii_whitespace())
        {
            return Some(start);
        }
        offset = name_end;
    }
    None
}

fn tag_end(source: &[u8], mut offset: usize) -> Option<usize> {
    let mut quote = None;
    while let Some(&byte) = source.get(offset) {
        match (quote, byte) {
            (Some(delimiter), byte) if byte == delimiter => quote = None,
            (None, b'\'' | b'\"') => quote = Some(byte),
            (None, b'>') => return Some(offset),
            _ => {}
        }
        offset += 1;
    }
    None
}

fn has_src_attribute(tag: &[u8]) -> bool {
    let mut offset = 0;
    while offset < tag.len() {
        while matches!(tag.get(offset), Some(byte) if byte.is_ascii_whitespace() || *byte == b'/') {
            offset += 1;
        }
        let start = offset;
        while matches!(tag.get(offset), Some(byte) if byte.is_ascii_alphanumeric() || *byte == b'-' || *byte == b':')
        {
            offset += 1;
        }
        if start == offset {
            offset += 1;
            continue;
        }
        if tag[start..offset].eq_ignore_ascii_case(b"src") {
            return true;
        }
        while tag
            .get(offset)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && *byte != b'>')
        {
            offset += 1;
        }
    }
    false
}

async fn allow_attempt_at(
    table: &Mutex<HashMap<String, (u32, Instant)>>,
    key: &str,
    now: Instant,
) -> bool {
    let mut attempts = table.lock().await;
    attempts
        .retain(|_, (_, started)| now.saturating_duration_since(*started) < LOGIN_ATTEMPT_WINDOW);
    let key = key.to_owned();
    if attempts.len() >= MAX_LOGIN_ATTEMPT_BUCKETS && !attempts.contains_key(&key) {
        // Keep the table bounded without evicting an active source bucket.
        return false;
    }
    let entry = attempts.entry(key).or_insert((0, now));
    if now.saturating_duration_since(entry.1) >= LOGIN_ATTEMPT_WINDOW {
        *entry = (0, now);
    }
    if entry.0 >= 5 {
        return false;
    }
    entry.0 += 1;
    true
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use tempfile::tempdir;
    use tokio::time::Instant;

    use super::{AppState, StateError, attribute_values};
    use crate::Config;

    fn config() -> Config {
        Config {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            database_path: PathBuf::from("learnvault.sqlite"),
            library_root: None,
            web_root: None,
            admin_username: None,
            admin_password: None,
            cookie_secure: false,
            update_check: true,
        }
    }

    #[tokio::test]
    async fn state_blocking_operations_reject_when_capacity_is_saturated() {
        let state = AppState::new(config());
        let _permits: Vec<_> = std::iter::from_fn(|| state.try_acquire_blocking_task()).collect();

        assert!(matches!(
            state.user("admin".to_owned()).await,
            Err(StateError::Capacity)
        ));
        assert!(matches!(
            state.session_user("token".to_owned()).await,
            Err(StateError::Capacity)
        ));
        assert!(matches!(
            state
                .create_session("token".to_owned(), "user".to_owned())
                .await,
            Err(StateError::Capacity)
        ));
        assert!(matches!(
            state.delete_session("token".to_owned()).await,
            Err(StateError::Capacity)
        ));
    }

    #[tokio::test]
    async fn missing_bootstrap_configuration_creates_loopback_only_temporary_admin() {
        let directory = tempdir().unwrap();
        let mut setup = config();
        setup.database_path = directory.path().join("learnvault.sqlite");
        let state = AppState::initialize(setup).await.unwrap();
        let user = state.user("admin".to_owned()).await.unwrap().unwrap();
        assert_eq!(user.username, "admin");
        assert!(user.is_admin);
        assert!(user.must_change_credentials);
        assert!(crate::verify_password("admin", &user.password_hash).unwrap());
        assert_ne!(user.password_hash, "admin");
    }

    #[tokio::test]
    async fn missing_bootstrap_credentials_are_rejected_on_network_listener() {
        let directory = tempdir().unwrap();
        let mut setup = config();
        setup.database_path = directory.path().join("learnvault.sqlite");
        setup.listen_addr = "0.0.0.0:8090".parse().unwrap();
        assert!(matches!(
            AppState::initialize(setup).await,
            Err(StateError::MissingAdministrator)
        ));
    }

    #[tokio::test]
    async fn network_listener_is_rejected_until_temporary_admin_changes_credentials() {
        let directory = tempdir().unwrap();
        let mut setup = config();
        setup.database_path = directory.path().join("learnvault.sqlite");
        AppState::initialize(setup.clone()).await.unwrap();
        setup.listen_addr = "0.0.0.0:8090".parse().unwrap();
        assert!(matches!(
            AppState::initialize(setup).await,
            Err(StateError::MissingAdministrator)
        ));
    }

    #[tokio::test]
    async fn database_waiters_do_not_consume_blocking_capacity() {
        let state = AppState::new(config());
        let database = state.database.clone().lock_owned().await;
        let tasks: Vec<_> = (0..4)
            .map(|_| {
                let state = state.clone();
                tokio::spawn(async move { state.user("admin".to_owned()).await })
            })
            .collect();

        tokio::task::yield_now().await;
        tokio::task::yield_now().await;
        let permit = state.try_acquire_blocking_task();
        assert!(permit.is_some());
        drop(permit);

        drop(database);
        for task in tasks {
            assert!(matches!(task.await.unwrap(), Ok(None)));
        }
    }

    #[tokio::test]
    async fn distinct_sources_do_not_evict_an_active_login_bucket() {
        let state = AppState::new(config());
        let now = Instant::now();

        for _ in 0..5 {
            assert!(state.allow_login_attempt_at("admin", now).await);
        }
        for index in 0..1023 {
            assert!(
                state
                    .allow_login_attempt_at(&format!("user-{index}"), now)
                    .await
            );
        }

        assert!(!state.allow_login_attempt_at("new-user", now).await);
        assert!(!state.allow_login_attempt_at("admin", now).await);
        let attempts = state.login_attempts.lock().await;
        assert_eq!(attempts.len(), 1024);
        assert!(attempts.contains_key("admin"));
        assert!(!attempts.contains_key("new-user"));
    }

    #[tokio::test]
    async fn one_source_limit_does_not_lock_the_same_username_for_other_sources() {
        let state = AppState::new(config());
        let now = Instant::now();
        for _ in 0..5 {
            assert!(state.allow_login_attempt_at("192.0.2.10", now).await);
        }
        assert!(!state.allow_login_attempt_at("192.0.2.10", now).await);
        assert!(state.allow_login_attempt_at("192.0.2.11", now).await);
    }

    #[tokio::test]
    async fn credential_attempt_table_saturation_does_not_affect_login_attempts() {
        let state = AppState::new(config());
        let now = Instant::now();

        for index in 0..1024 {
            assert!(
                state
                    .allow_credential_attempt_at(&format!("usr-{index}\x00192.0.2.10"), now)
                    .await
            );
        }
        assert!(
            !state
                .allow_credential_attempt_at("usr-overflow\x00192.0.2.10", now)
                .await
        );
        assert!(state.allow_login_attempt_at("192.0.2.10", now).await);
        assert_eq!(state.credential_attempts.lock().await.len(), 1024);
        assert_eq!(state.login_attempts.lock().await.len(), 1);
    }

    #[test]
    fn attribute_values_skips_boolean_and_valueless_attributes() {
        let tag =
            br#"div hidden defer data-pending style="display: grid" disabled STYLE='color: red'"#;

        assert_eq!(
            attribute_values(tag, b"style"),
            vec![b"display: grid".as_slice(), b"color: red".as_slice()]
        );
    }
}
