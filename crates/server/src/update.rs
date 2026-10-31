//! Verificação de atualização do LearnVault.
//!
//! A checagem acontece **no servidor local**: o processo consulta a API pública de
//! releases do GitHub por HTTPS e devolve o resultado à SPA, que apenas o exibe. Nada é
//! baixado, verificado por checksum ou instalado automaticamente — o produto avisa e
//! oferece o link da release, e a atualização continua manual, como descrito em
//! `docs/operations/distribution.md`.
//!
//! O destino é fixo: somente HTTPS, host constante no código e sem nenhuma URL vinda da
//! requisição (o que evita SSRF). Falhas de rede nunca viram erro HTTP: a resposta
//! continua `200`, com `available: false`, `latest: null` e um `error` curto em pt-BR,
//! para que estar offline não quebre nem trave a tela de Configurações.
//!
//! Checagens externas respeitam um intervalo mínimo em processo (padrão de uma hora) e
//! requisições concorrentes são coalescidas em um único `fetch`. O botão manual usa
//! `?force=1`, que ignora o intervalo mínimo.

use std::{future::Future, pin::Pin, sync::Arc, sync::OnceLock, time::Duration};

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::Instant;

use crate::{AppState, VERSION};

/// Endpoint público de release do repositório. Constante: nenhuma URL vem da requisição.
pub(crate) const RELEASES_URL: &str =
    "https://api.github.com/repos/AndersonTavares0/LearnVault/releases/latest";

/// Intervalo mínimo entre checagens externas quando o cliente não força uma nova.
pub(crate) const DEFAULT_CACHE_INTERVAL: Duration = Duration::from_secs(3600);

/// Orçamento de rede de cada checagem. Estar offline não pode segurar a tela.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Limite defensivo do JSON da API de releases.
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Motivo genérico quando não há um erro mais específico do último attempt.
const FALLBACK_ERROR: &str = "não foi possível verificar a atualização";

/// Release publicado, reduzido aos campos que a decisão consome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReleaseInfo {
    pub(crate) tag_name: String,
    pub(crate) published_at: Option<String>,
    pub(crate) draft: bool,
    pub(crate) prerelease: bool,
    pub(crate) changelog: Option<String>,
}

/// Busca a release mais recente. Devolve um motivo curto e sanitizado em pt-BR ao falhar.
///
/// É um closure injetável para que os testes exercitem parsing, decisão, cache, coerção e
/// erro sem tocar a rede.
pub(crate) type Fetcher = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<ReleaseInfo, String>> + Send>> + Send + Sync,
>;

/// Resposta do endpoint `/api/update`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateResponse {
    current: &'static str,
    enabled: bool,
    available: bool,
    latest: Option<String>,
    released_at: Option<String>,
    url: Option<String>,
    changelog: Option<String>,
    checked_at: Option<String>,
    error: Option<String>,
}

/// Resultado de uma checagem bem-sucedida mantido em memória.
#[derive(Clone, Debug)]
struct Success {
    available: bool,
    latest: Option<String>,
    released_at: Option<String>,
    url: Option<String>,
    changelog: Option<String>,
    /// Instante da checagem em segundos desde a época Unix, como string opaca.
    checked_at: String,
    checked_instant: Instant,
}

#[derive(Debug, Default)]
struct Cache {
    success: Option<Success>,
    last_attempt: Option<Instant>,
    last_error: Option<String>,
}

/// Checador em processo: cache do último sucesso, coerção de concorrentes e limite de
/// frequência entre chamadas externas.
#[derive(Clone)]
pub(crate) struct UpdateChecker {
    enabled: bool,
    interval: Duration,
    fetch: Fetcher,
    cache: Arc<Mutex<Cache>>,
}

impl std::fmt::Debug for UpdateChecker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UpdateChecker")
            .field("enabled", &self.enabled)
            .field("interval", &self.interval)
            .finish_non_exhaustive()
    }
}

