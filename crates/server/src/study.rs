//! Authenticated HTTP endpoints for study progress.
//!
//! The owner of the progress is always the session's user (`user_id`): no route accepts an
//! `owner_id` coming from the client, so a session only reads and writes its own progress. The
//! domain in [`learnvault_core::study`] already guarantees isolation by owner; this adapter
//! only derives the owner from the authenticated session, validates the typed body and translates the
//! core's errors into stable HTTP codes.
//!
//! Each endpoint requires a valid session and refuses the temporary session that still has to
//! change its credentials. Mutations also require the same origin (CSRF), like the server's other
//! writes. No route writes `media`, `scan`, `library` or the physical
//! library.

use axum::{
    Json,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use learnvault_core::{
    AuthSession, Database, FocusKind, FocusSessionInput, StudyProgress, StudySummary,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, StateError,
    auth::{hash_session_token, request_token, same_origin},
    catalog::service_unavailable,
};

/// Default item limit of [`recent`] when the query gives no `limit`.
///
/// The real ceiling remains [`learnvault_core::MAX_RECENT_STUDY`], applied by the core.
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

#[derive(Debug, Deserialize)]
pub(crate) struct FocusSessionRequest {
    id: String,
    kind: String,
    planned_seconds: i64,
    actual_seconds: i64,
    completed: bool,
    started_at: i64,
    ended_at: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WeeklyGoalRequest {
    weekly_goal: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct NotesQuery {
    pub(crate) media_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateNoteRequest {
    media_id: String,
    position_seconds: i64,
    content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateNoteRequest {
    content: String,
}

#[derive(Serialize)]
pub(crate) struct StudyNoteResponse {
    note_id: String,
    media_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    relative_path: Option<String>,
    position_seconds: i64,
    content: String,
    created_at: i64,
    updated_at: i64,
}

impl From<learnvault_core::StudyNote> for StudyNoteResponse {
    fn from(note: learnvault_core::StudyNote) -> Self {
        Self {
            note_id: note.note_id,
            media_id: note.media_id,
            relative_path: None,
            position_seconds: note.position_seconds,
            content: note.content,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}

impl From<learnvault_core::GlobalStudyNote> for StudyNoteResponse {
    fn from(note: learnvault_core::GlobalStudyNote) -> Self {
        Self {
            note_id: note.note_id,
            media_id: note.media_id,
            relative_path: Some(note.relative_path),
            position_seconds: note.position_seconds,
            content: note.content,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}

#[derive(Serialize)]
struct NotesResponse {
    items: Vec<StudyNoteResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BookmarksQuery {
    media_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateBookmarkRequest {
    media_id: String,
    position_seconds: i64,
    label: String,
}

#[derive(Serialize)]
pub(crate) struct MediaBookmarkResponse {
    bookmark_id: String,
    media_id: String,
    position_seconds: i64,
    label: String,
    created_at: i64,
}

impl From<learnvault_core::MediaBookmark> for MediaBookmarkResponse {
    fn from(bookmark: learnvault_core::MediaBookmark) -> Self {
        Self {
            bookmark_id: bookmark.bookmark_id,
            media_id: bookmark.media_id,
            position_seconds: bookmark.position_seconds,
            label: bookmark.label,
            created_at: bookmark.created_at,
        }
    }
}

#[derive(Serialize)]
struct BookmarksResponse {
    items: Vec<MediaBookmarkResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PdfAnnotationsQuery {
    media_id: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PdfRectDto {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreatePdfAnnotationRequest {
    media_id: String,
    page: i64,
    kind: String,
    color: String,
    rects: Vec<PdfRectDto>,
    #[serde(default)]
    quote: String,
    #[serde(default)]
    content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdatePdfAnnotationRequest {
    color: Option<String>,
    content: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct PdfAnnotationResponse {
    annotation_id: String,
    media_id: String,
    page: i64,
    kind: &'static str,
    color: &'static str,
    rects: Vec<PdfRectDto>,
    quote: String,
    content: String,
    created_at: i64,
    updated_at: i64,
}

impl From<learnvault_core::PdfAnnotation> for PdfAnnotationResponse {
    fn from(annotation: learnvault_core::PdfAnnotation) -> Self {
        Self {
            annotation_id: annotation.annotation_id,
            media_id: annotation.media_id,
            page: annotation.page,
            kind: annotation.kind.as_str(),
            color: annotation.color.as_str(),
            rects: annotation
                .rects
                .into_iter()
                .map(|rect| PdfRectDto {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                })
                .collect(),
            quote: annotation.quote,
            content: annotation.content,
            created_at: annotation.created_at,
            updated_at: annotation.updated_at,
        }
    }
}

#[derive(Serialize)]
struct PdfAnnotationsResponse {
    items: Vec<PdfAnnotationResponse>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FlashcardsQuery {
    /// Absent: every card of the user, for the review hub.
    media_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CreateFlashcardRequest {
    media_id: String,
    front: String,
    back: String,
    /// Instant of the lesson at which the card was created; absent on cards with no time link.
    #[serde(default)]
    position_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateFlashcardRequest {
    front: String,
    back: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReviewFlashcardRequest {
    rating: String,
}

#[derive(Serialize)]
pub(crate) struct FlashcardResponse {
    card_id: String,
    media_id: String,
    position_seconds: Option<i64>,
    front: String,
    back: String,
    interval_days: i64,
    next_due: i64,
    created_at: i64,
    updated_at: i64,
}

impl From<learnvault_core::Flashcard> for FlashcardResponse {
    fn from(c: learnvault_core::Flashcard) -> Self {
        Self {
            card_id: c.card_id,
            media_id: c.media_id,
            position_seconds: c.position_seconds,
            front: c.front,
            back: c.back,
            interval_days: c.interval_days,
            next_due: c.next_due,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

#[derive(Serialize)]
struct FlashcardsResponse {
    items: Vec<FlashcardResponse>,
}

#[derive(Serialize)]
pub(crate) struct FavoritesResponse {
    course_ids: Vec<String>,
    media_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CourseFavoriteRequest {
    course_id: String,
    favorite: bool,
}

#[derive(Serialize)]
pub(crate) struct CourseFavoriteResponse {
    course_id: String,
    favorite: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LessonFavoriteRequest {
    media_id: String,
    favorite: bool,
}

#[derive(Serialize)]
pub(crate) struct LessonFavoriteResponse {
    media_id: String,
    favorite: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

/// Progress of a media as the client consumes it.
///
/// `recorded` tells "never studied" from "studied at position zero": absence is not
/// an error in the domain, so the read answers `200` and signals the absence through the field.
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
struct SummaryResponse {
    current_streak: i64,
    longest_streak: i64,
    today: String,
    today_active: bool,
    at_risk: bool,
    week_active_days: i64,
    completed_lessons: i64,
    days: Vec<SummaryDay>,
    /// Active days of the last year, for the activity heatmap.
    active_days_year: Vec<String>,
}

#[derive(Serialize)]
struct SummaryDay {
    day: String,
    active: bool,
}

#[derive(Serialize)]
struct FocusSessionResponse {
    id: String,
    kind: &'static str,
    planned_seconds: i64,
    actual_seconds: i64,
    completed: bool,
    started_at: i64,
    ended_at: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FocusSessionsQuery {
    limit: Option<usize>,
}

#[derive(Serialize)]
struct FocusSessionsListResponse {
    items: Vec<FocusSessionResponse>,
}

#[derive(Serialize)]
struct FocusSummaryResponse {
    today_minutes: i64,
    week_minutes: i64,
    week_sessions: i64,
    total_minutes: i64,
    total_sessions: i64,
    completion_rate: i64,
    best_day: Option<String>,
    days: Vec<FocusDayResponse>,
}

#[derive(Serialize)]
struct FocusDayResponse {
    day: String,
    minutes: i64,
    sessions: i64,
}

#[derive(Serialize)]
struct MissionResponse {
    id: String,
    progress: i64,
    target: i64,
    claimed: bool,
}

#[derive(Serialize)]
struct RewardSummaryResponse {
    xp: i64,
    completed_lessons: i64,
    weekly_goal: i64,
    missions: Vec<MissionResponse>,
    study: SummaryResponse,
}

#[derive(Serialize)]
struct WeeklyGoalResponse {
    weekly_goal: i64,
}

/// Study capabilities document, the explicit contract for feature detection.
///
/// The shape mirrors the reference Go service so that the SPA detects features on both
/// transports, but the values report the real behaviour of this server. Rust's media
/// identity is the opaque `media_id` generated by the domain, which survives renames
/// — it is the reference Go's `relative_path` that is not stable. That is exactly why
/// the endpoint exists: so the client does not assume parity where there is none.
#[derive(Serialize)]
struct CapabilitiesResponse {
    study: bool,
    media_identity: &'static str,
    stable_media_identity: bool,
    recent_default_limit: usize,
    recent_max_limit: usize,
    removal_keeps_progress: bool,
}

/// Value of this server's `media_identity`: the identity is the opaque `media_id`, not the
/// file's relative path.
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

    /// Neutral state for a known media with no progress: nothing has been written yet.
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

/// Lists the authenticated user's progress, from most recently changed to oldest.
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
    let admin = user.is_admin;
    match run_study(&state, move |db| {
        let states = learnvault_core::recent_study(db, &owner, learnvault_core::MAX_RECENT_STUDY)?;
        Ok(states
            .into_iter()
            .filter(|progress| require_media_access(db, &owner, admin, &progress.media_id).is_ok())
            .take(limit)
            .collect::<Vec<_>>())
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

impl From<StudySummary> for SummaryResponse {
    fn from(summary: StudySummary) -> Self {
        Self {
            current_streak: summary.current_streak,
            longest_streak: summary.longest_streak,
            today: summary.today,
            today_active: summary.today_active,
            at_risk: summary.at_risk,
            week_active_days: summary.week_active_days,
            completed_lessons: summary.completed_lessons,
            days: summary
                .days
                .into_iter()
                .map(|day| SummaryDay {
                    day: day.day,
                    active: day.active,
                })
                .collect(),
            active_days_year: summary.active_days_year,
        }
    }
}

impl From<learnvault_core::FocusSession> for FocusSessionResponse {
    fn from(session: learnvault_core::FocusSession) -> Self {
        Self {
            id: session.id,
            kind: match session.kind {
                FocusKind::Focus => "focus",
                FocusKind::ShortBreak => "short",
                FocusKind::LongBreak => "long",
            },
            planned_seconds: session.planned_seconds,
            actual_seconds: session.actual_seconds,
            completed: session.completed,
            started_at: session.started_at,
            ended_at: session.ended_at,
        }
    }
}

impl From<learnvault_core::FocusSummary> for FocusSummaryResponse {
    fn from(summary: learnvault_core::FocusSummary) -> Self {
        Self {
            today_minutes: summary.today_minutes,
            week_minutes: summary.week_minutes,
            week_sessions: summary.week_sessions,
            total_minutes: summary.total_minutes,
            total_sessions: summary.total_sessions,
            completion_rate: summary.completion_rate,
            best_day: summary.best_day,
            days: summary
                .days
                .into_iter()
                .map(|day| FocusDayResponse {
                    day: day.day,
                    minutes: day.minutes,
                    sessions: day.sessions,
                })
                .collect(),
        }
    }
}

impl From<learnvault_core::RewardSummary> for RewardSummaryResponse {
    fn from(summary: learnvault_core::RewardSummary) -> Self {
        Self {
            xp: summary.xp,
            completed_lessons: summary.completed_lessons,
            weekly_goal: summary.weekly_goal,
            missions: summary
                .missions
                .into_iter()
                .map(|mission| MissionResponse {
                    id: mission.id,
                    progress: mission.progress,
                    target: mission.target,
                    claimed: mission.claimed,
                })
                .collect(),
            study: SummaryResponse::from(summary.study),
        }
    }
}

/// Summarizes the authenticated user's streak and current week.
pub(crate) async fn summary(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| learnvault_core::study_summary(db, &owner)).await {
        Ok(summary) => private_no_store(Json(SummaryResponse::from(summary)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Records a Pomodoro session in the authenticated owner's history. The ID makes retries idempotent.
pub(crate) async fn record_focus_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<FocusSessionRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let kind = match request.kind.as_str() {
        "focus" => FocusKind::Focus,
        "short" => FocusKind::ShortBreak,
        "long" => FocusKind::LongBreak,
        _ => {
            return error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_focus_session");
        }
    };
    let owner = user.user_id;
    let input = FocusSessionInput {
        id: request.id,
        kind,
        planned_seconds: request.planned_seconds,
        actual_seconds: request.actual_seconds,
        completed: request.completed,
        started_at: request.started_at,
        ended_at: request.ended_at,
    };
    match run_study(&state, move |db| {
        learnvault_core::record_focus_session(db, &owner, input)
    })
    .await
    {
        Ok(session) => private_no_store(Json(FocusSessionResponse::from(session)).into_response()),
        Err(failure) => focus_failure_response(failure),
    }
}

/// Lists the history of the authenticated owner's persisted focus sessions.
pub(crate) async fn list_focus_sessions(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let query =
        serde_urlencoded::from_str::<FocusSessionsQuery>(raw.as_deref().unwrap_or_default()).ok();
    let limit = query.and_then(|q| q.limit).unwrap_or(100);
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::list_focus_sessions(db, &owner, limit)
    })
    .await
    {
        Ok(sessions) => private_no_store(
            Json(FocusSessionsListResponse {
                items: sessions
                    .into_iter()
                    .map(FocusSessionResponse::from)
                    .collect(),
            })
            .into_response(),
        ),
        Err(failure) => focus_failure_response(failure),
    }
}

/// Summarizes the authenticated owner's persisted focus sessions.
pub(crate) async fn focus_summary(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| learnvault_core::focus_summary(db, &owner)).await {
        Ok(summary) => private_no_store(Json(FocusSummaryResponse::from(summary)).into_response()),
        Err(failure) => focus_failure_response(failure),
    }
}

/// Summarizes the XP ledger and the progress of the daily missions.
pub(crate) async fn reward_summary(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::reward_summary(db, &owner)
    })
    .await
    {
        Ok(summary) => private_no_store(Json(RewardSummaryResponse::from(summary)).into_response()),
        Err(failure) => focus_failure_response(failure),
    }
}

/// Redeems a completed daily mission, at most once per owner and day.
pub(crate) async fn claim_mission(
    State(state): State<AppState>,
    Path(mission_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::claim_daily_mission_today(db, &owner, &mission_id)
    })
    .await
    {
        Ok(claimed) => {
            private_no_store(Json(serde_json::json!({ "claimed": claimed })).into_response())
        }
        Err(failure) => focus_failure_response(failure),
    }
}

/// Reads the authenticated owner's weekly goal.
pub(crate) async fn weekly_goal(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| learnvault_core::weekly_goal(db, &owner)).await {
        Ok(weekly_goal) => {
            private_no_store(Json(WeeklyGoalResponse { weekly_goal }).into_response())
        }
        Err(failure) => focus_failure_response(failure),
    }
}

/// Updates the weekly goal, validated by the core and by the authenticated owner.
pub(crate) async fn set_weekly_goal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<WeeklyGoalRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    match run_study(&state, move |db| {
        learnvault_core::set_weekly_goal(db, &owner, request.weekly_goal)
    })
    .await
    {
        Ok(weekly_goal) => {
            private_no_store(Json(WeeklyGoalResponse { weekly_goal }).into_response())
        }
        Err(failure) => focus_failure_response(failure),
    }
}

/// Reads the authenticated user's progress on a media.
///
/// A known media with no progress answers `200` with `recorded: false`; a media
/// unknown to the catalog answers `404`.
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
    let admin = user.is_admin;
    let media_id = query.media_id;
    let lookup = media_id.clone();
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &lookup)?;
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

/// Records the authenticated user's resume position, preserving completion.
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
    let admin = user.is_admin;
    let media_id = request.media_id;
    let lookup = media_id.clone();
    let position_seconds = request.position_seconds;
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &lookup)?;
        learnvault_core::browser_write(
            db,
            &owner,
            || learnvault_core::record_position(db, &owner, &media_id, position_seconds),
            |device| {
                learnvault_core::queue_position(db, &owner, device, &media_id, position_seconds)
            },
        )
    })
    .await
    {
        Ok(progress) => {
            private_no_store(Json(ProgressResponse::recorded(progress)).into_response())
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Marks or unmarks the authenticated user's completion, preserving the position.
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
    let admin = user.is_admin;
    let media_id = request.media_id;
    let lookup = media_id.clone();
    let completed = request.completed;
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &lookup)?;
        learnvault_core::browser_write(
            db,
            &owner,
            || learnvault_core::set_completed(db, &owner, &media_id, completed),
            |device| learnvault_core::queue_completed(db, &owner, device, &media_id, completed),
        )
    })
    .await
    {
        Ok(progress) => {
            private_no_store(Json(ProgressResponse::recorded(progress)).into_response())
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Lists the authenticated user's notes for the requested media.
pub(crate) async fn list_notes(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) =
        serde_urlencoded::from_str::<NotesQuery>(raw.as_deref().unwrap_or_default()).ok()
    else {
        return invalid_query();
    };
    let owner = user.user_id;
    let admin = user.is_admin;
    if let Some(media_id) = query.media_id.filter(|s| !s.trim().is_empty()) {
        let lookup = media_id.clone();
        match run_study(&state, move |db| {
            require_media_access(db, &owner, admin, &lookup)?;
            learnvault_core::list_study_notes(db, &owner, &media_id)
        })
        .await
        {
            Ok(notes) => private_no_store(
                Json(NotesResponse {
                    items: notes.into_iter().map(StudyNoteResponse::from).collect(),
                })
                .into_response(),
            ),
            Err(failure) => study_failure_response(failure),
        }
    } else {
        match run_study(&state, move |db| {
            let notes = learnvault_core::list_all_study_notes(db, &owner)?;
            Ok(notes
                .into_iter()
                .filter(|note| require_media_access(db, &owner, admin, &note.media_id).is_ok())
                .collect::<Vec<_>>())
        })
        .await
        {
            Ok(notes) => private_no_store(
                Json(NotesResponse {
                    items: notes.into_iter().map(StudyNoteResponse::from).collect(),
                })
                .into_response(),
            ),
            Err(failure) => study_failure_response(failure),
        }
    }
}

/// Creates a study note linked to the media and to the current timestamp.
pub(crate) async fn create_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateNoteRequest>,
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
    let lookup = request.media_id.clone();
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &lookup)?;
        learnvault_core::browser_write(
            db,
            &owner,
            || {
                learnvault_core::create_study_note(
                    db,
                    &owner,
                    &request.media_id,
                    request.position_seconds,
                    &request.content,
                )
            },
            |device| {
                learnvault_core::queue_note_create(
                    db,
                    &owner,
                    device,
                    &request.media_id,
                    request.position_seconds,
                    &request.content,
                )
            },
        )
    })
    .await
    {
        Ok(note) => private_no_store(
            (StatusCode::CREATED, Json(StudyNoteResponse::from(note))).into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Updates the content of an existing study note.
pub(crate) async fn update_note(
    State(state): State<AppState>,
    Path(note_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<UpdateNoteRequest>,
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
    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::study_note_media_id(db, &note_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        learnvault_core::browser_write(
            db,
            &owner,
            || learnvault_core::update_study_note(db, &owner, &note_id, &request.content),
            |device| {
                learnvault_core::queue_note_update(db, &owner, device, &note_id, &request.content)
            },
        )
    })
    .await
    {
        Ok(note) => private_no_store(Json(StudyNoteResponse::from(note)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Deletes a study note that belongs to the user.
pub(crate) async fn delete_note(
    State(state): State<AppState>,
    Path(note_id): Path<String>,
    headers: HeaderMap,
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
    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::study_note_media_id(db, &note_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        // The core delete records a tombstone, so an offline create cannot resurrect the
        // note. For an owner who syncs, the deletion is also journaled as device `server`
        // (ADR 0005), so a desktop receives it.
        learnvault_core::browser_write(
            db,
            &owner,
            || learnvault_core::delete_study_note(db, &owner, &note_id),
            |device| learnvault_core::queue_note_delete(db, &owner, device, &note_id),
        )
    })
    .await
    {
        Ok(deleted) => {
            if deleted {
                status_private_no_store(StatusCode::NO_CONTENT)
            } else {
                error_response(StatusCode::NOT_FOUND, "note_not_found")
            }
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Lists the authenticated user's timestamp bookmarks for one lesson.
pub(crate) async fn list_bookmarks(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) =
        serde_urlencoded::from_str::<BookmarksQuery>(raw.as_deref().unwrap_or_default()).ok()
    else {
        return invalid_query();
    };
    let owner = user.user_id;
    let admin = user.is_admin;
    let lookup = query.media_id.clone();
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &lookup)?;
        learnvault_core::list_media_bookmarks(db, &owner, &query.media_id)
    })
    .await
    {
        Ok(items) => private_no_store(
            Json(BookmarksResponse {
                items: items.into_iter().map(MediaBookmarkResponse::from).collect(),
            })
            .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Creates a timestamp bookmark for the authenticated user.
pub(crate) async fn create_bookmark(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateBookmarkRequest>,
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
    let lookup = request.media_id.clone();
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &lookup)?;
        learnvault_core::browser_write(
            db,
            &owner,
            || {
                learnvault_core::create_media_bookmark(
                    db,
                    &owner,
                    &request.media_id,
                    request.position_seconds,
                    &request.label,
                )
            },
            |device| {
                learnvault_core::queue_bookmark_create(
                    db,
                    &owner,
                    device,
                    &request.media_id,
                    request.position_seconds,
                    &request.label,
                )
            },
        )
    })
    .await
    {
        Ok(bookmark) => private_no_store(
            (
                StatusCode::CREATED,
                Json(MediaBookmarkResponse::from(bookmark)),
            )
                .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Deletes one of the authenticated user's timestamp bookmarks.
pub(crate) async fn delete_bookmark(
    State(state): State<AppState>,
    Path(bookmark_id): Path<String>,
    headers: HeaderMap,
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
    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::media_bookmark_media_id(db, &bookmark_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        // Same as `delete_note`: the tombstone blocks a stale offline create, and the deletion
        // is journaled as device `server` for an owner who syncs (ADR 0005).
        learnvault_core::browser_write(
            db,
            &owner,
            || learnvault_core::delete_media_bookmark(db, &owner, &bookmark_id),
            |device| learnvault_core::queue_bookmark_delete(db, &owner, device, &bookmark_id),
        )
    })
    .await
    {
        Ok(true) => status_private_no_store(StatusCode::NO_CONTENT),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "bookmark_not_found"),
        Err(failure) => study_failure_response(failure),
    }
}

/// Lists the authenticated user's annotations on one PDF.
pub(crate) async fn list_pdf_annotations(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) =
        serde_urlencoded::from_str::<PdfAnnotationsQuery>(raw.as_deref().unwrap_or_default()).ok()
    else {
        return invalid_query();
    };
    let owner = user.user_id;
    let admin = user.is_admin;
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &query.media_id)?;
        learnvault_core::list_pdf_annotations(db, &owner, &query.media_id)
    })
    .await
    {
        Ok(items) => private_no_store(
            Json(PdfAnnotationsResponse {
                items: items.into_iter().map(PdfAnnotationResponse::from).collect(),
            })
            .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Creates an annotation. An unknown kind or color is the same input error as a bad rectangle.
pub(crate) async fn create_pdf_annotation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreatePdfAnnotationRequest>,
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
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &request.media_id)?;
        let (Some(kind), Some(color)) = (
            learnvault_core::PdfAnnotationKind::parse(&request.kind),
            learnvault_core::PdfAnnotationColor::parse(&request.color),
        ) else {
            return Err(learnvault_core::Error::InvalidPdfAnnotation);
        };
        learnvault_core::create_pdf_annotation(
            db,
            &owner,
            &request.media_id,
            learnvault_core::NewPdfAnnotation {
                page: request.page,
                kind,
                color,
                rects: request
                    .rects
                    .into_iter()
                    .map(|rect| learnvault_core::PdfRect {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height,
                    })
                    .collect(),
                quote: request.quote,
                content: request.content,
            },
        )
    })
    .await
    {
        Ok(annotation) => private_no_store(
            (
                StatusCode::CREATED,
                Json(PdfAnnotationResponse::from(annotation)),
            )
                .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Changes an annotation's color and/or text.
pub(crate) async fn update_pdf_annotation(
    State(state): State<AppState>,
    Path(annotation_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<UpdatePdfAnnotationRequest>,
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
    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::pdf_annotation_media_id(db, &annotation_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        let color = match request.color.as_deref() {
            Some(value) => Some(
                learnvault_core::PdfAnnotationColor::parse(value)
                    .ok_or(learnvault_core::Error::InvalidPdfAnnotation)?,
            ),
            None => None,
        };
        learnvault_core::update_pdf_annotation(
            db,
            &owner,
            &annotation_id,
            color,
            request.content.as_deref(),
        )
    })
    .await
    {
        Ok(annotation) => {
            private_no_store(Json(PdfAnnotationResponse::from(annotation)).into_response())
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Deletes one of the user's annotations.
pub(crate) async fn delete_pdf_annotation(
    State(state): State<AppState>,
    Path(annotation_id): Path<String>,
    headers: HeaderMap,
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
    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::pdf_annotation_media_id(db, &annotation_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        learnvault_core::delete_pdf_annotation(db, &owner, &annotation_id)
    })
    .await
    {
        Ok(true) => status_private_no_store(StatusCode::NO_CONTENT),
        Ok(false) => error_response(StatusCode::NOT_FOUND, "pdf_annotation_not_found"),
        Err(failure) => study_failure_response(failure),
    }
}

/// Lists the user's memorization cards for the lesson.
pub(crate) async fn list_flashcards(
    State(state): State<AppState>,
    RawQuery(raw): RawQuery,
    headers: HeaderMap,
) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let Some(query) =
        serde_urlencoded::from_str::<FlashcardsQuery>(raw.as_deref().unwrap_or_default()).ok()
    else {
        return invalid_query();
    };
    let owner = user.user_id;
    let admin = user.is_admin;
    let media_id = query.media_id.filter(|id| !id.trim().is_empty());
    match run_study(&state, move |db| match media_id {
        Some(media_id) => {
            require_media_access(db, &owner, admin, &media_id)?;
            learnvault_core::list_flashcards(db, &owner, &media_id)
        }
        None => {
            // Every card of the owner, but only for lessons they may still open.
            let cards = learnvault_core::list_all_flashcards(db, &owner)?;
            Ok(cards
                .into_iter()
                .filter(|card| require_media_access(db, &owner, admin, &card.media_id).is_ok())
                .collect::<Vec<_>>())
        }
    })
    .await
    {
        Ok(cards) => private_no_store(
            Json(FlashcardsResponse {
                items: cards.into_iter().map(FlashcardResponse::from).collect(),
            })
            .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Creates a study card linked to the lesson.
pub(crate) async fn create_flashcard(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateFlashcardRequest>,
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
    let lookup = request.media_id.clone();
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &lookup)?;
        learnvault_core::browser_write(
            db,
            &owner,
            || {
                learnvault_core::create_flashcard(
                    db,
                    &owner,
                    &request.media_id,
                    &request.front,
                    &request.back,
                    request.position_seconds,
                )
            },
            |device| {
                learnvault_core::queue_flashcard_create(
                    db,
                    &owner,
                    device,
                    &request.media_id,
                    &request.front,
                    &request.back,
                    request.position_seconds,
                )
            },
        )
    })
    .await
    {
        Ok(card) => private_no_store(
            (StatusCode::CREATED, Json(FlashcardResponse::from(card))).into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Updates the front and/or back of a study card.
pub(crate) async fn update_flashcard(
    State(state): State<AppState>,
    Path(card_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<UpdateFlashcardRequest>,
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
    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::flashcard_media_id(db, &card_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        learnvault_core::browser_write(
            db,
            &owner,
            || {
                learnvault_core::update_flashcard(
                    db,
                    &owner,
                    &card_id,
                    &request.front,
                    &request.back,
                )
            },
            |device| {
                learnvault_core::queue_flashcard_update(
                    db,
                    &owner,
                    device,
                    &card_id,
                    &request.front,
                    &request.back,
                )
            },
        )
    })
    .await
    {
        Ok(card) => private_no_store(Json(FlashcardResponse::from(card)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Deletes a study card of the user.
pub(crate) async fn delete_flashcard(
    State(state): State<AppState>,
    Path(card_id): Path<String>,
    headers: HeaderMap,
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
    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::flashcard_media_id(db, &card_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        learnvault_core::browser_write(
            db,
            &owner,
            || learnvault_core::delete_flashcard(db, &owner, &card_id),
            |device| learnvault_core::queue_flashcard_delete(db, &owner, device, &card_id),
        )
    })
    .await
    {
        Ok(deleted) => {
            if deleted {
                status_private_no_store(StatusCode::NO_CONTENT)
            } else {
                error_response(StatusCode::NOT_FOUND, "flashcard_not_found")
            }
        }
        Err(failure) => study_failure_response(failure),
    }
}

/// Records a card review (Again or Good).
pub(crate) async fn review_flashcard(
    State(state): State<AppState>,
    Path(card_id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ReviewFlashcardRequest>,
) -> Response {
    if !same_origin(&headers) {
        return csrf_failed();
    }
    let Some(rating) = learnvault_core::ReviewRating::parse(&request.rating) else {
        return error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_flashcard_rating");
    };
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    let admin = user.is_admin;
    match run_study(&state, move |db| {
        if let Some(media_id) = learnvault_core::flashcard_media_id(db, &card_id)? {
            require_media_access(db, &owner, admin, &media_id)?;
        }
        learnvault_core::browser_write(
            db,
            &owner,
            || learnvault_core::review_flashcard(db, &owner, &card_id, rating),
            |device| learnvault_core::queue_review(db, &owner, device, &card_id, rating),
        )
    })
    .await
    {
        Ok(card) => private_no_store(Json(FlashcardResponse::from(card)).into_response()),
        Err(failure) => study_failure_response(failure),
    }
}

/// Lists all courses and lessons favourited by the authenticated user.
pub(crate) async fn list_favorites(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let user = match user_or_response(&state, &headers).await {
        Ok(user) => user,
        Err(failure) => return failure.into_response(),
    };
    let owner = user.user_id;
    let admin = user.is_admin;
    match run_study(&state, move |db| {
        let mut favorites = learnvault_core::list_favorites(db, &owner)?;
        favorites.course_ids.retain(|course_id| {
            learnvault_core::can_user_access_course(db, &owner, admin, course_id).unwrap_or(false)
        });
        favorites
            .media_ids
            .retain(|media_id| require_media_access(db, &owner, admin, media_id).is_ok());
        Ok(favorites)
    })
    .await
    {
        Ok(favs) => private_no_store(
            Json(FavoritesResponse {
                course_ids: favs.course_ids,
                media_ids: favs.media_ids,
            })
            .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Marks or unmarks a course as a favourite for the authenticated user.
pub(crate) async fn set_course_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CourseFavoriteRequest>,
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
    let course_id = request.course_id.trim().to_owned();
    let lookup_id = course_id.clone();
    let favorite = request.favorite;
    match run_study(&state, move |db| {
        if !learnvault_core::can_user_access_course(db, &owner, admin, &lookup_id)? {
            return Err(learnvault_core::Error::MediaNotFound {
                media_id: lookup_id,
            });
        }
        learnvault_core::set_course_favorite(db, &owner, &lookup_id, favorite)
    })
    .await
    {
        Ok(saved) => private_no_store(
            Json(CourseFavoriteResponse {
                course_id,
                favorite: saved,
            })
            .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Marks or unmarks a lesson as a favourite for the authenticated user.
pub(crate) async fn set_lesson_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LessonFavoriteRequest>,
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
    let media_id = request.media_id.trim().to_owned();
    let lookup_id = media_id.clone();
    let favorite = request.favorite;
    match run_study(&state, move |db| {
        require_media_access(db, &owner, admin, &lookup_id)?;
        learnvault_core::set_lesson_favorite(db, &owner, &lookup_id, favorite)
    })
    .await
    {
        Ok(saved) => private_no_store(
            Json(LessonFavoriteResponse {
                media_id,
                favorite: saved,
            })
            .into_response(),
        ),
        Err(failure) => study_failure_response(failure),
    }
}

/// Answers the Study capabilities contract.
///
/// It requires the same authenticated session (and the same refusal of the temporary session) as the other
/// Study read routes, without reading or writing the catalog, study state or physical
/// library. The response is always `private, no-store`. Reading the session takes the *blocking*
/// capacity like the other routes, so under saturation it answers `503` instead of holding the
/// database mutex.
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

/// Failure of the study layer, before translation to HTTP.
pub(crate) enum StudyFailure {
    /// There is no capacity to start another blocking task.
    Busy,
    /// The blocking task could not finish.
    Blocking,
    /// The domain refused the operation.
    Core(learnvault_core::Error),
}

pub(crate) fn require_media_access(
    db: &Database,
    user_id: &str,
    is_admin: bool,
    media_id: &str,
) -> learnvault_core::Result<()> {
    let Some(library_id) = learnvault_core::find_media_library_id(db, media_id)? else {
        return Err(learnvault_core::Error::MediaNotFound {
            media_id: media_id.to_owned(),
        });
    };
    if learnvault_core::can_user_access_course(db, user_id, is_admin, &library_id)? {
        Ok(())
    } else {
        Err(learnvault_core::Error::MediaNotFound {
            media_id: media_id.to_owned(),
        })
    }
}

/// Runs a core operation over the database, off the Tokio threads.
///
/// The permit and the database guard are moved into the closure, so they stay accounted for
/// until the work ends, even if the HTTP request expires first.
pub(crate) async fn run_study<T, F>(state: &AppState, work: F) -> Result<T, StudyFailure>
where
    F: FnOnce(&Database) -> learnvault_core::Result<T> + Send + 'static,
    T: Send + 'static,
{
    // Avoids holding the database mutex just to return 503 under saturation.
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

pub(crate) fn study_failure_response(failure: StudyFailure) -> Response {
    use learnvault_core::Error;

    match failure {
        StudyFailure::Busy => service_unavailable(),
        StudyFailure::Blocking => internal_error("study unavailable"),
        // A missing media is 404; a negative position is an input validation (422).
        // Any other core error is operational and cannot become 404, so as not to
        // confuse legitimate absence with a corrupted database.
        StudyFailure::Core(Error::MediaNotFound { .. }) => {
            error_response(StatusCode::NOT_FOUND, "media_not_found")
        }
        StudyFailure::Core(Error::InvalidStudyPosition { .. }) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_position")
        }
        StudyFailure::Core(Error::InvalidStudyNoteContent) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_note_content")
        }
        StudyFailure::Core(Error::StudyNoteNotFound { .. }) => {
            error_response(StatusCode::NOT_FOUND, "note_not_found")
        }
        StudyFailure::Core(Error::InvalidPdfAnnotation) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_pdf_annotation")
        }
        StudyFailure::Core(Error::PdfAnnotationNotFound { .. }) => {
            error_response(StatusCode::NOT_FOUND, "pdf_annotation_not_found")
        }
        StudyFailure::Core(Error::InvalidFlashcardContent) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_flashcard_content",
        ),
        StudyFailure::Core(Error::InvalidFlashcardRating) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_flashcard_rating")
        }
        StudyFailure::Core(Error::FlashcardNotFound { .. }) => {
            error_response(StatusCode::NOT_FOUND, "flashcard_not_found")
        }
        StudyFailure::Core(Error::InvalidFavoriteTarget) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_favorite_target")
        }
        StudyFailure::Core(Error::InvalidBookmarkLabel) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_bookmark_label")
        }
        StudyFailure::Core(Error::BookmarkNotFound { .. }) => {
            error_response(StatusCode::NOT_FOUND, "bookmark_not_found")
        }
        StudyFailure::Core(Error::InvalidPlayerPreference) => error_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_player_preference",
        ),
        StudyFailure::Core(Error::InvalidSyncOperation { .. }) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_sync_operation")
        }
        StudyFailure::Core(_) => internal_error("study unavailable"),
    }
}

fn focus_failure_response(failure: StudyFailure) -> Response {
    use learnvault_core::Error;

    match failure {
        StudyFailure::Busy => service_unavailable(),
        StudyFailure::Blocking => internal_error("focus unavailable"),
        StudyFailure::Core(Error::InvalidFocusSession { .. }) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_focus_session")
        }
        StudyFailure::Core(Error::FocusSessionConflict) => {
            error_response(StatusCode::CONFLICT, "focus_session_conflict")
        }
        StudyFailure::Core(Error::InvalidMission { .. }) => {
            error_response(StatusCode::NOT_FOUND, "mission_not_found")
        }
        StudyFailure::Core(Error::MissionNotComplete) => {
            error_response(StatusCode::CONFLICT, "mission_not_complete")
        }
        StudyFailure::Core(Error::InvalidWeeklyGoal { .. }) => {
            error_response(StatusCode::UNPROCESSABLE_ENTITY, "invalid_weekly_goal")
        }
        StudyFailure::Core(_) => internal_error("focus unavailable"),
    }
}

/// Authentication/limit failure before translation to HTTP.
pub(crate) enum AuthFailure {
    Unauthorized,
    CredentialsRequired,
    Busy,
}

impl AuthFailure {
    pub(crate) fn into_response(self) -> Response {
        match self {
            AuthFailure::Unauthorized => status_private_no_store(StatusCode::UNAUTHORIZED),
            AuthFailure::CredentialsRequired => credentials_required(),
            AuthFailure::Busy => service_unavailable(),
        }
    }
}

pub(crate) async fn user_or_response(
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

pub(crate) fn invalid_query() -> Response {
    error_response(StatusCode::BAD_REQUEST, "invalid_query")
}

pub(crate) fn error_response(status: StatusCode, error: &'static str) -> Response {
    private_no_store((status, Json(ErrorResponse { error })).into_response())
}

fn credentials_required() -> Response {
    error_response(StatusCode::FORBIDDEN, "credentials_change_required")
}

pub(crate) fn csrf_failed() -> Response {
    error_response(StatusCode::FORBIDDEN, "csrf_failed")
}

pub(crate) fn private_no_store(mut response: Response) -> Response {
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

fn status_private_no_store(status: StatusCode) -> Response {
    private_no_store(status.into_response())
}
