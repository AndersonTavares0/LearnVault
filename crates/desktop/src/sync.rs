//! Fila offline e cliente de sincronização do desktop.
//!
//! Cada escrita sincronizável grava no banco local e entra no outbox (`sync_outbox`) na
//! mesma transação, então um crash nunca deixa a escrita local sem a operação
//! correspondente. A operação mantém o mesmo `op_id` entre tentativas: reenviar o que o
//! servidor já aceitou é uma duplicata e não repete o efeito.
//!
//! O cliente fala apenas com o servidor configurado: `POST /api/login`,
//! `POST /api/logout`, `POST /api/sync/push` e `GET /api/sync/pull`. A sessão é o cookie
//! `learnvault_session`, guardado em `sync.json` no diretório de dados do aplicativo com
//! permissão `0600` no Unix. O cookie nunca é serializado para o webview, nunca aparece
//! em log nem em mensagem de erro.
//!
//! A URL do servidor é validada antes de qualquer requisição: somente `http`/`https`, com
//! host, sem credenciais embutidas, sem query, sem fragmento e sem caminho. Redireciona-
//! mentos são desligados para que o cookie nunca siga para outra origem. Cada requisição
//! tem timeout; erros de transporte e 5xx são repetidos com espera crescente e limitada,
//! enquanto 401 e 403 (`credentials_change_required`) falham imediatamente.
//!
//! Uma sincronização empurra o outbox em páginas, só confirma (`acknowledge`) depois que
//! o servidor aceita e então puxa do cursor até `has_more` ser falso. O cursor e os
//! efeitos entram no mesmo commit (`apply_operations`), então uma execução interrompida
//! retoma do ponto anterior sem duplicar nem perder operações. Duas execuções nunca se
//! sobrepõem: um `SyncFlight` garante execução única.

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

/// Nome do arquivo de configuração e sessão dentro do diretório de dados.
pub const SYNC_FILE_NAME: &str = "sync.json";

/// Versão do formato do arquivo de sincronização.
const SYNC_FILE_VERSION: u32 = 1;

/// Orçamento de cada requisição HTTP. Estar offline não pode prender a interface.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Espera entre tentativas de transporte/5xx (a primeira tentativa não espera).
const RETRY_DELAYS_MS: [u64; 3] = [0, 250, 750];

/// Nome do cookie de sessão do servidor (`auth::COOKIE_NAME`).
const SESSION_COOKIE_NAME: &str = "learnvault_session";

/// Página de leitura do diário remoto.
const PULL_LIMIT: usize = 200;

/// Falha estável de sincronização, traduzida para um código curto no resumo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncFailure {
    /// Servidor ou sessão ainda não configurados.
    NotConfigured,
    /// A sessão expirou (401); entra de novo.
    Unauthorized,
    /// O servidor exige troca de credenciais (403).
    CredentialsRequired,
    /// O servidor recusou a requisição (outro 4xx).
    Rejected,
    /// Falha de transporte, timeout ou 5xx depois das tentativas.
    Unavailable,
    /// Falha local do banco.
    Local,
}

impl SyncFailure {
    /// Código estável consumido pela interface.
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

/// Configuração persistida: servidor e sessão.
///
/// O `Debug` é manual para que a sessão nunca apareça em diagnóstico.
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SyncSettings {
    /// Versão do arquivo, para descartar um formato desconhecido.
    pub version: u32,
    /// URL base do servidor, já normalizada.
    pub server_url: String,
    /// Cookie de sessão no formato `nome=valor`; vazio quando não há sessão.
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
    /// Indica que existe um servidor escolhido.
    pub fn configured(&self) -> bool {
        !self.server_url.trim().is_empty()
    }

    /// Indica que existe uma sessão guardada.
    pub fn signed_in(&self) -> bool {
        !self.session.is_empty()
    }
}

/// Lê a configuração do diretório de dados; ausente, corrompida ou em versão desconhecida
/// vale como não configurada.
pub fn load_settings(data_dir: &Path) -> SyncSettings {
    fs::read(data_dir.join(SYNC_FILE_NAME))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SyncSettings>(&bytes).ok())
        .filter(|settings| settings.version == SYNC_FILE_VERSION)
        .unwrap_or_default()
}

