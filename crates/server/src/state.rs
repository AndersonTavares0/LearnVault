use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use learnvault_core::{AuthSession, AuthUser, Database, Error as CoreError};
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::{
    Config,
    auth::hash_password,
    courses::{RegisteredCourse, legacy_seed, load_registry, registry_path},
};

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
    /// Cursos registrados, na ordem de importação. A leitura é rápida (snapshot) e a
    /// escrita acontece apenas na importação/remoção, fora das threads Tokio.
    pub(crate) courses: Arc<RwLock<Vec<RegisteredCourse>>>,
    /// Arquivo versionado do registro de cursos, ao lado do banco.
    pub(crate) courses_file: PathBuf,
    pub(crate) database: Arc<Mutex<Database>>,
    pub(crate) login_attempts: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    pub(crate) rescan_lock: Arc<Mutex<()>>,
    pub(crate) inline_script_hashes: Arc<[String]>,
    pub(crate) inline_style_hashes: Arc<[String]>,
    spa_shell: Option<Arc<SpaShell>>,
    spa_shell_too_large: bool,
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
        let spa_shell_too_large = shell_too_large(config.web_root.as_deref());
        let spa_shell = load_spa_shell(config.web_root.as_deref()).map(Arc::new);
        let (inline_script_hashes, inline_style_hashes) = shell_hashes(spa_shell.as_deref());
        let courses_file = registry_path(&config.database_path);
        Self {
            inline_script_hashes: inline_script_hashes.into(),
            inline_style_hashes: inline_style_hashes.into(),
            spa_shell,
            spa_shell_too_large,
            courses: Arc::new(RwLock::new(Vec::new())),
            courses_file,
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

    /// Abre o banco, carrega o registro de cursos e executa o bootstrap administrativo
    /// fora das threads Tokio.
    pub async fn initialize(config: Config) -> Result<Self, StateError> {
        let setup = config.clone();
        let (database, spa_shell, spa_shell_too_large, courses, courses_file) =
            tokio::task::spawn_blocking(move || {
                let shell_too_large = shell_too_large(setup.web_root.as_deref());
                let shell = load_spa_shell(setup.web_root.as_deref()).map(Arc::new);
                if let Some(parent) = setup
                    .database_path
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                {
                    std::fs::create_dir_all(parent).map_err(|_| StateError::DatabaseDirectory)?;
                }
                // O registro é carregado ao lado do banco; a raiz semeada só entra em cena
                // quando nenhum registro foi persistido ainda, e nunca é regravada aqui:
                // uma remoção explícita do curso semente não volta no próximo start.
                let courses_file = registry_path(&setup.database_path);
                let mut courses = load_registry(&courses_file);
                if !courses_file.exists()
                    && let Some(seed) = setup.library_root.as_deref().and_then(legacy_seed)
                {
                    courses.push(seed);
                }
                let mut database =
                    Database::open(&setup.database_path).map_err(StateError::Database)?;
                let existing =
                    learnvault_core::has_admin(&database).map_err(StateError::Database)?;
                if existing {
                    return Ok((database, shell, shell_too_large, courses, courses_file));
                }
                let using_temporary_default =
                    setup.admin_username.is_none() && setup.admin_password.is_none();
                let username = setup.admin_username.as_deref().unwrap_or("admin");
                let password = setup.admin_password.as_deref().unwrap_or("admin");
                crate::auth::validate_username(username)
                    .map_err(|_| StateError::InvalidAdministrator)?;
                // A senha temporária não força troca após o login: a troca acontece
                // em "Configurações → Conta", autenticada pela senha atual.
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
                    false,
                )
                .map_err(StateError::Database)?;
                Ok((database, shell, shell_too_large, courses, courses_file))
            })
            .await
            .map_err(StateError::Blocking)??;
        let (inline_script_hashes, inline_style_hashes) = shell_hashes(spa_shell.as_deref());
        Ok(Self {
            config,
            courses: Arc::new(RwLock::new(courses)),
            courses_file,
            database: Arc::new(Mutex::new(database)),
            login_attempts: Arc::new(Mutex::new(HashMap::new())),
            rescan_lock: Arc::new(Mutex::new(())),
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

    /// Fotografia imutável dos cursos registrados.
    ///
    /// Uma leitura de catálogo, rescan ou mídia usa a fotografia e nunca segura o cadeado
    /// de escrita enquanto lê arquivos ou o banco.
    pub(crate) fn courses_snapshot(&self) -> Vec<RegisteredCourse> {
        self.courses
            .read()
            .map(|courses| courses.clone())
            .unwrap_or_default()
    }

    /// Reserva capacidade para uma tarefa que pode sobreviver ao timeout HTTP.
    ///
    /// O permit deve ser movido para a closure de `spawn_blocking`, para continuar
    /// contabilizado até que o trabalho efetivamente termine.
    pub fn try_acquire_blocking_task(&self) -> Option<OwnedSemaphorePermit> {
        self.blocking_tasks.clone().try_acquire_owned().ok()
    }

    /// Peek barato de capacidade para evitar segurar mutexes quando o servidor
    /// já está saturado. É apenas uma verificação (sujeita a corrida); a
    /// aquisição real continua via `try_acquire_*` após os locks.
    pub(crate) fn has_blocking_capacity(&self) -> bool {
        self.blocking_tasks.available_permits() > 0
    }

    /// Verifica a prontidão do banco sem expor o erro de persistência ao cliente.
    ///
    /// O health check não espera por uma operação que já esteja usando o banco:
    /// nesse caso o processo ainda está vivo, mas a dependência não está pronta
    /// para responder de forma determinística.
    pub(crate) async fn database_ready(&self) -> bool {
        let database = match self.database.clone().try_lock_owned() {
            Ok(database) => database,
            Err(_) => return false,
        };
        let blocking_task = match self.try_acquire_blocking_task() {
            Some(blocking_task) => blocking_task,
            None => return false,
        };
        tokio::task::spawn_blocking(move || {
            let _blocking_task = blocking_task;
            database.has_current_schema().unwrap_or(false)
        })
        .await
        .unwrap_or(false)
    }

    /// Reserva uma conexão da SPA até que o corpo HTTP seja consumido, descartado ou fique ocioso.
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

    /// Aguarda (sem bloquear o executor) até uma reserva de stream de asset da SPA
    /// ficar disponível, devolvendo o permit.
    ///
    /// É o irmão *aguardável* de [`Self::try_acquire_spa_stream`]: em vez de falhar
    /// de imediato, registra no semáforo e resolve exatamente quando o watchdog
    /// ocioso devolve a reserva. Exposto para que o teste de integração observe de
    /// forma determinística a liberação de capacidade pelo watchdog, em vez de
    /// assumir que um avanço fixo do relógio virtual basta para o produtor alcançar
    /// o deadline depois do IO de leitura real (issue #154). A admissão de `serve`
    /// continua usando `try_acquire_spa_stream`; nenhuma lógica de backpressure muda.
    #[doc(hidden)]
    pub async fn acquire_spa_asset_stream_permit(&self) -> OwnedSemaphorePermit {
        self.spa_asset_streams
            .clone()
            .acquire_owned()
            .await
            .expect("o semáforo de streams de asset da SPA nunca é fechado")
    }

    /// Reserva um stream autenticado de mídia até que o corpo seja consumido ou descartado.
    pub(crate) fn try_acquire_media_stream(&self) -> Option<OwnedSemaphorePermit> {
        self.media_streams.clone().try_acquire_owned().ok()
    }

    /// Retorna o shell imutável carregado na inicialização apenas se o arquivo configurado ainda
    /// tiver os mesmos bytes. Assim, uma troca de `200.html` não pode ser servida com hashes CSP
    /// antigos; o deploy deve reiniciar o processo para publicar um novo shell.
    pub(crate) fn current_spa_shell(&self) -> Option<Arc<[u8]>> {
        let shell = self.spa_shell.as_ref()?;
        (load_spa_shell(self.config.web_root.as_deref()).as_ref()? == shell.as_ref())
            .then(|| shell.bytes.clone())
    }

    /// O tamanho do shell é validado na inicialização, antes de disputar a capacidade blocking.
    /// Isso preserva o 413 para uma configuração inválida mesmo sob saturação operacional.
    pub(crate) fn spa_shell_is_too_large(&self) -> bool {
        self.spa_shell_too_large
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
        let mut database = self.database.clone().lock_owned().await;
        let blocking_task = self
            .try_acquire_blocking_task()
            .ok_or(StateError::Capacity)?;
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

/// Carrega o shell pelo mesmo conteúdo que é usado para os hashes CSP. Falhas de I/O ou um
/// shell ausente deixam a política sem hashes, preservando o fallback HTTP sem revelar caminhos
/// locais. A raiz web precisa permanecer sem escrita não confiável: essa verificação detecta
/// alterações entre requisições, mas não substitui contenção atômica contra um processo hostil.
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
            // O nome e todo separador que o sucede já foram consumidos. Manter `offset` nesse
            // ponto garante que atributos booleanos (por exemplo `hidden` e `defer`) avancem
            // para o próximo atributo em vez de reprocessar o nome atual.
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

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, time::Instant};
    use tempfile::tempdir;

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
    async fn missing_bootstrap_configuration_creates_temporary_admin_admin() {
        let directory = tempdir().unwrap();
        let mut setup = config();
        setup.database_path = directory.path().join("learnvault.sqlite");
        let state = AppState::initialize(setup).await.unwrap();
        let user = state.user("admin".to_owned()).await.unwrap().unwrap();
        assert_eq!(user.username, "admin");
        assert!(user.is_admin);
        // A senha temporária não é trocada à força após o login: a configuração
        // acontece em "Configurações → Conta", com a senha atual como verificação.
        assert!(!user.must_change_credentials);
        assert!(crate::verify_password("admin", &user.password_hash).unwrap());
        assert_ne!(user.password_hash, "admin");
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
