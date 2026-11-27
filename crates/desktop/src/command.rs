//! Desktop IPC commands and the free functions they wrap.
//!
//! Each `#[tauri::command]` wrapper is thin: it delegates to a free function that knows nothing of
//! Tauri. The tests exercise the free functions; the webview runtime stays outside them.
//!
//! The import command takes no arguments: the webview only triggers the native
//! picker, which runs in Rust, and gets back only the configuration boolean. The
//! removal takes only the course's opaque identity — never a path.
//!
//! The study commands take only catalog data (`media_id`, position, completion and
//! limit). The owner of the progress is fixed in Rust: no command accepts an `owner_id`.

use std::path::{Path, PathBuf};

use learnvault_core::{Database, FocusKind, FocusSessionInput};
use tauri::State;

use crate::config::DesktopConfigDto;
use crate::courses::CourseEntry;
use crate::dto::{
    CatalogDto, CatalogNodeDto, CoverDto, CoverRemovalDto, FlashcardDto, FlashcardReviewsDto,
    FocusSessionDto, FocusSummaryDto, MediaBookmarkDto, MediaRemovalImpactDto,
    MediaRemovalOutcomeDto, NewPdfAnnotationDto, PdfAnnotationDto, PlaybackPreferencesDto,
    RecentStudyDto, RescanSummaryDto, RewardSummaryDto, SearchResultsDto, StudyFavoritesDto,
    StudyNoteDto, StudyProgressDto, StudyStateDto, StudySummaryDto, SyncResultDto, SyncStateDto,
};
use crate::error::IpcError;
use crate::ordering::CourseOrders;
use crate::state::DesktopState;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
/// Camel-case IPC payload for recording one focus session.
pub struct FocusSessionRequest {
    id: String,
    kind: String,
    planned_seconds: i64,
    actual_seconds: i64,
    completed: bool,
    started_at: i64,
    ended_at: i64,
}

/// Builds the aggregated catalog DTO from the data persisted in the local database.
///
/// A free, testable function: it knows nothing of Tauri and any failure of the core is translated
/// into a sanitized IPC error, without echoing the domain's `Display` error. Each registered
/// course becomes a top-level node with an opaque identity, a title derived from the folder and the
/// custom cover's version (derived from the covers directory, never from a path of the course),
/// exposed as `nodes` (canonical tree) and as `categories` (list of top-level courses).
///
/// The ordering preference is applied as an overlay over the natural order of each
/// course's subtree; a course with no preference stays in the automatic order.
pub fn catalog(
    database: &Database,
    courses: &[CourseEntry],
    covers_dir: &Path,
) -> Result<CatalogDto, IpcError> {
    catalog_with_orders(database, courses, &CourseOrders::new(), covers_dir)
}

/// Like [`catalog`], applying the per-course manual ordering preferences.
pub fn catalog_with_orders(
    database: &Database,
    courses: &[CourseEntry],
    orders: &CourseOrders,
    covers_dir: &Path,
) -> Result<CatalogDto, IpcError> {
    let empty = learnvault_core::CatalogOrder::new();
    let mut complete = true;
    let mut nodes = Vec::with_capacity(courses.len());
    let mut courses: Vec<_> = courses.iter().collect();
    courses.sort_by(|a, b| {
        let a_name = a
            .root
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let b_name = b
            .root
            .as_path()
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        learnvault_core::natural_cmp(a_name, b_name).then_with(|| a.id.cmp(&b.id))
    });
    for course in courses {
        let inner = learnvault_core::build_catalog(database, &course.id)
            .map_err(|_| IpcError::catalog_unavailable())?;
        let is_complete = inner.complete;
        let (mut categories, loose_files) = learnvault_core::shape_course_catalog(inner);
        learnvault_core::apply_order(&mut categories, orders.get(&course.id).unwrap_or(&empty));
        let shaped = learnvault_core::Catalog {
            complete: is_complete,
            loose_files,
            categories,
        };
        complete = complete && shaped.complete;
        let source_folder = learnvault_core::course_source_folder(course.root.as_path());
        nodes.push(CatalogNodeDto::course(
            &course.id,
            &course.title(),
            shaped,
            crate::covers::cover_version(covers_dir, &course.id),
            source_folder,
        ));
    }
    Ok(CatalogDto {
        complete,
        loose_files: Vec::new(),
        // The same top-level course node feeds both fields: `nodes` is the canonical tree
        // and `categories` is the list of top-level courses of the IPC contract.
        nodes: nodes.clone(),
        categories: nodes,
    })
}