impl UpdateChecker {
    /// Cria o checador com o intervalo padrão.
    pub(crate) fn new(enabled: bool, fetch: Fetcher) -> Self {
        Self::with_interval(enabled, DEFAULT_CACHE_INTERVAL, fetch)
    }

    fn with_interval(enabled: bool, interval: Duration, fetch: Fetcher) -> Self {
        Self {
            enabled,
            interval,
            fetch,
            cache: Arc::new(Mutex::new(Cache::default())),
        }
    }

    /// Estado atual da verificação, disparando uma checagem externa se necessário.
    pub(crate) async fn snapshot(&self, force: bool) -> UpdateResponse {
        if !self.enabled {
            return UpdateResponse::disabled();
        }
        // O instante de chegada é capturado antes do cadeado: uma checagem concluída
        // enquanto esta requisição esperava já responde por ela (coerção/single-flight).
        let arrival = Instant::now();
        let mut cache = self.cache.lock().await;
        if let Some(success) = cache.success.clone() {
            let completed_after_arrival = success.checked_instant >= arrival;
            let fresh = arrival.saturating_duration_since(success.checked_instant) < self.interval;
            if completed_after_arrival || (!force && fresh) {
                return UpdateResponse::from_success(success);
            }
        }
        if !force
            && let Some(last_attempt) = cache.last_attempt
            && arrival.saturating_duration_since(last_attempt) < self.interval
        {
            return UpdateResponse::error(
                cache
                    .last_error
                    .clone()
                    .unwrap_or_else(|| FALLBACK_ERROR.to_owned()),
            );
        }
        cache.last_attempt = Some(Instant::now());
        match (self.fetch)().await {
            Ok(release) => match evaluate(&release, VERSION) {
                Ok(evaluation) => {
                    let success = success_from(&release, evaluation, checked_at_now());
                    cache.success = Some(success.clone());
                    cache.last_error = None;
                    UpdateResponse::from_success(success)
                }
                Err(message) => {
                    cache.last_error = Some(message.clone());
                    UpdateResponse::error(message)
                }
            },
            Err(message) => {
                cache.last_error = Some(message.clone());
                UpdateResponse::error(message)
            }
        }
    }
}

/// Consulta autenticada do estado da atualização.
///
/// Usa a mesma proteção das rotas de estudo: sessão válida e sem troca de credenciais
/// pendente. A resposta é sempre `private, no-store`.
pub(crate) async fn update(
    State(state): State<AppState>,
    Query(query): Query<UpdateQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(failure) = crate::study::user_or_response(&state, &headers).await {
        return failure.into_response();
    }
    private_no_store(Json(state.update.snapshot(query.force()).await).into_response())
}

/// Entrada de histórico de atualização (gerada pelo script autônomo de atualização).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UpdateHistoryEntry {
    pub(crate) timestamp: String,
    pub(crate) status: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) message: String,
}

/// Resposta da listagem do histórico de atualizações.
#[derive(Debug, Serialize)]
pub(crate) struct UpdateHistoryResponse {
    pub(crate) items: Vec<UpdateHistoryEntry>,
}

