//! Endpoints HTTP autenticados para a sessão Pomodoro persistente.

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{PomodoroState, PomodoroStatus};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token, same_origin},
    catalog::service_unavailable,
    study::{
        credentials_required, error_response, internal_error, private_no_store,
        status_private_no_store,
    },
};

#[derive(Debug, Deserialize)]
pub(crate) struct StartRequest {
    duration_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PauseRequest {
    elapsed_seconds: i64,
}

#[derive(Serialize)]
struct PomodoroResponse {
    status: &'static str,
    duration_seconds: i64,
    elapsed_seconds: i64,
    updated_at: i64,
}

impl From<PomodoroState> for PomodoroResponse {
    fn from(value: PomodoroState) -> Self {
        Self {
            status: match value.status {
                PomodoroStatus::Idle => "idle",
                PomodoroStatus::Running => "running",
                PomodoroStatus::Paused => "paused",
            },
            duration_seconds: value.duration_seconds,
            elapsed_seconds: value.elapsed_seconds,
            updated_at: value.updated_at,
        }
    }
}

pub(crate) async fn state(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(owner) = owner(&state, &headers).await else {
        return unauthorized(&state, &headers).await;
    };
    run(&state, owner, |db, owner| {
        learnvault_core::pomodoro_state(db, owner)
    })
    .await
}

pub(crate) async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<StartRequest>,
) -> Response {
    let Some(owner) = owner(&state, &headers).await else {
        return unauthorized(&state, &headers).await;
    };
    let duration = request
        .duration_seconds
        .unwrap_or(learnvault_core::DEFAULT_POMODORO_SECONDS);
    if !same_origin(&headers) {
        return error_response(StatusCode::FORBIDDEN, "csrf_failed");
    }
    run(&state, owner, move |db, owner| {
        learnvault_core::start_pomodoro(db, owner, duration)
    })
    .await
}

pub(crate) async fn pause(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PauseRequest>,
) -> Response {
    let Some(owner) = owner(&state, &headers).await else {
        return unauthorized(&state, &headers).await;
    };
    if !same_origin(&headers) {
        return error_response(StatusCode::FORBIDDEN, "csrf_failed");
    }
    run(&state, owner, move |db, owner| {
        learnvault_core::pause_pomodoro(db, owner, request.elapsed_seconds)
    })
    .await
}

pub(crate) async fn reset(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(owner) = owner(&state, &headers).await else {
        return unauthorized(&state, &headers).await;
    };
    if !same_origin(&headers) {
        return error_response(StatusCode::FORBIDDEN, "csrf_failed");
    }
    run(&state, owner, |db, owner| {
        learnvault_core::reset_pomodoro(db, owner)
    })
    .await
}

async fn owner(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let token = request_token(headers)?;
    match state.session_user(hash_session_token(&token)).await {
        Ok(Some(user)) if !user.must_change_credentials => Some(user.user_id),
        _ => None,
    }
}

async fn unauthorized(state: &AppState, headers: &HeaderMap) -> Response {
    let Some(token) = request_token(headers) else {
        return status_private_no_store(StatusCode::UNAUTHORIZED);
    };
    match state.session_user(hash_session_token(&token)).await {
        Ok(Some(user)) if user.must_change_credentials => credentials_required(),
        Err(StateError::Capacity) => service_unavailable(),
        _ => status_private_no_store(StatusCode::UNAUTHORIZED),
    }
}

async fn run<F>(state: &AppState, owner: String, work: F) -> Response
where
    F: FnOnce(&learnvault_core::Database, &str) -> learnvault_core::Result<PomodoroState>
        + Send
        + 'static,
{
    if !state.has_blocking_capacity() {
        return service_unavailable();
    }
    let database = state.database.clone().lock_owned().await;
    let Some(permit) = state.try_acquire_blocking_task() else {
        return service_unavailable();
    };
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        work(&database, &owner)
    })
    .await
    {
        Ok(Ok(value)) => private_no_store(Json(PomodoroResponse::from(value)).into_response()),
        Ok(Err(learnvault_core::Error::InvalidPomodoroDuration { .. })) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_pomodoro_duration",
        ),
        Ok(Err(learnvault_core::Error::InvalidStudyPosition { .. })) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_pomodoro_elapsed")
        }
        _ => internal_error("pomodoro unavailable"),
    }
}