/// IPC command: returns the desktop's catalog.
///
/// The command is `async` on purpose: a synchronous Tauri command runs on the webview's main
/// thread, and the blocking SQLite read would freeze the UI. The work is moved to
/// the Tauri runtime's `spawn_blocking` — without adding `tokio` as a direct dependency —
/// keeping the contract, the errors, the state and the free functions testable.
#[tauri::command]
pub async fn desktop_catalog(state: State<'_, DesktopState>) -> Result<CatalogDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.catalog())
        .await
        .map_err(|_| IpcError::database_unavailable())?
}

/// IPC command: opens the native picker and registers the chosen folder as a course.
///
/// No arguments: the webview sends no path at all. The picker and the write run in
/// `spawn_blocking` so as not to hold the main thread, and the result is only the configuration
/// boolean. Cancelling preserves the previous state; a failure becomes a sanitized error.
#[tauri::command]
pub async fn desktop_import_course(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<DesktopConfigDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.import_course(|| pick_folder(&app)))
        .await
        .map_err(|_| IpcError::courses_config_unavailable())?
}

/// IPC command: opens the native image picker and registers a course's cover.
///
/// The webview sends only the course's opaque identity — never a path. The picker, the
/// read and the atomic write run in `spawn_blocking` so as not to hold the main thread. The
/// type (magic bytes) and the size are validated in Rust: an image outside the contract becomes
/// `invalid_cover` and nothing is written. Cancelling preserves the previous cover.
#[tauri::command]
pub async fn desktop_import_course_cover(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
    course_id: String,
) -> Result<CoverDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.import_course_cover(&course_id, || pick_cover(&app))
    })
    .await
    .map_err(|_| IpcError::cover_unavailable())?
}

/// IPC command: removes a course's custom cover by its opaque identity.
///
/// The removal does not touch the course folder and is idempotent: a course with no cover returns
/// `removed: false`. No path crosses the IPC in either direction.
#[tauri::command]
pub async fn desktop_remove_course_cover(
    state: State<'_, DesktopState>,
    course_id: String,
) -> Result<CoverRemovalDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.remove_course_cover(&course_id))
        .await
        .map_err(|_| IpcError::cover_unavailable())?
}

/// IPC command: removes a course from the registry by its opaque identity.
///
/// The removal does not touch the files and is idempotent: an unknown identity only
/// returns the current state. The webview never sends nor receives a path.
#[tauri::command]
pub async fn desktop_remove_course(
    state: State<'_, DesktopState>,
    course_id: String,
) -> Result<DesktopConfigDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.remove_course(&course_id))
        .await
        .map_err(|_| IpcError::courses_config_unavailable())?
}

/// IPC command: sets the manual order of the siblings of a course node.
///
/// The webview sends only the course's opaque identity, the parent's relative path and the
/// desired order of the children (relative paths). No file-system path
/// crosses the IPC. The validation (exact permutation of the siblings) and the atomic write run in
/// Rust, in `spawn_blocking`; an invalid list becomes `invalid_order` without writing anything.
#[tauri::command]
pub async fn desktop_set_course_order(
    state: State<'_, DesktopState>,
    course_id: String,
    parent: String,
    children: Vec<String>,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.set_course_order(&course_id, &parent, &children)
    })
    .await
    .map_err(|_| IpcError::order_unavailable())?
}