/// Consulta autenticada do histórico de atualizações registradas no volume de dados.
pub(crate) async fn update_history(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(failure) = crate::study::user_or_response(&state, &headers).await {
        return failure.into_response();
    }
    let data_dir = state
        .config
        .database_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let history_file = data_dir.join("update-history.json");
    let items = if history_file.is_file() {
        match std::fs::read_to_string(&history_file) {
            Ok(content) => {
                serde_json::from_str::<Vec<UpdateHistoryEntry>>(&content).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };
    private_no_store(Json(UpdateHistoryResponse { items }).into_response())
}

/// Consulta do endpoint. `force=1` (ou `force=true`) ignora o intervalo mínimo de cache.
#[derive(Debug, Deserialize)]
pub(crate) struct UpdateQuery {
    force: Option<String>,
}

impl UpdateQuery {
    fn force(&self) -> bool {
        matches!(
            self.force.as_deref(),
            Some(value) if value.eq_ignore_ascii_case("1") || value.eq_ignore_ascii_case("true")
        )
    }
}

impl UpdateResponse {
    fn disabled() -> Self {
        Self {
            current: VERSION,
            enabled: false,
            available: false,
            latest: None,
            released_at: None,
            url: None,
            changelog: None,
            checked_at: None,
            error: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            current: VERSION,
            enabled: true,
            available: false,
            latest: None,
            released_at: None,
            url: None,
            changelog: None,
            checked_at: None,
            error: Some(message),
        }
    }

    fn from_success(success: Success) -> Self {
        Self {
            current: VERSION,
            enabled: true,
            available: success.available,
            latest: success.latest,
            released_at: success.released_at,
            url: success.url,
            changelog: success.changelog,
            checked_at: Some(success.checked_at),
            error: None,
        }
    }
}

/// Decisão da checagem: uma versão publicada mais nova, ou nada a avisar.
#[derive(Debug, PartialEq, Eq)]
enum Evaluation {
    UpToDate,
    Available { latest: String },
}

fn success_from(release: &ReleaseInfo, evaluation: Evaluation, checked_at: String) -> Success {
    let latest = match evaluation {
        Evaluation::Available { latest } => Some(latest),
        Evaluation::UpToDate => None,
    };
    let available = latest.is_some();
    let url = if available {
        release_url(&release.tag_name)
    } else {
        None
    };
    Success {
        available,
        latest,
        // O link só faz sentido quando há uma versão nova; fora disso a resposta não
        // carrega a release mais recente nem a sua data.
        released_at: if available {
            release.published_at.clone()
        } else {
            None
        },
        url: if available { url } else { None },
        changelog: if available {
            release.changelog.clone()
        } else {
            None
        },
        checked_at,
        checked_instant: Instant::now(),
    }
}

/// Decide se a release publicada é mais nova que a versão em execução.
///
/// Rascunhos e pré-lançamentos são ignorados como se não existissem: o canal estável só
/// anuncia releases publicados.
fn evaluate(release: &ReleaseInfo, current: &str) -> Result<Evaluation, String> {
    if release.draft || release.prerelease {
        return Ok(Evaluation::UpToDate);
    }
    let latest =
        parse_version(&release.tag_name).ok_or_else(|| "versão de release inválida".to_owned())?;
    let current = parse_version(current).ok_or_else(|| "versão local inválida".to_owned())?;
    Ok(if latest > current {
        Evaluation::Available {
            latest: latest.original,
        }
    } else {
        Evaluation::UpToDate
    })
}

/// Interpreta o corpo JSON do endpoint de release do GitHub.
fn parse_release(body: &str) -> Result<ReleaseInfo, ()> {
    #[derive(Deserialize)]
    struct Payload {
        tag_name: Option<String>,
        #[serde(default)]
        published_at: Option<String>,
        #[serde(default)]
        draft: bool,
        #[serde(default)]
        prerelease: bool,
        #[serde(default)]
        body: Option<String>,
    }

    let payload: Payload = serde_json::from_str(body).map_err(|_| ())?;
    let tag_name = payload
        .tag_name
        .filter(|tag| !tag.trim().is_empty())
        .ok_or(())?;
    let changelog = payload.body.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.len() > 16 * 1024 {
            let mut truncated = trimmed[..16 * 1024].to_string();
            truncated.push_str("\n\n...(conteúdo truncado)");
            Some(truncated)
        } else {
            Some(trimmed.to_string())
        }
    });
    Ok(ReleaseInfo {
        tag_name,
        published_at: payload.published_at,
        draft: payload.draft,
        prerelease: payload.prerelease,
        changelog,
    })
}

