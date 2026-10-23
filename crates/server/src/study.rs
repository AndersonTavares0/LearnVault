//! Endpoints HTTP autenticados do progresso de estudo.
//!
//! O dono do progresso é sempre o usuário da sessão (`user_id`): nenhuma rota aceita um
//! `owner_id` vindo do cliente, então uma sessão só lê e escreve o próprio progresso. O
//! domínio em [`learnvault_core::study`] já garante o isolamento por dono; este adaptador
//! apenas deriva o dono da sessão autenticada, valida o corpo tipado e traduz os erros do
//! núcleo para códigos HTTP estáveis.
//!
//! Cada endpoint exige uma sessão válida e recusa a sessão temporária que ainda precisa
//! trocar as credenciais. As mutações também exigem a mesma origem (CSRF), como as demais
//! escritas do servidor. Nenhuma rota grava `media`, `scan`, `library` ou a biblioteca
//! física.

use axum::{
    Json,
    extract::{RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use learnvault_core::{AuthSession, Database, PomodoroState, PomodoroStatus, StudyAnalytics, StudyProgress};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token, same_origin},
    catalog::service_unavailable,
};

/// Limite padrão de itens de [`recent`] quando a consulta não informa `limit`.
///
/// O teto real continua sendo [`learnvault_core::MAX_RECENT_STUDY`], aplicado pelo núcleo.
const DEFAULT_RECENT_LIMIT: usize = 20;