/// IPC command: restores the automatic order of a specific parent or of the whole course.
///
/// An absent `parent` restores the whole course. The response says whether there was a preference to
/// remove, so the interface is idempotent without touching the course files.
#[tauri::command]
pub async fn desktop_reset_course_order(
    state: State<'_, DesktopState>,
    course_id: String,
    parent: Option<String>,
) -> Result<bool, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.reset_course_order(&course_id, parent.as_deref())
    })
    .await
    .map_err(|_| IpcError::order_unavailable())?
}

/// IPC command: runs the explicit rescan of the configured library.
///
/// No arguments: the webview sends no path nor any parameter, it only triggers the
/// indexing when the user asks. The scan reads the file system and SQLite in a
/// blocking way, so it runs in `spawn_blocking` — like the catalog —, without direct
/// `tokio`. The result is the core's sanitized summary; with no configured root or with the
/// root inaccessible the failure becomes a stable error. Concurrent calls return
/// `rescan_in_progress`, without cancelling or corrupting the state.
#[tauri::command]
pub async fn desktop_rescan(state: State<'_, DesktopState>) -> Result<RescanSummaryDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.rescan())
        .await
        .map_err(|_| IpcError::rescan_unavailable())?
}

/// IPC command: searches notes and catalog in the local database.
///
/// Course titles come from the registry, so the index is synced before the query;
/// the validation of the text and the limit is in the core.
#[tauri::command]
pub async fn desktop_search(
    state: State<'_, DesktopState>,
    query: String,
    limit: Option<usize>,
) -> Result<SearchResultsDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.search(&query, limit))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: reads the local owner's study progress on a known media.
///
/// The only argument is the `media_id` coming from the catalog: the owner is the fixed local identifier
/// resolved in Rust ([`crate::study::LOCAL_OWNER_ID`]) and never crosses the IPC. Like the
/// catalog, the blocking SQLite read runs in `spawn_blocking` so as not to hold the
/// main thread. A known media with no progress answers `recorded: false`; an
/// unknown media becomes the stable error `media_not_found`.
#[tauri::command]
pub async fn desktop_study_progress(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<StudyProgressDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.study_progress(&media_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: records the local owner's resume position, preserving completion.
///
/// The webview sends only `media_id` and `position_seconds` — never the owner. The rule of
/// independence between position and completion, the validation of a negative position and the
/// idempotence of a repeated value belong to the core; the command only delegates in
/// `spawn_blocking`.
#[tauri::command]
pub async fn desktop_record_position(
    state: State<'_, DesktopState>,
    media_id: String,
    position_seconds: i64,
) -> Result<StudyProgressDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.record_position(&media_id, position_seconds))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: marks or unmarks the local owner's completion, preserving the position.
///
/// The webview sends only `media_id` and `completed`; the owner stays in Rust. An unknown
/// media becomes the stable error `media_not_found` and nothing is written.
#[tauri::command]
pub async fn desktop_set_completed(
    state: State<'_, DesktopState>,
    media_id: String,
    completed: bool,
) -> Result<StudyProgressDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.set_completed(&media_id, completed))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: lists the local owner's study progress, from newest to oldest.
///
/// The limit is optional; without it the adapter applies the contract's default and the core's ceiling
/// still holds. The history always belongs to the local owner: the webview does not send `owner_id`.
#[tauri::command]
pub async fn desktop_recent_study(
    state: State<'_, DesktopState>,
    limit: Option<usize>,
) -> Result<RecentStudyDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.recent_study(limit))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: one page of the local owner's whole study state, newest first.
///
/// The cursor is the opaque `next_cursor` of the previous page; the webview never sends an owner.
#[tauri::command]
pub async fn desktop_study_state(
    state: State<'_, DesktopState>,
    limit: Option<usize>,
    cursor: Option<String>,
) -> Result<StudyStateDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.study_state(limit, cursor.as_deref()))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: summary of the local owner's streak and active days.
///
/// The webview does not send `owner_id`; the summary covers the whole local history and the current
/// week, always in the desktop's time zone.
#[tauri::command]
pub async fn desktop_study_summary(
    state: State<'_, DesktopState>,
) -> Result<StudySummaryDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.study_summary())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: lists the local owner's notes.
///
/// If `mediaId` is given, it filters by that media; otherwise, it returns all
/// the notes recorded for global search.
#[tauri::command]
pub async fn desktop_list_notes(
    state: State<'_, DesktopState>,
    media_id: Option<String>,
) -> Result<Vec<StudyNoteDto>, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_notes(media_id.as_deref()))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: creates a new study note for the local owner.
#[tauri::command]
pub async fn desktop_create_note(
    state: State<'_, DesktopState>,
    media_id: String,
    position_seconds: i64,
    content: String,
) -> Result<StudyNoteDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.create_note(&media_id, position_seconds, &content)
    })
    .await
    .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: updates the text of a note that belongs to the local owner.
