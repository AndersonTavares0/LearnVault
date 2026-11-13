//! HTTP endpoints of the synchronization journal.
//!
//! The owner is always the session user, as in the other study routes: no request accepts
//! `owner_id`. Pushing is idempotent by `op_id` and mutations require the same origin
//! (CSRF). The read returns the operations after the given cursor, except those of the
//! device itself, which has already applied them locally.
//!
//! Media is not synced. An operation that references media known but outside the user's
//! access is refused as missing media; unknown media is accepted and kept in the journal,
//! because the other device may know it.

use axum::{
    Json,
    extract::{RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{Database, SyncOperationInput};
use serde::Deserialize;

use crate::AppState;
use crate::study::{
    csrf_failed, error_response, invalid_query, private_no_store, run_study,
    study_failure_response, user_or_response,
};

/// Default page of [`pull`] when the query omits `limit`.
const DEFAULT_PULL_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
pub(crate) struct PushRequest {
    device_id: String,
    operations: Vec<SyncOperationInput>,
}

#[derive(Debug, Deserialize)]
struct PullQuery {
    device: String,
    cursor: Option<i64>,
    limit: Option<usize>,
}

/// Applies the sent batch and records each new operation in the journal.
pub(crate) async fn push(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PushRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    let admin = user.is_admin;
    let device_id = request.device_id;
    let operations = request.operations;
    match run_study(&state, move |db| {
        require_sync_access(db, &owner, admin, &operations)?;
        learnvault_core::push_operations(db, &owner, &device_id, &operations)
    })
    .await
    {
        Ok(outcome) => private_no_store(Json(outcome).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Reads the owner's operations after the given cursor.
pub(crate) async fn pull(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) =
        serde_urlencoded::from_str::<PullQuery>(raw.as_deref().unwrap_or_default()).ok()
    else {
        return invalid_query();
    };
    if query.device.trim().is_empty() {
        return error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_sync_operation");
    }
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::pull_operations(
            db,
            &owner,
            &query.device,
            query.cursor.unwrap_or(0),
            query.limit.unwrap_or(DEFAULT_PULL_LIMIT),
        )
    })
    .await
    {
        Ok(batch) => private_no_store(Json(batch).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Refuses existing media outside the user's access; unknown media is accepted.
fn require_sync_access(
    db: &Database,
    user_id: &str,
    is_admin: bool,
    operations: &[SyncOperationInput],
) -> learnvault_core::Result<()> {
    for operation in operations {
        let Some(media_id) = operation.payload.media_id() else {
            continue;
        };
        let Some(library_id) = learnvault_core::find_media_library_id(db, media_id)? else {
            continue;
        };
        if !learnvault_core::can_user_access_course(db, user_id, is_admin, &library_id)? {
            return Err(learnvault_core::Error::MediaNotFound {
                media_id: media_id.to_owned(),
            });
        }
    }
    Ok(())
}

/// The `same_origin` helper lives in the auth adapter; wrapped locally so the handler
/// stays readable.
fn same_origin(headers: &HeaderMap) -> bool {
    crate::auth::same_origin(headers)
}
