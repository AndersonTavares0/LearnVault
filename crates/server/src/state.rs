use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use learnvault_core::{AuthSession, AuthUser, Database, Error as CoreError};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::{Config, auth::hash_password};

const SESSION_LIFETIME_SECONDS: i64 = 12 * 60 * 60;
const MAX_BLOCKING_TASKS: usize = 4;
const LOGIN_ATTEMPT_WINDOW: Duration = Duration::from_secs(60);
const MAX_LOGIN_ATTEMPT_BUCKETS: usize = 1024;
const MAX_SPA_SHELL_SIZE: u64 = 16 * 1024 * 1024;
const MAX_SPA_ASSET_STREAMS: usize = 3;
const MAX_SPA_SHELL_STREAMS: usize = 1;
const MAX_MEDIA_STREAMS: usize = 4;

/// Estado compartilhado mínimo da aplicação HTTP.
#[derive(Clone, Debug)]
pub struct AppState {
    /// Configuração validada usada pelo servidor.
    pub config: Config,
    pub(crate) database: Arc<Mutex<Database>>,
    pub(crate) login_attempts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    pub(crate) rescan_lock: Arc<Mutex<()>>,
    pub(crate) inline_script_hashes: Arc<[String]>,
    pub(crate) inline_style_hashes: Arc<[String]>,
    blocking_tasks: Arc<Semaphore>,
    spa_asset_streams: Arc<Semaphore>,
    spa_shell_streams: Arc<Semaphore>,
    media_streams: Arc<Semaphore>,
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
        let (inline_script_hashes, inline_style_hashes) =
            shell_inline_hashes(config.web_root.as_deref());
        Self {
            inline_script_hashes: inline_script_hashes.into(),
            inline_style_hashes: inline_style_hashes.into(),
            config,
            database: Arc::new(Mutex::new(database)),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
            rescan_lock: Arc::new(Mutex::new(())),
            blocking_tasks: Arc::new(Semaphore::new(MAX_BLOCKING_TASKS)),
            spa_asset_streams: Arc::new(Semaphore::new(MAX_SPA_ASSET_STREAMS)),
            spa_shell_streams: Arc::new(Semaphore::new(MAX_SPA_SHELL_STREAMS)),
            media_streams: Arc::new(Semaphore::new(MAX_MEDIA_STREAMS)),
        }
    }

    /// Abre o banco e executa o bootstrap administrativo fora das threads Tokio.
    pub async fn initialize(config: Config) -> Result<Self, StateError> {
        let setup = config.clone();
        let (database, (inline_script_hashes, inline_style_hashes)) =
            tokio::task::spawn_blocking(move || {
                let hashes = shell_inline_hashes(setup.web_root.as_deref());
                if let Some(parent) = setup
                    .database_path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    std::fs::create_dir_all(parent).map_err(|_| StateError::DatabaseDirectory)?;
                }
                let mut database =
                    Database::open(&setup.database_path).map_err(StateError::Database)?;
                let existing =
                    learnvault_core::has_admin(&database).map_err(StateError::Database)?;
                if existing {
                    return Ok((database, hashes));
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
                learnvault_core::bootstrap_admin(
                    &mut database,
                    "usr-admin",
                    username,
                    &password_hash,
                    unix_now(),
                )
                .map_err(StateError::Database)?;
                Ok((database, hashes))
            })
            .await
            .map_err(StateError::Blocking)??;
        Ok(Self {
            config,
            database: Arc::new(Mutex::new(database)),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
            rescan_lock: Arc::new(Mutex::new(())),
            inline_script_hashes: inline_script_hashes.into(),
            inline_style_hashes: inline_style_hashes.into(),
            blocking_tasks: Arc::new(Semaphore::new(MAX_BLOCKING_TASKS)),
            spa_asset_streams: Arc::new(Semaphore::new(MAX_SPA_ASSET_STREAMS)),
            spa_shell_streams: Arc::new(Semaphore::new(MAX_SPA_SHELL_STREAMS)),
            media_streams: Arc::new(Semaphore::new(MAX_MEDIA_STREAMS)),
        })
    }

    /// Reserva capacidade para uma tarefa que pode sobreviver ao timeout HTTP.
    ///
    /// O permit deve ser movido para a closure de `spawn_blocking`, para continuar
    /// contabilizado até que o trabalho efetivamente termine.
    pub fn try_acquire_blocking_task(&self) -> Option<OwnedSemaphorePermit> {
        self.blocking_tasks.clone().try_acquire_owned().ok()
    }

    /// Reserva uma conexão da SPA até que o corpo HTTP seja consumido ou descartado.
    ///
    /// O shell recebe uma reserva própria para que clientes lentos baixando assets não o impeçam
    /// de carregar uma rota da aplicação.
    pub(crate) fn try_acquire_spa_stream(&self, shell: bool) -> Option<OwnedSemaphorePermit> {
        let streams = if shell {
            &self.spa_shell_streams
        } else {
            &self.spa_asset_streams
        };
        streams.clone().try_acquire_owned().ok()
    }

    /// Reserva um stream autenticado de mídia até que o corpo seja consumido ou descartado.
    pub(crate) fn try_acquire_media_stream(&self) -> Option<OwnedSemaphorePermit> {
        self.media_streams.clone().try_acquire_owned().ok()
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

/// Deriva hashes CSP apenas do shell que pode ser servido pela raiz configurada. Falhas de I/O
/// ou um shell ausente deixam a política sem hashes, preservando o fallback HTTP sem revelar
/// caminhos locais.
fn shell_inline_hashes(web_root: Option<&Path>) -> (Vec<String>, Vec<String>) {
    let Some(root) = web_root else {
        return (Vec::new(), Vec::new());
    };
    let shell = root.join("200.html");
    let metadata = match fs::symlink_metadata(&shell) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
        _ => return (Vec::new(), Vec::new()),
    };
    if metadata.len() > MAX_SPA_SHELL_SIZE {
        return (Vec::new(), Vec::new());
    }
    let source = match fs::read(shell) {
        Ok(source) => source,
        Err(_) => return (Vec::new(), Vec::new()),
    };
    (
        csp_hashes(inline_scripts(&source)),
        csp_hashes(inline_style_attributes(&source)),
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
        let Some(close_start) = find_ascii_case_insensitive(source, b"</script", open_end + 1)
        else {
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
            let Some(close_start) = find_ascii_case_insensitive(source, b"</script", end + 1)
            else {
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
            web_root: None,
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