#[tauri::command]
pub async fn desktop_update_note(
    state: State<'_, DesktopState>,
    note_id: String,
    content: String,
) -> Result<StudyNoteDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.update_note(&note_id, &content))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: deletes a study note that belongs to the local owner.
#[tauri::command]
pub async fn desktop_delete_note(
    state: State<'_, DesktopState>,
    note_id: String,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.delete_note(&note_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: lists the local owner's timestamp bookmarks for one lesson.
#[tauri::command]
pub async fn desktop_list_bookmarks(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<Vec<MediaBookmarkDto>, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_bookmarks(&media_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: creates a timestamp bookmark for the local owner.
#[tauri::command]
pub async fn desktop_create_bookmark(
    state: State<'_, DesktopState>,
    media_id: String,
    position_seconds: i64,
    label: String,
) -> Result<MediaBookmarkDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.create_bookmark(&media_id, position_seconds, &label)
    })
    .await
    .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: deletes one of the local owner's timestamp bookmarks.
#[tauri::command]
pub async fn desktop_delete_bookmark(
    state: State<'_, DesktopState>,
    bookmark_id: String,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.delete_bookmark(&bookmark_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: lists the local owner's annotations on one PDF.
#[tauri::command]
pub async fn desktop_list_pdf_annotations(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<Vec<PdfAnnotationDto>, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_pdf_annotations(&media_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: creates an annotation on a PDF for the local owner.
#[tauri::command]
pub async fn desktop_create_pdf_annotation(
    state: State<'_, DesktopState>,
    media_id: String,
    input: NewPdfAnnotationDto,
) -> Result<PdfAnnotationDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.create_pdf_annotation(&media_id, input))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: changes the color and/or text of one of the local owner's annotations.
#[tauri::command]
pub async fn desktop_update_pdf_annotation(
    state: State<'_, DesktopState>,
    annotation_id: String,
    color: Option<String>,
    content: Option<String>,
) -> Result<PdfAnnotationDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.update_pdf_annotation(&annotation_id, color.as_deref(), content.as_deref())
    })
    .await
    .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: deletes one of the local owner's annotations.
#[tauri::command]
pub async fn desktop_delete_pdf_annotation(
    state: State<'_, DesktopState>,
    annotation_id: String,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.delete_pdf_annotation(&annotation_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: lists the local owner's memorization cards for a specific media.
#[tauri::command]
pub async fn desktop_list_flashcards(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<Vec<FlashcardDto>, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_flashcards(&media_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: lists every card of the local owner, for the review hub.
#[tauri::command]
pub async fn desktop_list_all_flashcards(
    state: State<'_, DesktopState>,
) -> Result<Vec<FlashcardDto>, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_all_flashcards())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: lists the recent review history of one local card.
#[tauri::command]
pub async fn desktop_list_flashcard_reviews(
    state: State<'_, DesktopState>,
    card_id: String,
    limit: Option<usize>,
) -> Result<FlashcardReviewsDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_flashcard_reviews(&card_id, limit))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: counts the local owner's cards that are due now.
#[tauri::command]
pub async fn desktop_due_flashcard_count(state: State<'_, DesktopState>) -> Result<i64, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.count_due_flashcards())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: creates a new memorization card for the local owner.
#[tauri::command]
pub async fn desktop_create_flashcard(
    state: State<'_, DesktopState>,
    media_id: String,
    front: String,
    back: String,
    position_seconds: Option<i64>,
) -> Result<FlashcardDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.create_flashcard(&media_id, &front, &back, position_seconds)
    })
    .await
    .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: updates the front and back text of a card that belongs to the local owner.
#[tauri::command]
pub async fn desktop_update_flashcard(
    state: State<'_, DesktopState>,
    card_id: String,
    front: String,
    back: String,
) -> Result<FlashcardDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.update_flashcard(&card_id, &front, &back))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: deletes a memorization card that belongs to the local owner.
#[tauri::command]
pub async fn desktop_delete_flashcard(
    state: State<'_, DesktopState>,
    card_id: String,
) -> Result<(), IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.delete_flashcard(&card_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: records a review ("again" or "good") on a card that belongs to the local owner.
#[tauri::command]
pub async fn desktop_review_flashcard(
    state: State<'_, DesktopState>,
    card_id: String,
    rating: String,
) -> Result<FlashcardDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.review_flashcard(&card_id, &rating))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: lists the courses and lessons favourited by the local owner.
#[tauri::command]
pub async fn desktop_list_favorites(
    state: State<'_, DesktopState>,
) -> Result<StudyFavoritesDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_favorites())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: marks or unmarks a course as a favourite for the local owner.
#[tauri::command]
pub async fn desktop_set_course_favorite(
    state: State<'_, DesktopState>,
    course_id: String,
    favorite: bool,
) -> Result<bool, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.set_course_favorite(&course_id, favorite))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: marks or unmarks a lesson (media) as a favourite for the local owner.
#[tauri::command]
pub async fn desktop_set_lesson_favorite(
    state: State<'_, DesktopState>,
    media_id: String,
    favorite: bool,
) -> Result<bool, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.set_lesson_favorite(&media_id, favorite))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Persists an idempotent Pomodoro session for the local profile.
#[tauri::command]
pub async fn desktop_record_focus_session(
    state: State<'_, DesktopState>,
    session: FocusSessionRequest,
) -> Result<FocusSessionDto, IpcError> {
    let kind = match session.kind.as_str() {
        "focus" => FocusKind::Focus,
        "short" => FocusKind::ShortBreak,
        "long" => FocusKind::LongBreak,
        _ => return Err(IpcError::invalid_focus_session()),
    };
    let input = FocusSessionInput {
        id: session.id,
        kind,
        planned_seconds: session.planned_seconds,
        actual_seconds: session.actual_seconds,
        completed: session.completed,
        started_at: session.started_at,
        ended_at: session.ended_at,
    };
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.record_focus_session(input))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Lists the focus sessions persisted in the local database, from newest to oldest.
#[tauri::command]
pub async fn desktop_list_focus_sessions(
    state: State<'_, DesktopState>,
    limit: Option<usize>,
) -> Result<Vec<FocusSessionDto>, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.list_focus_sessions(limit))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Reads the focus metrics persisted in the local database.
#[tauri::command]
pub async fn desktop_focus_summary(
    state: State<'_, DesktopState>,
) -> Result<FocusSummaryDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.focus_summary())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Reads XP, goals and missions persisted in the local database.
#[tauri::command]
pub async fn desktop_reward_summary(
    state: State<'_, DesktopState>,
) -> Result<RewardSummaryDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.reward_summary())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Redeems a daily mission of the local profile.
#[tauri::command]
pub async fn desktop_claim_daily_mission(
    state: State<'_, DesktopState>,
    mission_id: String,
) -> Result<bool, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.claim_daily_mission(&mission_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Reads the local profile's weekly goal.
#[tauri::command]
pub async fn desktop_weekly_goal(state: State<'_, DesktopState>) -> Result<i64, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.weekly_goal())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Updates the local profile's weekly goal.
#[tauri::command]
pub async fn desktop_set_weekly_goal(
    state: State<'_, DesktopState>,
    weekly_goal: i64,
) -> Result<i64, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.set_weekly_goal(weekly_goal))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: queries the impact of removing a media.
#[tauri::command]
pub async fn desktop_media_removal_impact(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<MediaRemovalImpactDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.media_removal_impact(&media_id))
        .await
        .map_err(|_| IpcError::database_unavailable())?
}