/// Versão com os componentes numéricos e o pré-lançamento opcional.
///
/// Comparação própria: numérica por componente e, pela regra do SemVer, um
/// pré-lançamento sempre precede o lançamento correspondente.
#[derive(Clone, Debug)]
struct Version {
    /// Texto normalizado (sem `v` inicial) usado na mensagem ao usuário.
    original: String,
    release: Vec<u64>,
    pre: Option<Vec<PreId>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PreId {
    Number(u64),
    Text(String),
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let length = self.release.len().max(other.release.len());
        for index in 0..length {
            let left = self.release.get(index).copied().unwrap_or(0);
            let right = other.release.get(index).copied().unwrap_or(0);
            match left.cmp(&right) {
                std::cmp::Ordering::Equal => {}
                order => return order,
            }
        }
        match (&self.pre, &other.pre) {
            (None, None) => std::cmp::Ordering::Equal,
            // Um lançamento tem precedência sobre o seu pré-lançamento.
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (Some(_), None) => std::cmp::Ordering::Less,
            (Some(left), Some(right)) => compare_pre(left, right),
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Version {}

fn compare_pre(left: &[PreId], right: &[PreId]) -> std::cmp::Ordering {
    for (a, b) in left.iter().zip(right.iter()) {
        let order = match (a, b) {
            (PreId::Number(a), PreId::Number(b)) => a.cmp(b),
            // Identificador numérico sempre precede um alfanumérico.
            (PreId::Number(_), PreId::Text(_)) => std::cmp::Ordering::Less,
            (PreId::Text(_), PreId::Number(_)) => std::cmp::Ordering::Greater,
            (PreId::Text(a), PreId::Text(b)) => a.cmp(b),
        };
        if order != std::cmp::Ordering::Equal {
            return order;
        }
    }
    // Um conjunto maior de identificadores tem precedência quando o prefixo é igual.
    left.len().cmp(&right.len())
}

/// Interpreta SemVer estrito aceitando `v` inicial opcional (convenção das tags GitHub).
fn parse_version(value: &str) -> Option<Version> {
    let value = value.strip_prefix('v').unwrap_or(value);
    let (without_build, build) = match value.split_once('+') {
        Some((left, right)) => (left, Some(right)),
        None => (value, None),
    };
    if let Some(build) = build
        && !valid_identifiers(build, false)
    {
        return None;
    }
    let (core, pre) = match without_build.split_once('-') {
        Some((core, pre)) => (core, Some(pre)),
        None => (without_build, None),
    };
    if core.is_empty() || core.contains('-') || core.split('.').count() != 3 {
        return None;
    }
    let mut release = Vec::new();
    for component in core.split('.') {
        if component.is_empty()
            || (component.len() > 1 && component.starts_with('0'))
            || !component.bytes().all(|byte| byte.is_ascii_digit())
        {
            return None;
        }
        release.push(component.parse::<u64>().ok()?);
    }
    let pre = match pre {
        // `1.0.0-` não é um pré-lançamento válido.
        Some("") => return None,
        Some(value) => Some(parse_pre(value)?),
        None => None,
    };
    Some(Version {
        original: without_build
            .strip_prefix('v')
            .unwrap_or(without_build)
            .to_owned(),
        release,
        pre,
    })
}

fn parse_pre(value: &str) -> Option<Vec<PreId>> {
    if !valid_identifiers(value, true) {
        return None;
    }
    let mut identifiers = Vec::new();
    for identifier in value.split('.') {
        identifiers.push(if identifier.bytes().all(|byte| byte.is_ascii_digit()) {
            PreId::Number(identifier.parse::<u64>().ok()?)
        } else {
            PreId::Text(identifier.to_owned())
        });
    }
    Some(identifiers)
}

fn valid_identifiers(value: &str, reject_numeric_leading_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && !(reject_numeric_leading_zero
                    && identifier.len() > 1
                    && identifier.bytes().all(|byte| byte.is_ascii_digit())
                    && identifier.starts_with('0'))
        })
}

fn release_url(version: &str) -> Option<String> {
    let parsed = parse_version(version)?;
    if parsed.pre.is_some() {
        return None;
    }
    Some(format!(
        "https://github.com/AndersonTavares0/LearnVault/releases/tag/{version}"
    ))
}

fn append_body_chunk(body: &mut Vec<u8>, chunk: &[u8]) -> Result<(), ()> {
    if body
        .len()
        .checked_add(chunk.len())
        .filter(|size| *size <= MAX_RESPONSE_BYTES)
        .is_none()
    {
        return Err(());
    }
    body.extend_from_slice(chunk);
    Ok(())
}

/// Instante da checagem como segundos desde a época Unix (string opaca para o cliente).
fn checked_at_now() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}

