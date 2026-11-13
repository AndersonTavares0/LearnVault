//! Read-only reporting for flashcard reviews: history of a card and the owner's due count.
//!
//! The owner is always the session user; no route accepts an `owner_id`. The review log in
//! [`learnvault_core::study`] already isolates by owner, and this adapter only derives the
//! owner from the session, caps the listing and keeps the same course-access filter used by
//! the card list. Nothing here changes scheduling or writes any row.

use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use learnvault_core::FlashcardReview;
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    catalog::private_no_store,
    study::{require_media_access, run_study, study_failure_response, user_or_response},
};

/// Default number of reviews returned when the query omits `limit`.
///
/// The real ceiling is [`learnvault_core::MAX_FLASHCARD_REVIEWS`], applied by the core.
const DEFAULT_REVIEW_LIMIT: usize = 20;

#[derive(Debug, Deserialize)]
struct ReviewsQuery {
    limit: Option<usize>,
}

#[derive(Serialize)]
struct ReviewResponse {
    review_id: i64,
    card_id: String,
    rating: &'static str,
    previous_interval_days: i64,
    next_due: i64,
    reviewed_at: i64,
}

impl From<FlashcardReview> for ReviewResponse {
    fn from(review: FlashcardReview) -> Self {
        Self {
            review_id: review.review_id,
            card_id: review.card_id,
            rating: review.rating.as_str(),
            previous_interval_days: review.previous_interval_days,
            next_due: review.next_due,
            reviewed_at: review.reviewed_at,
        }
    }
}

#[derive(Serialize)]
struct ReviewsResponse {
    items: Vec<ReviewResponse>,
    total: i64,
}

#[derive(Serialize)]
struct DueResponse {
    due: i64,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

/// Lists the most recent reviews of one card, newest first, with the full log count.
pub(crate) async fn history(
    State(state): State<AppState>,
    Path(card_id): Path<String>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) =
        serde_urlencoded::from_str::<ReviewsQuery>(raw.as_deref().unwrap_or_default()).ok()
    else {
        return error_response(StatusCode::BAD_REQUEST, "invalid_query");
    };
    let owner = user.user_id;
    let admin = user.is_admin;
    let limit = query.limit.unwrap_or(DEFAULT_REVIEW_LIMIT);

    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::flashcard_media_id(db, &card_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        let items = learnvault_core::list_flashcard_reviews(db, &owner, &card_id, limit)?;
        let total = learnvault_core::count_flashcard_reviews(db, &owner, &card_id)?;
        Ok(ReviewsResponse {
            items: items.into_iter().map(ReviewResponse::from).collect(),
            total,
        })
    })
    .await
    {
        Ok(reviews) => private_no_store(Json(reviews).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Counts the cards due now for the session user, matching the deck the review page shows.
pub(crate) async fn due_count(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    let admin = user.is_admin;

    match run_study(&state, move |db| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        // The card list filters lessons the user may no longer open; the count follows it.
        // shortcut: this loads the owner's cards to filter by access, switch to a SQL count
        // with the allowed libraries if decks grow large.
        let cards = learnvault_core::list_all_flashcards(db, &owner)?;
        let due = cards
            .into_iter()
            .filter(|card| {
                card.next_due <= now
                    && require_media_access(db, &owner, admin, &card.media_id).is_ok()
            })
            .count();
        Ok(DueResponse {
            due: i64::try_from(due).unwrap_or(i64::MAX),
        })
    })
    .await
    {
        Ok(due) => private_no_store(Json(due).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

fn error_response(status: StatusCode, error: &'static str) -> Response {
    private_no_store((status, Json(ErrorResponse { error })).into_response())
}