/// IPC command: removes a media from the local catalog (persistent tombstone).
#[tauri::command]
pub async fn desktop_remove_media(
    state: State<'_, DesktopState>,
    media_id: String,
    confirm_progress: bool,
) -> Result<MediaRemovalOutcomeDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.remove_media(&media_id, confirm_progress))
        .await
        .map_err(|_| IpcError::database_unavailable())?
}

/// Opens the native directory picker and returns the chosen path, if any.
///
/// The dialog is internal to Rust (`tauri-plugin-dialog`); it is not exposed to the webview.
/// Cancelling returns `None`, with no path or error.
fn pick_folder(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;

    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|path| path.into_path().ok())
}

/// Opens the native image picker and returns the chosen file, if any.
///
/// The extension filter is only to guide the user: the real decision of the format belongs to the
/// file's initial bytes ([`crate::covers::detect_image_type`]), never to the name. Like the
/// folder picker, the dialog is internal to Rust and is not exposed to the webview.
fn pick_cover(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri_plugin_dialog::DialogExt;

    app.dialog()
        .file()
        .set_title("Escolha a imagem da capa")
        .add_filter("Imagem (PNG, JPEG, WebP)", &["png", "jpg", "jpeg", "webp"])
        .blocking_pick_file()
        .and_then(|path| path.into_path().ok())
}

