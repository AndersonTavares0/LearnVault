//! HTTP endpoints of the synchronization journal.
//!
//! The owner is always the session user, as in the other study routes: no request accepts
//! `owner_id`. Pushing is idempotent by `op_id` and mutations require the same origin
//! (CSRF). The read returns the operations after the given cursor, except those of the
//! device itself, which has already applied them locally.
//!
//! Media is not synced. An operation that references media known but outside the user's
//! access is refused as missing media; unknown media is accepted and kept in the journal,
//! because the other device may know it. The read applies the current grant too: operations
//! on media the user can no longer access are skipped (the cursor still advances).

use axum::{
    Json,
    extract::{RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::{Database, SyncOperationInput, SyncPayload};
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
        let outcome = learnvault_core::push_operations(db, &owner, &device_id, &operations)?;
        prune_once_a_day(db);
        Ok(outcome)
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
    let admin = user.is_admin;
    match run_study(&state, move |db| {
        let batch = learnvault_core::pull_operations_for_user(
            db,
            &owner,
            admin,
            &query.device,
            query.cursor.unwrap_or(0),
            query.limit.unwrap_or(DEFAULT_PULL_LIMIT),
        )?;
        prune_once_a_day(db);
        Ok(batch)
    })
    .await
    {
        Ok(batch) => private_no_store(Json(batch).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Retention housekeeping (old tombstones, idle devices) at most once a day per process. It runs on a
/// sync request, like the session cleanup on login, so no scheduler is needed; a failure is not
/// worth failing the request for and is retried the next day.
fn prune_once_a_day(db: &Database) {
    static LAST: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64);
    let last = LAST.load(std::sync::atomic::Ordering::Relaxed);
    if now - last >= 24 * 60 * 60
        && LAST
            .compare_exchange(
                last,
                now,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
    {
        let _ = learnvault_core::prune_sync(db, learnvault_core::DEFAULT_RETENTION_SECS);
    }
}

fn check_media_access(
    db: &Database,
    user_id: &str,
    is_admin: bool,
    media_id: &str,
) -> learnvault_core::Result<()> {
    let Some(library_id) = learnvault_core::find_media_library_id(db, media_id)? else {
        return Ok(());
    };
    if !learnvault_core::can_user_access_course(db, user_id, is_admin, &library_id)? {
        return Err(learnvault_core::Error::MediaNotFound {
            media_id: media_id.to_owned(),
        });
    }
    Ok(())
}

fn check_existing_record_media(
    db: &Database,
    user_id: &str,
    is_admin: bool,
    raw_id: &str,
    lookup: fn(&Database, &str, &str) -> learnvault_core::Result<Option<String>>,
) -> learnvault_core::Result<()> {
    let trimmed = raw_id.trim();
    if let Some(existing_media) = lookup(db, user_id, trimmed)? {
        check_media_access(db, user_id, is_admin, &existing_media)?;
    }
    // Also inspect the raw untrimmed key in case an older row was persisted before trimming.
    if raw_id != trimmed
        && let Some(existing_media) = lookup(db, user_id, raw_id)?
    {
        check_media_access(db, user_id, is_admin, &existing_media)?;
    }
    Ok(())
}

/// Refuses existing media outside the user's access; unknown media is accepted.
fn require_sync_access(
    db: &Database,
    user_id: &str,
    is_admin: bool,
    operations: &[SyncOperationInput],
) -> learnvault_core::Result<()> {
    for operation in operations {
        match &operation.payload {
            SyncPayload::Progress { media_id, .. } | SyncPayload::Bookmark { media_id, .. } => {
                check_media_access(db, user_id, is_admin, media_id)?;
            }
            SyncPayload::Note {
                note_id, media_id, ..
            } => {
                check_media_access(db, user_id, is_admin, media_id)?;
                check_existing_record_media(
                    db,
                    user_id,
                    is_admin,
                    note_id,
                    learnvault_core::study_note_media_id_for_owner,
                )?;
            }
            SyncPayload::FlashcardCreated {
                card_id, media_id, ..
            } => {
                check_media_access(db, user_id, is_admin, media_id)?;
                check_existing_record_media(
                    db,
                    user_id,
                    is_admin,
                    card_id,
                    learnvault_core::flashcard_media_id_for_owner,
                )?;
            }
            SyncPayload::NoteDeleted { note_id } => {
                check_existing_record_media(
                    db,
                    user_id,
                    is_admin,
                    note_id,
                    learnvault_core::study_note_media_id_for_owner,
                )?;
            }
            SyncPayload::BookmarkDeleted { bookmark_id } => {
                check_existing_record_media(
                    db,
                    user_id,
                    is_admin,
                    bookmark_id,
                    learnvault_core::media_bookmark_media_id_for_owner,
                )?;
            }
            SyncPayload::FlashcardUpdated { card_id, .. }
            | SyncPayload::FlashcardDeleted { card_id }
            | SyncPayload::FlashcardReview { card_id, .. } => {
                check_existing_record_media(
                    db,
                    user_id,
                    is_admin,
                    card_id,
                    learnvault_core::flashcard_media_id_for_owner,
                )?;
            }
            SyncPayload::Preference { .. } => {}
        }
    }
    Ok(())
}

/// The `same_origin` helper lives in the auth adapter; wrapped locally so the handler
/// stays readable.
fn same_origin(headers: &HeaderMap) -> bool {
    crate::auth::same_origin(headers)
}
