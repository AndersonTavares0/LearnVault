//! HTTP autenticado para notas pessoais.

use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use learnvault_core::{AuthSession, Database, StudyNote};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token, same_origin},
    catalog::service_unavailable,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NotesQuery {
    media_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteRequest {
    pub media_id: String,
    pub text: String,
    pub position_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateNoteRequest {
    pub text: String,
    pub position_seconds: Option<i64>,
}

#[derive(Serialize)]
struct NotesResponse {
    items: Vec<NoteResponse>,
}

#[derive(Serialize)]
struct NoteResponse {
    note_id: i64,
    media_id: String,
    text: String,
    position_seconds: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

impl From<StudyNote> for NoteResponse {
    fn from(note: StudyNote) -> Self {
        Self {
            note_id: note.note_id,
            media_id: note.media_id,
            text: note.text,
            position_seconds: note.position_seconds,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

enum Failure {
    Busy,
    Blocking,
    Core(learnvault_core::Error),
}

type CoreResult<T> = std::result::Result<T, Failure>;

pub(crate) async fn list(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) = raw
        .as_deref()
        .and_then(|raw| serde_urlencoded::from_str::<NotesQuery>(raw).ok())
    else {
        return error(StatusCode::BAD_REQUEST, "invalid_query");
    };
    let owner = user.user_id;
    match run(&state, move |db| {
        learnvault_core::list_notes(db, &owner, &query.media_id)
    })
    .await
    {
        Ok(items) => private(
            Json(NotesResponse {
                items: items.into_iter().map(NoteResponse::from).collect(),
            })
            .into_response(),
        ),
        Err(failure) => failure_response(failure),
    }
}

pub(crate) async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<NoteRequest>,
) -> Response {
    if !same_origin(&headers) {
        return error(StatusCode::FORBIDDEN, "csrf_failed");
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run(&state, move |db| {
        learnvault_core::create_note(
            db,
            &owner,
            &request.media_id,
            &request.text,
            request.position_seconds,
        )
    })
    .await
    {
        Ok(note) => private((StatusCode::CREATED, Json(NoteResponse::from(note))).into_response()),
        Err(failure) => failure_response(failure),
    }
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(note_id): Path<i64>,
    headers: HeaderMap,
    Json(request): Json<UpdateNoteRequest>,
) -> Response {
    if !same_origin(&headers) {
        return error(StatusCode::FORBIDDEN, "csrf_failed");
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run(&state, move |db| {
        learnvault_core::update_note(db, &owner, note_id, &request.text, request.position_seconds)
    })
    .await
    {
        Ok(note) => private(Json(NoteResponse::from(note)).into_response()),
        Err(failure) => failure_response(failure),
    }
}

pub(crate) async fn delete(
    State(state): State<AppState>,
    Path(note_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    if !same_origin(&headers) {
        return error(StatusCode::FORBIDDEN, "csrf_failed");
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run(&state, move |db| {
        learnvault_core::delete_note(db, &owner, note_id)
    })
    .await
    {
        Ok(()) => private(StatusCode::NO_CONTENT.into_response()),
        Err(failure) => failure_response(failure),
    }
}

async fn run<T, F>(state: &AppState, work: F) -> CoreResult<T>
where
    F: FnOnce(&Database) -> learnvault_core::Result<T> + Send + 'static,
    T: Send + 'static,
{
    if !state.has_blocking_capacity() {
        return Err(Failure::Busy);
    }
    let database = state.database.clone().lock_owned().await;
    let Some(permit) = state.try_acquire_blocking_task() else {
        return Err(Failure::Busy);
    };
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work(&database)
    })
    .await
    .map_err(|_| Failure::Blocking)?
    .map_err(Failure::Core)
}

/// Falha da autenticação/limite antes de traduzir para HTTP.
///
/// A variante de erro é pequena de propósito: a resposta HTTP completa tem 128 bytes ou
/// mais e o clippy do CI recusa um `Result` que carrega esse tamanho.
enum AuthFailure {
    /// A requisição não tem sessão válida.
    Unauthorized,
    /// A sessão exige troca de credenciais antes de continuar.
    CredentialsRequired,
    /// O servidor está sem capacidade para a tarefa.
    Busy,
    /// Falha inesperada do estado da aplicação.
    Unavailable,
}

impl AuthFailure {
    fn into_response(self) -> Response {
        match self {
            AuthFailure::Unauthorized => private(StatusCode::UNAUTHORIZED.into_response()),
            AuthFailure::CredentialsRequired => {
                error(StatusCode::FORBIDDEN, "credentials_change_required")
            }
            AuthFailure::Busy => service_unavailable(),
            AuthFailure::Unavailable => {
                error(StatusCode::INTERNAL_SERVER_ERROR, "notes_unavailable")
            }
        }
    }
}

async fn user_or_response(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthSession, AuthFailure> {
    let Some(token) = request_token(headers) else {
        return Err(AuthFailure::Unauthorized);
    };
    match state.session_user(hash_session_token(&token)).await {
        Ok(Some(user)) if !user.must_change_credentials => Ok(user),
        Ok(Some(_)) => Err(AuthFailure::CredentialsRequired),
        Ok(None) => Err(AuthFailure::Unauthorized),
        Err(StateError::Capacity) => Err(AuthFailure::Busy),
        Err(_) => Err(AuthFailure::Unavailable),
    }
}

fn failure_response(failure: Failure) -> Response {
    use learnvault_core::Error as CoreError;
    match failure {
        Failure::Busy => service_unavailable(),
        Failure::Blocking => error(StatusCode::INTERNAL_SERVER_ERROR, "notes_unavailable"),
        Failure::Core(CoreError::MediaNotFound { .. }) => {
            error(StatusCode::NOT_FOUND, "media_not_found")
        }
        Failure::Core(CoreError::StudyNoteNotFound) => {
            error(StatusCode::NOT_FOUND, "note_not_found")
        }
        Failure::Core(CoreError::InvalidStudyPosition { .. }) => {
            error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_position")
        }
        Failure::Core(CoreError::InvalidStudyNote) => {
            error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_note")
        }
        Failure::Core(CoreError::TooManyNotes) => {
            error(StatusCode::UNPROCESSABLE_ENTITY, "too_many_notes")
        }
        Failure::Core(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "notes_unavailable"),
    }
}

fn error(status: StatusCode, code: &'static str) -> Response {
    private((status, Json(ErrorResponse { error: code })).into_response())
}
fn private(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}