/// Flatpak exposes its app ID in the environment and a marker at the sandbox root.
pub(crate) fn runs_in_flatpak() -> bool {
    detect_flatpak(
        |name| std::env::var_os(name).is_some(),
        |path| Path::new(path).exists(),
    )
}

fn detect_flatpak(
    env_is_set: impl FnOnce(&str) -> bool,
    path_exists: impl FnOnce(&str) -> bool,
) -> bool {
    env_is_set("FLATPAK_ID") || path_exists("/.flatpak-info")
}

/// The built-in updater only runs outside Flatpak.
pub(crate) fn updater_enabled(in_flatpak: bool) -> bool {
    !in_flatpak
}

/// Queries the availability of a new version through tauri-plugin-updater in a non-blocking, safe way.
///
/// It tells the absence of an update (`available: false`) from connection/network failures (`IpcError::update_check_failed()`),
/// keeping the application fully functional offline without hanging or exposing sensitive data.
#[tauri::command]
pub async fn desktop_check_update(
    app: tauri::AppHandle,
) -> Result<crate::dto::DesktopUpdateInfoDto, IpcError> {
    use tauri_plugin_updater::UpdaterExt;

    let current_version = app.package_info().version.to_string();
    if !updater_enabled(runs_in_flatpak()) {
        return Ok(crate::dto::DesktopUpdateInfoDto {
            available: false,
            current_version,
            version: None,
            date: None,
            body: None,
        });
    }
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(_) => {
            return Ok(crate::dto::DesktopUpdateInfoDto {
                available: false,
                current_version,
                version: None,
                date: None,
                body: None,
            });
        }
    };

    match updater.check().await {
        Ok(Some(update)) => Ok(crate::dto::DesktopUpdateInfoDto {
            available: true,
            current_version,
            version: Some(update.version),
            date: update.date.map(|d| d.to_string()),
            body: update.body,
        }),
        Ok(None) => Ok(crate::dto::DesktopUpdateInfoDto {
            available: false,
            current_version,
            version: None,
            date: None,
            body: None,
        }),
        Err(_) => Err(IpcError::update_check_failed()),
    }
}

