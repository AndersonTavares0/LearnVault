//! Fontes de curso remotas (WebDAV) e os endpoints administrativos que as mantêm.
//!
//! A configuração vive em `sources.json`, ao lado do banco, como o registro de cursos
//! locais. A senha nunca é gravada em claro: cada fonte guarda `v1:<ciphertext>` com a
//! chave de [`crate::secret`]. A API devolve apenas identidade, nome, URL, usuário e a
//! presença de senha; o segredo não aparece em respostas, logs nem mensagens de erro.
//!
//! A fonte é somente leitura: o cliente WebDAV só emite `PROPFIND` e `GET`.

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
use learnvault_core::{SourceError, scan_source};
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
    webdav::{SourceUrlError, WebDavClient, parse_source_url},
};

/// Nome do arquivo de configuração de fontes dentro do diretório de dados.
pub(crate) const SOURCES_FILE_NAME: &str = "sources.json";
/// Versão do formato do arquivo de configuração.
const SOURCES_VERSION: u32 = 1;
/// Limite de caracteres do nome apresentado.
const MAX_SOURCE_NAME_CHARS: usize = 120;
/// Tempo máximo para percorrer uma fonte remota.
const SOURCE_SCAN_TIMEOUT: Duration = Duration::from_secs(120);
/// Tempo máximo do teste de conexão.
const PROBE_TIMEOUT: Duration = Duration::from_secs(20);

/// Fonte de curso remota carregada em memória.
#[derive(Clone)]
pub(crate) struct RemoteSource {
    /// Identidade opaca usada como `library_id` e nos endpoints.
    pub(crate) id: String,
    /// Nome apresentado no catálogo.
    pub(crate) name: String,
    /// URL base normalizada do WebDAV.
    pub(crate) url: Url,
    /// Usuário de autenticação básica, quando houver.
    pub(crate) username: Option<String>,
    /// Senha em claro apenas em memória, nunca serializada.
    password: Option<String>,
    /// Cifra original, preservada para não perder o segredo quando a chave não carrega.
    stored_secret: Option<String>,
    /// Verdadeiro quando existe um segredo gravado que não pôde ser decifrado.
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
    /// Senha em claro para montar o cabeçalho de autenticação.
    pub(crate) fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// Verdadeiro quando a fonte tem usuário e senha utilizáveis.
    pub(crate) fn has_password(&self) -> bool {
        self.password.is_some()
    }
}

/// Entrada persistida. O segredo é o único campo cifrado.
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

/// Caminho do arquivo de fontes para um banco.
pub(crate) fn sources_path(database_path: &Path) -> PathBuf {
    let parent = database_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    parent.join(SOURCES_FILE_NAME)
}

/// Carrega as fontes persistidas.
///
/// Arquivo ausente, corrompido ou em versão desconhecida resulta em lista vazia, como no
/// registro de cursos. Entradas cuja URL é inválida são descartadas. Um segredo que não
/// pôde ser decifrado mantém a fonte, marcada para que a interface explique a falha em
/// vez de tentar autenticar sem senha.
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

/// Grava as fontes de forma serializada e atômica.
///
/// O temporário é criado exclusivamente e trocado por `rename`. Segredos em claro são
/// cifrados antes da gravação; um segredo que já estava gravado e não pôde ser decifrado
/// mantém a cifra original.
pub(crate) fn store_sources(
    path: &Path,
    key_path: &Path,
    sources: &[RemoteSource],
) -> Result<(), SourcesError> {
    let key = if sources.iter().any(|source| source.password.is_some()) {
        // Criar uma chave nova enquanto outra fonte ainda tem uma cifra órfã tornaria
        // essa cifra irrecuperável. O administrador precisa restaurar a chave ou remover
        // a fonte quebrada antes de gravar uma senha nova.
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

/// Erro sanitizado da configuração de fontes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourcesError {
    /// A gravação do arquivo falhou.
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

/// `GET /api/admin/sources` — lista fontes sem nunca devolver o segredo.
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

/// `POST /api/admin/sources` — registra uma fonte remota.
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

/// `DELETE /api/admin/sources/{id}` — remove a configuração sem tocar no servidor remoto.
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

/// `POST /api/admin/sources/{id}/test` — valida alcance e autenticação.
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
    let client = match WebDavClient::new(
        source.url.clone(),
        source.username.as_deref(),
        source.password(),
    ) {
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

/// `POST /api/admin/sources/{id}/scan` — descobre a árvore remota e aplica ao catálogo.
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
    let client = match WebDavClient::new(
        source.url.clone(),
        source.username.as_deref(),
        source.password(),
    ) {
        Ok(client) => client,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "invalid_source"),
    };

    // A rede acontece antes do lock do banco: uma fonte lenta não bloqueia as demais
    // operações de leitura do catálogo.
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
        learnvault_core::apply_source_scan(&mut database, &library_id, outcome)
    })
    .await;

    match blocking {
        Ok(Ok(summary)) => {
            private_no_store(Json(crate::catalog::RescanResponse::from(summary)).into_response())
        }
        // A falha da fonte já foi registrada no histórico pelo `apply_source_scan`; a
        // resposta devolve o motivo sanitizado em vez de um 500 genérico.
        Ok(Err(_)) => match failure {
            Some(error) => json_error(StatusCode::BAD_GATEWAY, error),
            None => internal_error("source_scan_failed"),
        },
        Err(_) => internal_error("source_scan_failed"),
    }
}

/// Motivo de recusa de uma sessão administrativa, traduzido só no handler.
enum AdminFailure {
    Unauthenticated,
    Forbidden,
    CredentialsRequired,
    Capacity,
}

async fn require_admin(
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

fn admin_failure_response(failure: AdminFailure) -> Response {
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
        SourceUrlError::InvalidUrl | SourceUrlError::MissingHost => "invalid_source_url",
        SourceUrlError::CredentialsInUrl => "source_url_contains_credentials",
        SourceUrlError::QueryOrFragment => "invalid_source_url",
    }
}

fn source_error_code(error: SourceError) -> &'static str {
    match error {
        SourceError::Unreachable => "unreachable",
        SourceError::Denied => "denied",
        SourceError::NotFound => "not_found",
        SourceError::InvalidResponse => "invalid_response",
        SourceError::Limit => "limit_exceeded",
    }
}

fn json_error(status: StatusCode, error: &'static str) -> Response {
    private_no_store((status, Json(ErrorBody { error })).into_response())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        RemoteSource, SourceResponse, SourcesError, SourcesResponse, load_sources, sources_path,
        store_sources,
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

        // Remover outra fonte não pode apagar a cifra que ainda não foi decifrada.
        store_sources(&path, &key_path, &loaded).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("v1:"));

        // Gravar uma senha nova criaria outra chave e perderia a cifra órfã.
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
}