/// Grava a configuração de forma atômica e com permissão `0600` no Unix.
///
/// Uma falha de gravação não altera o arquivo anterior. No Windows não há modo POSIX; o
/// arquivo herda a ACL do diretório de dados do aplicativo.
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

/// Valida e normaliza a URL base do servidor.
///
/// Somente `http`/`https`, com host explícito, sem credenciais embutidas, query,
/// fragmento ou caminho. A normalização remove a barra final para concatenar `/api/...`.
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

/// Autentica no servidor e devolve o cookie de sessão para o chamador guardar.
///
/// A senha só existe nesta chamada; o cookie devolvido nunca é registrado em log.
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

/// Encerra a sessão no servidor. Falha de rede é ignorada: o chamador apaga a cópia local
/// de qualquer forma.
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

/// Executa uma sincronização completa: empurra o outbox, confirma o aceite e puxa do
/// cursor até a última página.
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
        // Um servidor que repete a mesma página não pode prender o cliente em loop.
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

/// Estado local do dono: dispositivo e cursor já aplicado.
pub fn state(database: &Database) -> Result<SyncState, IpcError> {
    learnvault_core::sync_state(database, LOCAL_OWNER_ID).map_err(|_| IpcError::sync_unavailable())
}

/// Identidade estável deste dispositivo.
pub(crate) fn device(database: &Database) -> Result<String, IpcError> {
    state(database).map(|state| state.device_id)
}

/// Operações do outbox ainda não confirmadas, da mais antiga para a mais nova.
pub fn pending(database: &Database, limit: usize) -> Result<Vec<SyncOperationInput>, IpcError> {
    learnvault_core::pending_operations(database, LOCAL_OWNER_ID, limit)
        .map_err(|_| IpcError::sync_unavailable())
}

/// Quantidade de operações ainda pendentes.
pub fn pending_count(database: &Database) -> Result<i64, IpcError> {
    learnvault_core::pending_count(database, LOCAL_OWNER_ID)
        .map_err(|_| IpcError::sync_unavailable())
}

/// Remove do outbox as operações que o servidor já aceitou.
pub fn acknowledge(database: &Database, op_ids: &[String]) -> Result<usize, IpcError> {
    learnvault_core::acknowledge_operations(database, LOCAL_OWNER_ID, op_ids)
        .map_err(|_| IpcError::sync_unavailable())
}

/// Aplica as operações recebidas e grava o cursor na mesma transação.
pub fn apply(
    database: &Database,
    operations: &[SyncOperation],
    cursor: i64,
) -> Result<usize, IpcError> {
    learnvault_core::apply_operations(database, LOCAL_OWNER_ID, operations, cursor)
        .map_err(|_| IpcError::sync_unavailable())
}

/// Garante execução única: a segunda chamada devolve `None` enquanto a primeira vive.
#[derive(Debug, Default)]
pub struct SyncFlight {
    running: AtomicBool,
}

impl SyncFlight {
    /// Marca uma execução como em andamento.
    pub fn try_begin(&self) -> Option<SyncGuard<'_>> {
        self.running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| SyncGuard(&self.running))
    }

    /// Indica se existe uma execução em andamento.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Execução em andamento; libera o `SyncFlight` ao ser descartada.
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
        // Sem redirecionamentos: o cookie de sessão nunca segue para outra origem.
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

/// Extrai apenas o par `learnvault_session=...` de um `Set-Cookie`.
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

/// Caminho do arquivo de configuração dentro do diretório de dados.
pub fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SYNC_FILE_NAME)
}