/// Downloads the signed update and holds its bytes for [`desktop_restart_to_update`].
///
/// The webview gets no bytes, URL, or progress: `true` means the update is ready to install,
/// `false` means no update is offered. The download never blocks the app and never restarts it.
#[tauri::command]
pub async fn desktop_download_update(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<bool, IpcError> {
    use tauri_plugin_updater::UpdaterExt;

    if !updater_enabled(runs_in_flatpak()) {
        return Err(IpcError::update_unavailable());
    }
    let updater = app.updater().map_err(|_| IpcError::update_unavailable())?;
    let update = match updater.check().await {
        Ok(Some(update)) => update,
        Ok(None) => return Ok(false),
        Err(_) => return Err(IpcError::update_check_failed()),
    };
    let version = update.version.clone();
    let bytes = update
        .download(|_downloaded, _total| {}, || {})
        .await
        .map_err(|_| IpcError::update_unavailable())?;
    state.hold_update(version, bytes)?;
    Ok(true)
}

/// Installs the held update and restarts the application, on an explicit request only.
///
/// The version is checked again before installing: the held bytes are applied only while the
/// same release is still offered, and a missing download is refused instead of restarting into
/// nothing. Failures stay sanitized ([`IpcError`]) and never echo a path or a URL.
#[tauri::command]
pub async fn desktop_restart_to_update(
    app: tauri::AppHandle,
    state: State<'_, DesktopState>,
) -> Result<bool, IpcError> {
    use tauri_plugin_updater::UpdaterExt;

    let Some(pending) = state.take_update()? else {
        return Err(IpcError::update_unavailable());
    };
    let updater = app.updater().map_err(|_| IpcError::update_unavailable())?;
    let update = match updater.check().await {
        Ok(Some(update)) if update.version == pending.version => update,
        Ok(_) => return Err(IpcError::update_unavailable()),
        Err(_) => return Err(IpcError::update_check_failed()),
    };
    update
        .install(&pending.bytes)
        .map_err(|_| IpcError::update_unavailable())?;

    app.restart();
}

/// Queries the native playback capabilities in the desktop environment.
#[tauri::command]
pub async fn desktop_media_capabilities()
-> Result<crate::dto::DesktopMediaCapabilitiesDto, IpcError> {
    Ok(crate::dto::DesktopMediaCapabilitiesDto {
        transcoding: false,
        system_player: !runs_in_flatpak(),
    })
}

/// Opens the media's physical file in the operating system's default player in a decoupled, read-only way.
pub fn open_in_system_player(
    database: &Database,
    courses: &[CourseEntry],
    media_id: &str,
) -> Result<bool, IpcError> {
    if runs_in_flatpak() {
        return Err(IpcError::system_player_unavailable());
    }
    let Some(library_id) = learnvault_core::find_media_library_id(database, media_id)
        .map_err(|_| IpcError::study_unavailable())?
    else {
        return Err(IpcError::media_not_found());
    };
    let Some(course) = courses.iter().find(|c| c.id == library_id) else {
        return Err(IpcError::media_not_found());
    };
    let opened =
        learnvault_core::open_media(database, &library_id, media_id, course.root.as_path())
            .map_err(|_| IpcError::media_not_found())?;

    if opened.kind != learnvault_core::MediaKind::Media {
        return Err(IpcError::media_not_found());
    }

    let canonical_root = course
        .root
        .as_path()
        .canonicalize()
        .map_err(|_| IpcError::media_not_found())?;

    let relative_path: String = database
        .connection()
        .query_row(
            "SELECT relative_path FROM media WHERE media_id = ?1",
            [media_id],
            |row| row.get(0),
        )
        .map_err(|_| IpcError::media_not_found())?;

    let full_path = learnvault_core::paths::resolve_within(&canonical_root, &relative_path)
        .map_err(|_| IpcError::media_not_found())?;

    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("rundll32.exe")
        .args(["url.dll,FileProtocolHandler"])
        .arg(&full_path)
        .spawn();
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(&full_path).spawn();
    #[cfg(target_os = "linux")]
    let spawned = std::process::Command::new("xdg-open")
        .arg(&full_path)
        .spawn();

    Ok(spawned.map(|_| true).unwrap_or(false))
}

/// IPC command to open the media in the operating system's default player.
#[tauri::command]
pub async fn desktop_open_in_system_player(
    state: State<'_, DesktopState>,
    media_id: String,
) -> Result<bool, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.open_in_system_player(&media_id))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// Reads a bounded byte slice from an authorized media or material file offline.
#[tauri::command]
pub async fn desktop_read_media_slice(
    state: State<'_, DesktopState>,
    media_id: String,
    start: u64,
    length: usize,
) -> Result<crate::dto::MediaSliceDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.read_media_slice(&media_id, start, length))
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: reads the local owner's playback preferences.
#[tauri::command]
pub async fn desktop_playback_preferences(
    state: State<'_, DesktopState>,
) -> Result<PlaybackPreferencesDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.playback_preference())
        .await
        .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: writes the playback preferences and queues them for the server.
