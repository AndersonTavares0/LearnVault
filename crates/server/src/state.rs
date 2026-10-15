use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use learnvault_core::{AuthSession, AuthUser, Database, Error as CoreError};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::{Config, auth::hash_password};

const SESSION_LIFETIME_SECONDS: i64 = 12 * 60 * 60;
const MAX_BLOCKING_TASKS: usize = 4;
const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_LOGIN_ATTEMPT_BUCKETS: usize = 1024;

/// Estado compartilhado mínimo da aplicação HTTP.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Configuração validada usada pelo servidor.
    pub config: Config,
    pub(crate) database: Arc<Mutex<Database>>,
    pub(crate) login_attempts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    pub(crate) rescan_lock: Arc<Mutex<()>>,
    blocking_tasks: Arc<Semaphore>,
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
    /// Não há capacidade para iniciar outra tarefa blocking.
    #[error("blocking task capacity exhausted")]
    Capacity,
}

impl AppState {
    /// Cria o estado inicial da aplicação.
    pub fn new(config: Config) -> Self {
        let database = Database::open_in_memory().expect("in-memory database must initialize");
        Self {
            config,
            database: Arc::new(Mutex::new(database)),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
            rescan_lock: Arc::new(Mutex::new(())),
            blocking_tasks: Arc::new(Semaphore::new(MAX_BLOCKING_TASKS)),
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
            rescan_lock: Arc::new(Mutex::new(())),
            blocking_tasks: Arc::new(Semaphore::new(MAX_BLOCKING_TASKS)),
        })
    }

    /// Reserva capacidade para uma tarefa que pode sobreviver ao timeout HTTP.
    ///
    /// O permit deve ser movido para a closure de `spawn_blocking`, para continuar
    /// contabilizado até que o trabalho efetivamente termine.
    pub fn try_acquire_blocking_task(&self) -> Option<OwnedSemaphorePermit> {
        self.blocking_tasks.clone().try_acquire_owned().ok()
    }

    pub(crate) async fn allow_login_attempt(&self, username: &str) -> bool {
        self.allow_login_attempt_at(username, Instant::now()).await
    }

    async fn allow_login_attempt_at(&self, username: &str, now: Instant) -> bool {
        let mut attempts = self.login_attempts.lock().await;
        attempts.retain(|_, (_, started)| now.duration_since(*started) < LOGIN_ATTEMPT_WINDOW);
        let key = username.to_ascii_lowercase();
        if attempts.len() >= MAX_LOGIN_ATTEMPT_BUCKETS && !attempts.contains_key(&key) {
            // Nunca descarte um bucket ainda ativo: uma enxurrada de usernames distintos
            // não pode reiniciar o limite de um usuário já acompanhado. Negar novos
            // usernames até o TTL expirar mantém o uso de memória limitado.
            return false;
        }
        let entry = attempts.entry(key).or_insert((0, now));
        if now.duration_since(entry.1) >= LOGIN_ATTEMPT_WINDOW {
            *entry = (0, now);
        }
        if entry.0 >= 5 {
            return false;
        }
        entry.0 += 1;
        true
    }

    pub(crate) async fn user(&self, username: String) -> Result<Option<AuthUser>, StateError> {
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        let database = self.database.clone().lock_owned().await;
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
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        let database = self.database.clone().lock_owned().await;
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
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
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        let mut database = self.database.clone().lock_owned().await;
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
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
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
        let mut database = self.database.clone().lock_owned().await;
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
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

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Instant};

    use super::{AppState, StateError};
    use crate::Config;

    fn config() -> Config {
        Config {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            database_path: PathBuf::from("learnvault.sqlite"),
            library_root: PathBuf::from("."),
            admin_username: None,
            admin_password: None,
            cookie_secure: false,
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
    async fn distinct_usernames_do_not_evict_an_active_login_bucket() {
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
}
