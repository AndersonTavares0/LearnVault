use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use learnvault_core::{AuthSession, AuthUser, Database, Error as CoreError};
use tokio::sync::Mutex;

use crate::{Config, auth::hash_password};

const SESSION_LIFETIME_SECONDS: i64 = 12 * 60 * 60;

/// Estado compartilhado mínimo da aplicação HTTP.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Configuração validada usada pelo servidor.
    pub config: Config,
    pub(crate) database: Arc<Mutex<Database>>,
    pub(crate) login_attempts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
}

/// Erro de inicialização seguro para logs do servidor.
#[derive(Debug, thiserror::Error)]
pub enum StateError {
    /// Falha de persistência do core.
    #[error("database initialization failed")]
    Database(#[source] CoreError),
    /// O diretório pai do banco não pôde ser criado.
    #[error("database directory initialization failed")]
    DatabaseDirectory,
    /// Não existe administrador e não foram fornecidas credenciais válidas.
    #[error("administrator credentials are required for first startup")]
    MissingAdministrator,
    /// A senha inicial não pôde ser transformada em hash.
    #[error("initial administrator password is invalid")]
    InvalidAdministrator,
    /// A thread de trabalho não pôde concluir.
    #[error("blocking authentication task failed")]
    Blocking(#[source] tokio::task::JoinError),
}

impl AppState {
    /// Cria o estado inicial da aplicação.
    pub fn new(config: Config) -> Self {
        let database = Database::open_in_memory().expect("in-memory database must initialize");
        Self {
            config,
            database: Arc::new(Mutex::new(database)),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Abre o banco e executa o bootstrap administrativo fora das threads Tokio.
    pub async fn initialize(config: Config) -> Result<Self, StateError> {
        let setup = config.clone();
        let (database, _) = tokio::task::spawn_blocking(move || {
            if let Some(parent) = setup
                .database_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent).map_err(|_| StateError::DatabaseDirectory)?;
            }
            let mut database =
                Database::open(&setup.database_path).map_err(StateError::Database)?;
            let existing = learnvault_core::has_admin(&database).map_err(StateError::Database)?;
            if existing {
                return Ok((database, false));
            }
            let (Some(username), Some(password)) = (
                setup.admin_username.as_deref(),
                setup.admin_password.as_deref(),
            ) else {
                return Err(StateError::MissingAdministrator);
            };
            crate::auth::validate_username(username)
                .map_err(|_| StateError::InvalidAdministrator)?;
            crate::auth::validate_password(password)
                .map_err(|_| StateError::InvalidAdministrator)?;
            let password_hash =
                hash_password(password).map_err(|_| StateError::InvalidAdministrator)?;
            let created = learnvault_core::bootstrap_admin(
                &mut database,
                "usr-admin",
                username,
                &password_hash,
                unix_now(),
            )
            .map_err(StateError::Database)?;
            Ok((database, created))
        })
        .await
        .map_err(StateError::Blocking)??;
        Ok(Self {
            config,
            database: Arc::new(Mutex::new(database)),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub(crate) async fn allow_login_attempt(&self, username: &str) -> bool {
        let mut attempts = self.login_attempts.lock().await;
        let now = Instant::now();
        attempts.retain(|_, (_, started)| now.duration_since(*started) < Duration::from_secs(60));
        let key = username.to_ascii_lowercase();
        if attempts.len() >= 1024 && !attempts.contains_key(&key) {
            return false;
        }
        let entry = attempts.entry(key).or_insert((0, now));
        if now.duration_since(entry.1) >= Duration::from_secs(60) {
            *entry = (0, now);
        }
        if entry.0 >= 5 {
            return false;
        }
        entry.0 += 1;
        true
    }

    pub(crate) async fn user(&self, username: String) -> Result<Option<AuthUser>, StateError> {
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            let database = database.blocking_lock();
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
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            let database = database.blocking_lock();
            learnvault_core::session_user(&database, &token_hash, unix_now())
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
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            let mut database = database.blocking_lock();
            learnvault_core::create_session(
                &mut database,
                &token_hash,
                &user_id,
                unix_now() + SESSION_LIFETIME_SECONDS,
            )
        })
        .await
        .map_err(StateError::Blocking)?
        .map_err(StateError::Database)
    }

    pub(crate) async fn delete_session(&self, token_hash: String) -> Result<(), StateError> {
        let database = self.database.clone();
        tokio::task::spawn_blocking(move || {
            let mut database = database.blocking_lock();
            learnvault_core::delete_session(&mut database, &token_hash)
        })
        .await
        .map_err(StateError::Blocking)?
        .map_err(StateError::Database)
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