#[tauri::command]
pub async fn desktop_save_playback_preferences(
    state: State<'_, DesktopState>,
    video_speed: i64,
    audio_speed: i64,
    subtitle_language: String,
) -> Result<PlaybackPreferencesDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        state.save_playback_preference(video_speed, audio_speed, &subtitle_language)
    })
    .await
    .map_err(|_| IpcError::study_unavailable())?
}

/// IPC command: device identity and the cursor already applied.
#[tauri::command]
pub async fn desktop_sync_state(state: State<'_, DesktopState>) -> Result<SyncStateDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.sync_state())
        .await
        .map_err(|_| IpcError::sync_unavailable())?
}

/// IPC command: sets the sync server's URL.
#[tauri::command]
pub async fn desktop_sync_configure(
    state: State<'_, DesktopState>,
    server_url: String,
) -> Result<SyncStateDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.sync_configure(&server_url))
        .await
        .map_err(|_| IpcError::sync_unavailable())?
}

/// IPC command: signs in to the configured server and stores the session locally.
#[tauri::command]
pub async fn desktop_sync_login(
    state: State<'_, DesktopState>,
    username: String,
    password: String,
) -> Result<SyncStateDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.sync_login(&username, &password))
        .await
        .map_err(|_| IpcError::sync_unavailable())?
}

/// IPC command: ends the sync session.
#[tauri::command]
pub async fn desktop_sync_logout(state: State<'_, DesktopState>) -> Result<SyncStateDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.sync_logout())
        .await
        .map_err(|_| IpcError::sync_unavailable())?
}

/// IPC command: runs a sync and returns the summary, even when it fails.
#[tauri::command]
pub async fn desktop_sync_now(state: State<'_, DesktopState>) -> Result<SyncResultDto, IpcError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.sync_now())
        .await
        .map_err(|_| IpcError::sync_unavailable())?
}

#[cfg(test)]
mod flatpak_gate_tests {
    use super::{detect_flatpak, updater_enabled};

    #[test]
    fn detects_flatpak_from_either_runtime_marker() {
        assert!(detect_flatpak(|name| name == "FLATPAK_ID", |_| false));
        assert!(detect_flatpak(|_| false, |path| path == "/.flatpak-info"));
        assert!(!detect_flatpak(|_| false, |_| false));
    }

    #[test]
    fn the_updater_is_off_inside_flatpak_and_on_elsewhere() {
        assert!(!updater_enabled(true));
        assert!(updater_enabled(false));
    }
}