/// Cliente HTTPS compartilhado. Construído na primeira checagem, já dentro do runtime.
fn github_client() -> Result<&'static reqwest::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::Client, ()>> = OnceLock::new();
    let slot = CLIENT.get_or_init(|| {
        // O provedor `ring` evita o `aws-lc-rs` (que exigiria CMake para compilar) e
        // mantém o binário sem OpenSSL.
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| ())
    });
    slot.as_ref()
        .map_err(|_| "cliente HTTP indisponível".to_owned())
}

/// Fetcher real: uma requisição GET fixa à API de releases do GitHub.
pub(crate) fn github_fetcher() -> Fetcher {
    Arc::new(|| {
        Box::pin(async {
            if VERSION == "0.0.0" {
                return Err("versão local indisponível para comparação".to_owned());
            }
            let client = github_client()?;
            let response = client
                .get(RELEASES_URL)
                .header("accept", "application/vnd.github+json")
                .header("user-agent", format!("learnvault/{VERSION}"))
                .send()
                .await
                .map_err(|_| "não foi possível contatar o GitHub".to_owned())?;
            if !response.status().is_success() {
                return Err("o GitHub recusou a consulta de release".to_owned());
            }
            if response
                .content_length()
                .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
            {
                return Err("resposta do GitHub excede o limite permitido".to_owned());
            }
            let mut response = response;
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| "não foi possível ler a resposta do GitHub".to_owned())?
            {
                append_body_chunk(&mut body, &chunk)
                    .map_err(|()| "resposta do GitHub excede o limite permitido".to_owned())?;
            }
            let body = std::str::from_utf8(&body)
                .map_err(|_| "resposta de release inválida".to_owned())?;
            parse_release(body).map_err(|()| "resposta de release inválida".to_owned())
        })
    })
}