#[derive(Debug, Deserialize)]
pub(crate) struct PositionRequest {
    media_id: String,
    position_seconds: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompletedRequest {
    media_id: String,
    completed: bool,
}

#[derive(Debug, Deserialize)]
struct RecentQuery {
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ProgressQuery {
    media_id: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

/// Progresso de uma mídia como o cliente o consome.
///
/// `recorded` distingue "nunca estudada" de "estudada na posição zero": a ausência não é
/// um erro no domínio, então a leitura responde `200` e sinaliza a ausência pelo campo.
#[derive(Serialize)]
struct ProgressResponse {
    media_id: String,
    position_seconds: i64,
    completed: bool,
    updated_at: Option<i64>,
    recorded: bool,
}

#[derive(Serialize)]
struct RecentResponse {
    items: Vec<ProgressResponse>,
}

#[derive(Serialize)]
struct CourseSummary {
    course_id: String,
    completed_media: usize,
}

#[derive(Serialize)]
struct SummaryResponse {
    courses: Vec<CourseSummary>,
}

#[derive(Serialize)]
struct AnalyticsResponse {
    tracked_count: usize,
    completed_count: usize,
    total_position_seconds: i64,
    last_activity_at: Option<i64>,
}

pub(crate) async fn analytics(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| learnvault_core::study_analytics(db, &owner)).await {
        Ok(stats) => private_no_store(Json(AnalyticsResponse::from(stats)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

impl From<StudyAnalytics> for AnalyticsResponse {
    fn from(stats: StudyAnalytics) -> Self {
        Self { tracked_count: stats.tracked_count, completed_count: stats.completed_count,
            total_position_seconds: stats.total_position_seconds, last_activity_at: stats.last_activity_at }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PomodoroRequest { duration_seconds: Option<i64>, elapsed_seconds: Option<i64> }

#[derive(Serialize)]
struct PomodoroResponse { status: &'static str, duration_seconds: i64, elapsed_seconds: i64, updated_at: i64 }

impl From<PomodoroState> for PomodoroResponse {
    fn from(state: PomodoroState) -> Self {
        Self {
            status: match state.status { PomodoroStatus::Idle => "idle", PomodoroStatus::Running => "running", PomodoroStatus::Paused => "paused" },
            duration_seconds: state.duration_seconds,
            elapsed_seconds: state.elapsed_seconds,
            updated_at: state.updated_at,
        }
    }
}

pub(crate) async fn pomodoro(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await { Ok(user) => user, Err(failure) => return failure.into_response() };
    let owner = user.user_id;
    match run_study(&state, move |db| learnvault_core::pomodoro_state(db, &owner)).await {
        Ok(session) => private_no_store(Json(PomodoroResponse::from(session)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

pub(crate) async fn start_pomodoro(State(state): State<AppState>, headers: HeaderMap, Json(request): Json<PomodoroRequest>) -> Response {
    if !same_origin(&headers) { return csrf_failed(); }
    let Some(duration) = request.duration_seconds else { return invalid_query(); };
    let user = match user_or_response(&state, &headers).await { Ok(user) => user, Err(failure) => return failure.into_response() };
    let owner = user.user_id;
    match run_study(&state, move |db| learnvault_core::start_pomodoro(db, &owner, duration)).await {
        Ok(session) => private_no_store(Json(PomodoroResponse::from(session)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

pub(crate) async fn pause_pomodoro(State(state): State<AppState>, headers: HeaderMap, Json(request): Json<PomodoroRequest>) -> Response {
    if !same_origin(&headers) { return csrf_failed(); }
    let Some(elapsed) = request.elapsed_seconds else { return invalid_query(); };
    let user = match user_or_response(&state, &headers).await { Ok(user) => user, Err(failure) => return failure.into_response() };
    let owner = user.user_id;
    match run_study(&state, move |db| learnvault_core::pause_pomodoro(db, &owner, elapsed)).await {
        Ok(session) => private_no_store(Json(PomodoroResponse::from(session)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

pub(crate) async fn reset_pomodoro(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !same_origin(&headers) { return csrf_failed(); }
    let user = match user_or_response(&state, &headers).await { Ok(user) => user, Err(failure) => return failure.into_response() };
    let owner = user.user_id;
    match run_study(&state, move |db| learnvault_core::reset_pomodoro(db, &owner)).await {
        Ok(session) => private_no_store(Json(PomodoroResponse::from(session)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Documento de capacidades do Study, o contrato explícito de detecção de recursos.
///
/// O shape espelha o serviço Go de referência para que a SPA detecte recursos nos dois
/// transportes, mas os valores relatam o comportamento real deste servidor. A identidade
/// de mídia do Rust é o `media_id` opaco gerado pelo domínio, que sobrevive a renomeações
/// — é o `relative_path` do Go de referência que não é estável. É exatamente por isso que
/// o endpoint existe: para o cliente não presumir paridade onde ela não existe.
#[derive(Serialize)]
struct CapabilitiesResponse {
    study: bool,
    media_identity: &'static str,
    stable_media_identity: bool,
    recent_default_limit: usize,
    recent_max_limit: usize,
    removal_keeps_progress: bool,
}

/// Valor de `media_identity` deste servidor: a identidade é o `media_id` opaco, não o
/// caminho relativo do arquivo.
const MEDIA_IDENTITY: &str = "media_id";

impl ProgressResponse {
    fn recorded(progress: StudyProgress) -> Self {
        Self {
            media_id: progress.media_id,
            position_seconds: progress.position_seconds,
            completed: progress.completed,
            updated_at: Some(progress.updated_at),
            recorded: true,
        }
    }

    /// Estado neutro para uma mídia conhecida sem progresso: nada foi escrito ainda.
    fn absent(media_id: String) -> Self {
        Self {
            media_id,
            position_seconds: 0,
            completed: false,
            updated_at: None,
            recorded: false,
        }
    }
}

/// Lista o progresso do usuário autenticado, do alterado mais recentemente ao mais antigo.
pub(crate) async fn recent(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) = parse_recent_query(raw.as_deref()) else {
        return invalid_query();
    };
    let limit = query.limit.unwrap_or(DEFAULT_RECENT_LIMIT);
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::recent_study(db, &owner, limit)
    })
    .await
    {
        Ok(states) => private_no_store(
            Json(RecentResponse {
                items: states.into_iter().map(ProgressResponse::recorded).collect(),
            })
            .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Resume conclusões por curso para a biblioteca do usuário autenticado.
pub(crate) async fn summary(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::course_study_summary(db, &owner)
    })
    .await
    {
        Ok(courses) => private_no_store(
            Json(SummaryResponse {
                courses: courses
                    .into_iter()
                    .map(|item| CourseSummary {
                        course_id: item.course_id,
                        completed_media: item.completed_media,
                    })
                    .collect(),
            })
            .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Lê o progresso do usuário autenticado em uma mídia.
///
/// Uma mídia conhecida sem progresso responde `200` com `recorded: false`; uma mídia
/// desconhecida no catálogo responde `404`.
pub(crate) async fn progress(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) = parse_progress_query(raw.as_deref()) else {
        return invalid_query();
    };
    let owner = user.user_id;
    let media_id = query.media_id;
    let lookup = media_id.clone();
    match run_study(&state, move |db| {
        learnvault_core::study_progress(db, &owner, &lookup)
    })
    .await
    {
        Ok(Some(found)) => {
            private_no_store(Json(ProgressResponse::recorded(found)).into_response())
        }
        Ok(None) => private_no_store(Json(ProgressResponse::absent(media_id)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Registra a posição de retomada do usuário autenticado, preservando a conclusão.
pub(crate) async fn record_position(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PositionRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    let media_id = request.media_id;
    let position_seconds = request.position_seconds;
    match run_study(&state, move |db| {
        learnvault_core::record_position(db, &owner, &media_id, position_seconds)
    })
    .await
    {
        Ok(progress) => {
            private_no_store(Json(ProgressResponse::recorded(progress)).into_response())
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Marca ou desmarca a conclusão do usuário autenticado, preservando a posição.
pub(crate) async fn set_completed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CompletedRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    let media_id = request.media_id;
    let completed = request.completed;
    match run_study(&state, move |db| {
        learnvault_core::set_completed(db, &owner, &media_id, completed)
    })
    .await
    {
        Ok(progress) => {
            private_no_store(Json(ProgressResponse::recorded(progress)).into_response())
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Responde o contrato de capacidades do Study.
///
/// Exige a mesma sessão autenticada (e a mesma recusa à sessão temporária) das demais
/// rotas de leitura do Study, sem ler nem gravar catálogo, estado de estudo ou biblioteca
/// física. A resposta é sempre `private, no-store`. A leitura da sessão ocupa a capacidade
/// *blocking* como as outras rotas, então sob saturação responde `503` em vez de segurar o
/// mutex do banco.
pub(crate) async fn capabilities(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Err(failure) = user_or_response(&state, &headers).await {
        return failure.into_response();
    }
    private_no_store(
        Json(CapabilitiesResponse {
            study: true,
            media_identity: MEDIA_IDENTITY,
            stable_media_identity: true,
            recent_default_limit: DEFAULT_RECENT_LIMIT,
            recent_max_limit: learnvault_core::MAX_RECENT_STUDY,
            removal_keeps_progress: true,
        })
        .into_response(),
    )
}

/// Falha da camada de estudo, antes da tradução para HTTP.
enum StudyFailure {
    /// Não há capacidade para iniciar outra tarefa blocking.
    Busy,
    /// A tarefa blocking não pôde concluir.
    Blocking,
    /// O domínio recusou a operação.
    Core(learnvault_core::Error),
}

/// Executa uma operação do núcleo sobre o banco, fora das threads Tokio.
///
/// O permit e o guard do banco são movidos para a closure, então continuam contabilizados
/// até o trabalho terminar, mesmo que a requisição HTTP expire antes.
async fn run_study<T, F>(state: &AppState, work: F) -> Result<T, StudyFailure>
where
    F: FnOnce(&Database) -> learnvault_core::Result<T> + Send + 'static,
    T: Send + 'static,
{
    // Evita segurar o mutex do banco só para devolver 503 sob saturação.
    if !state.has_blocking_capacity() {
        return Err(StudyFailure::Busy);
    }
    let database = state.database.clone().lock_owned().await;
    let Some(permit) = state.try_acquire_blocking_task() else {
        return Err(StudyFailure::Busy);
    };
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work(&database)
    })
    .await
    .map_err(|_| StudyFailure::Blocking)?
    .map_err(StudyFailure::Core)
}

fn study_failure_response(failure: StudyFailure) -> Response {
    use learnvault_core::Error;

    match failure {
        StudyFailure::Busy => service_unavailable(),
        StudyFailure::Blocking => internal_error("study unavailable"),
        // Uma mídia ausente é 404; uma posição negativa é uma validação de entrada (422).
        // Qualquer outro erro do núcleo é operacional e não pode virar 404, para não
        // confundir ausência legítima com banco corrompido.
        StudyFailure::Core(Error::MediaNotFound { .. }) => {
            error_response(StatusCode::NOT_FOUND, "media_not_found")
        }
        StudyFailure::Core(Error::InvalidStudyPosition { .. }) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_position")
        }
        StudyFailure::Core(Error::InvalidPomodoroDuration { .. }) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_pomodoro_duration")
        }
        StudyFailure::Core(_) => internal_error("study unavailable"),
    }
}

/// Falha da autenticação/limite antes de traduzir para HTTP.
enum AuthFailure {
    Unauthorized,
    CredentialsRequired,
    Busy,
}

impl AuthFailure {
    fn into_response(self) -> Response {
        match self {
            AuthFailure::Unauthorized => status_private_no_store(StatusCode::UNAUTHORIZED),
            AuthFailure::CredentialsRequired => credentials_required(),
            AuthFailure::Busy => service_unavailable(),
        }
    }
}

async fn user_or_response(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthSession, AuthFailure> {
    match authenticated_user(state, headers).await {
        Ok(Some(user)) if !user.must_change_credentials => Ok(user),
        Ok(Some(_)) => Err(AuthFailure::CredentialsRequired),
        Err(StateError::Capacity) => Err(AuthFailure::Busy),
        Ok(None) | Err(_) => Err(AuthFailure::Unauthorized),
    }
}

async fn authenticated_user(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Option<AuthSession>, StateError> {
    let Some(token) = request_token(headers) else {
        return Ok(None);
    };
    state.session_user(hash_session_token(&token)).await
}

fn parse_recent_query(raw: Option<&str>) -> Option<RecentQuery> {
    serde_urlencoded::from_str::<RecentQuery>(raw.unwrap_or_default()).ok()
}

fn parse_progress_query(raw: Option<&str>) -> Option<ProgressQuery> {
    serde_urlencoded::from_str::<ProgressQuery>(raw.unwrap_or_default()).ok()
}

fn internal_error(message: &'static str) -> Response {
    error_response(StatusCode::INTERNAL_SERVER_ERROR, message)
}

fn invalid_query() -> Response {
    error_response(StatusCode::BAD_REQUEST, "invalid_query")
}

fn error_response(status: StatusCode, error: &'static str) -> Response {
    private_no_store((status, Json(ErrorResponse { error })).into_response())
}

fn credentials_required() -> Response {
    error_response(StatusCode::FORBIDDEN, "credentials_change_required")
}

fn csrf_failed() -> Response {
    error_response(StatusCode::FORBIDDEN, "csrf_failed")
}

fn private_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

fn status_private_no_store(status: StatusCode) -> Response {
    private_no_store(status.into_response())
}