fn private_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn release(tag: &str) -> ReleaseInfo {
        ReleaseInfo {
            tag_name: tag.to_owned(),
            published_at: Some("2026-10-17T12:00:00Z".to_owned()),
            draft: false,
            prerelease: false,
            changelog: None,
        }
    }

    /// Fetcher controlado pelo teste: conta chamadas e devolve um resultado roteirizado.
    fn counting_fetcher(
        calls: Arc<AtomicUsize>,
        results: Vec<Result<ReleaseInfo, String>>,
    ) -> Fetcher {
        let results = Arc::new(Mutex::new(results));
        Arc::new(move || {
            let calls = calls.clone();
            let results = results.clone();
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                results.lock().await.remove(0)
            })
        })
    }

    fn ok_fetcher(calls: Arc<AtomicUsize>, tag: &str) -> Fetcher {
        counting_fetcher(calls, vec![Ok(release(tag))])
    }

    #[test]
    fn parses_a_published_release_payload() {
        let parsed = parse_release(
            "{\"tag_name\":\"v0.4.0\",\"published_at\":\"2026-10-17T12:00:00Z\",\"html_url\":\"https://attacker.test/\",\"draft\":false,\"prerelease\":false,\"body\":\"### Funcionalidades\\n- Novo recurso\"}",
        )
        .unwrap();
        assert_eq!(parsed.tag_name, "v0.4.0");
        assert_eq!(parsed.published_at.as_deref(), Some("2026-10-17T12:00:00Z"));
        assert!(!parsed.draft && !parsed.prerelease);
        assert_eq!(
            parsed.changelog.as_deref(),
            Some("### Funcionalidades\n- Novo recurso")
        );
    }

    #[test]
    fn parses_a_minimal_release_payload_without_optional_fields() {
        let parsed = parse_release(r#"{"tag_name":"0.4.0"}"#).unwrap();
        assert_eq!(parsed.tag_name, "0.4.0");
        assert_eq!(parsed.published_at, None);
    }

    #[test]
    fn rejects_a_release_payload_without_a_usable_tag() {
        assert!(parse_release("{}").is_err());
        assert!(parse_release(r#"{"tag_name":"  "}"#).is_err());
        assert!(parse_release(r#"{"tag_name":42}"#).is_err());
        assert!(parse_release("not json").is_err());
    }

    #[test]
    fn compares_versions_numerically_with_an_optional_leading_v() {
        assert!(parse_version("v0.4.0").unwrap() > parse_version("0.3.9").unwrap());
        assert!(parse_version("0.10.0").unwrap() > parse_version("0.9.0").unwrap());
        assert_eq!(
            parse_version("v1.2.3").unwrap(),
            parse_version("1.2.3").unwrap()
        );
        // Metadados de build não participam da comparação.
        assert_eq!(
            parse_version("1.2.3+build.7").unwrap(),
            parse_version("1.2.3").unwrap()
        );
    }

    #[test]
    fn treats_a_prerelease_as_older_than_its_release() {
        assert!(parse_version("1.0.0-rc.1").unwrap() < parse_version("1.0.0").unwrap());
        assert!(parse_version("1.0.0").unwrap() > parse_version("1.0.0-rc.1").unwrap());
        assert!(parse_version("1.0.0-rc.2").unwrap() > parse_version("1.0.0-rc.1").unwrap());
        assert!(parse_version("1.0.0-alpha.1").unwrap() < parse_version("1.0.0-beta").unwrap());
        assert!(parse_version("1.0.0-1").unwrap() < parse_version("1.0.0-alpha").unwrap());
    }

    #[test]
    fn rejects_unparsable_versions() {
        for value in [
            "",
            "v",
            "1.0.0-",
            "1.x",
            "abc",
            "1..2",
            "-1.0.0",
            "1.2",
            "1.2.3.4",
            " 1.2.3",
            "1.2.3 ",
            "01.2.3",
            "1.02.3",
            "1.2.03",
            "1.2.3-01",
            "1.2.3-alpha.01",
            "1.2.3-alpha..1",
            "1.2.3+build..1",
            "1.2.3+bad_thing",
            "1.2.3-ümlaut",
            "1.2.3+",
        ] {
            assert!(parse_version(value).is_none(), "{value}");
        }
        for value in ["1.2.3+001", "1.2.3-alpha.1", "1.2.3-rc.1+build.007"] {
            assert!(parse_version(value).is_some(), "{value}");
        }
    }

    #[test]
    fn release_link_is_constructed_only_for_the_official_repository() {
        assert_eq!(
            release_url("v1.2.3"),
            Some("https://github.com/AndersonTavares0/LearnVault/releases/tag/v1.2.3".to_owned())
        );
        assert_eq!(
            release_url("1.2.3"),
            Some("https://github.com/AndersonTavares0/LearnVault/releases/tag/1.2.3".to_owned())
        );
        assert_eq!(
            release_url("v1.2.3+build.7"),
            Some(
                "https://github.com/AndersonTavares0/LearnVault/releases/tag/v1.2.3+build.7"
                    .to_owned()
            )
        );
        assert_eq!(release_url("1.2.3-rc.1+build.7"), None);
        assert_eq!(release_url("https://attacker.test"), None);
    }

    #[test]
    fn preserves_the_original_release_tag_in_the_available_update_link() {
        let release = release("1.2.3");
        let result = success_from(
            &release,
            evaluate(&release, "1.0.0").unwrap(),
            "1".to_owned(),
        );

        assert_eq!(result.latest.as_deref(), Some("1.2.3"));
        assert_eq!(
            result.url.as_deref(),
            Some("https://github.com/AndersonTavares0/LearnVault/releases/tag/1.2.3")
        );
    }

    #[test]
    fn bounds_accumulated_github_response_body() {
        let mut body = Vec::new();
        assert!(append_body_chunk(&mut body, &vec![b'a'; MAX_RESPONSE_BYTES]).is_ok());
        assert_eq!(body.len(), MAX_RESPONSE_BYTES);
        assert!(append_body_chunk(&mut body, b"x").is_err());
        assert_eq!(body.len(), MAX_RESPONSE_BYTES);

        let mut body = Vec::new();
        assert!(append_body_chunk(&mut body, &vec![b'a'; MAX_RESPONSE_BYTES - 1]).is_ok());
        assert!(append_body_chunk(&mut body, b"ab").is_err());
        assert_eq!(body.len(), MAX_RESPONSE_BYTES - 1);
    }

    #[test]
    fn ignores_draft_and_prerelease_releases() {
        let draft = ReleaseInfo {
            draft: true,
            ..release("v9.9.9")
        };
        assert_eq!(evaluate(&draft, "0.1.0").unwrap(), Evaluation::UpToDate);
        let prerelease = ReleaseInfo {
            prerelease: true,
            ..release("v9.9.9-rc.1")
        };
        assert_eq!(
            evaluate(&prerelease, "0.1.0").unwrap(),
            Evaluation::UpToDate
        );
    }

    #[test]
    fn decides_availability_from_the_published_tag() {
        assert_eq!(
            evaluate(&release("v0.4.0"), "0.3.0").unwrap(),
            Evaluation::Available {
                latest: "0.4.0".to_owned()
            }
        );
        // Igual ou mais antiga não é atualização.
        assert_eq!(
            evaluate(&release("v0.3.0"), "0.3.0").unwrap(),
            Evaluation::UpToDate
        );
        assert_eq!(
            evaluate(&release("v0.2.9"), "0.3.0").unwrap(),
            Evaluation::UpToDate
        );
        // Um pré-lançamento publicado nunca é anunciado.
        assert_eq!(
            evaluate(
                &ReleaseInfo {
                    prerelease: true,
                    ..release("v0.4.0-rc.1")
                },
                "0.3.0"
            )
            .unwrap(),
            Evaluation::UpToDate
        );
    }

    #[test]
    fn reports_a_flag_when_a_newer_release_is_published() {
        let calls = Arc::new(AtomicUsize::new(0));
        let checker = UpdateChecker::new(true, ok_fetcher(calls.clone(), "v9.9.9"));

        let response = futures_block_on(checker.snapshot(false));

        assert!(response.enabled);
        assert!(response.available);
        assert_eq!(response.current, VERSION);
        assert_eq!(response.latest.as_deref(), Some("9.9.9"));
        assert_eq!(
            response.released_at.as_deref(),
            Some("2026-10-17T12:00:00Z")
        );
        assert!(
            response
                .url
                .as_deref()
                .unwrap()
                .starts_with("https://github.com/")
        );
        assert!(response.checked_at.is_some());
        assert_eq!(response.error, None);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn serves_the_cached_result_within_the_minimum_interval() {
        let calls = Arc::new(AtomicUsize::new(0));
        let checker = UpdateChecker::with_interval(
            true,
            Duration::from_secs(3600),
            ok_fetcher(calls.clone(), "v9.9.9"),
        );

        assert!(checker.snapshot(false).await.available);
        assert!(checker.snapshot(false).await.available);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn refreshes_after_the_minimum_interval() {
        let calls = Arc::new(AtomicUsize::new(0));
        let checker = UpdateChecker::with_interval(
            true,
            Duration::from_secs(3600),
            counting_fetcher(
                calls.clone(),
                vec![Ok(release("v9.9.9")), Ok(release("v9.9.10"))],
            ),
        );

        assert_eq!(
            checker.snapshot(false).await.latest.as_deref(),
            Some("9.9.9")
        );
        tokio::time::advance(Duration::from_secs(3601)).await;
        assert_eq!(
            checker.snapshot(false).await.latest.as_deref(),
            Some("9.9.10")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn a_forced_check_bypasses_the_minimum_interval() {
        let calls = Arc::new(AtomicUsize::new(0));
        let checker = UpdateChecker::with_interval(
            true,
            Duration::from_secs(3600),
            counting_fetcher(
                calls.clone(),
                vec![Ok(release("v9.9.9")), Ok(release("v9.9.10"))],
            ),
        );

        assert_eq!(
            checker.snapshot(false).await.latest.as_deref(),
            Some("9.9.9")
        );
        // Avança o relógio para que a nova tentativa force uma checagem (sem isso, a
        // conclusão capturada seria tratada como coerção da mesma requisição).
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(
            checker.snapshot(true).await.latest.as_deref(),
            Some("9.9.10")
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn concurrent_checks_are_coalesced_into_a_single_fetch() {
        let calls = Arc::new(AtomicUsize::new(0));
        let results = Arc::new(Mutex::new(vec![Ok(release("v9.9.9"))]));
        let checker = UpdateChecker::with_interval(true, Duration::from_secs(3600), {
            let calls = calls.clone();
            Arc::new(move || {
                let calls = calls.clone();
                let results = results.clone();
                Box::pin(async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    // Cede a vez para que as demais requisições cheguem e esperem o
                    // cadeado em vez de disparar a própria chamada externa.
                    tokio::task::yield_now().await;
                    tokio::task::yield_now().await;
                    results.lock().await.remove(0)
                })
            })
        });

        let mut tasks = Vec::new();
        for _ in 0..8 {
            let checker = checker.clone();
            tasks.push(tokio::spawn(async move { checker.snapshot(true).await }));
        }
        for task in tasks {
            assert!(task.await.unwrap().available);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_network_failure_is_reported_without_dropping_the_last_success() {
        let calls = Arc::new(AtomicUsize::new(0));
        let checker = UpdateChecker::with_interval(
            true,
            Duration::from_secs(3600),
            counting_fetcher(
                calls.clone(),
                vec![
                    Ok(release("v9.9.9")),
                    Err("sem ligação ao GitHub".to_owned()),
                    Err("sem ligação ao GitHub".to_owned()),
                ],
            ),
        );

        assert!(checker.snapshot(false).await.available);
        tokio::time::advance(Duration::from_secs(3601)).await;

        let failed = checker.snapshot(false).await;
        assert!(failed.enabled);
        assert!(!failed.available);
        assert_eq!(failed.latest, None);
        assert_eq!(failed.released_at, None);
        assert_eq!(failed.url, None);
        assert_eq!(failed.checked_at, None);
        assert_eq!(failed.error.as_deref(), Some("sem ligação ao GitHub"));

        // A falha não pode apagar o último sucesso conhecido.
        assert!(checker.cache.lock().await.success.is_some());

        // Dentro do intervalo, a resposta seguinte repete o erro sem nova chamada.
        let repeated = checker.snapshot(false).await;
        assert_eq!(repeated.error.as_deref(), Some("sem ligação ao GitHub"));
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        // O botão manual continua podendo tentar de novo.
        assert!(checker.snapshot(true).await.error.is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_disabled_checker_never_calls_the_fetcher() {
        let calls = Arc::new(AtomicUsize::new(0));
        let checker = UpdateChecker::new(false, ok_fetcher(calls.clone(), "v9.9.9"));

        let response = checker.snapshot(true).await;

        assert!(!response.enabled);
        assert!(!response.available);
        assert_eq!(response.current, VERSION);
        assert_eq!(response.latest, None);
        assert_eq!(response.checked_at, None);
        assert_eq!(response.error, None);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn parses_the_force_flag_leniently() {
        assert!(!UpdateQuery { force: None }.force());
        assert!(
            UpdateQuery {
                force: Some("1".to_owned())
            }
            .force()
        );
        assert!(
            UpdateQuery {
                force: Some("true".to_owned())
            }
            .force()
        );
        assert!(
            !UpdateQuery {
                force: Some("0".to_owned())
            }
            .force()
        );
        assert!(
            !UpdateQuery {
                force: Some(String::new())
            }
            .force()
        );
    }

    /// Bloco `async` mínimo para o teste síncrono do caminho de sucesso.
    fn futures_block_on<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(future)
    }
}
