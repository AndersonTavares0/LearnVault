//! Desktop study progress over the IPC.
//!
//! This adapter reimplements no rule: each operation delegates to
//! [`learnvault_core::study`] and only fixes the owner. The desktop's `owner_id` is a fixed
//! local identifier ([`LOCAL_OWNER_ID`]) resolved by Rust; it is never received
//! from the webview, so the command contract neither offers — nor accepts — an `owner_id` as an
//! argument, and the webview can neither read nor write another owner's progress.
//!
//! Like the HTTP adapter, the core's error is translated into a stable code and the
//! domain's `Display` is not echoed: the media identifier queried never appears in the
//! response sent to the webview.
//!
//! Known limitation of this slice: the desktop serves a single local owner. Several local
//! owners and the selection among them belong to a later slice — the core already isolates by
//! owner, but here there is neither a session nor a local selector.

use learnvault_core::{Database, FocusSessionInput, ReviewRating};

use crate::dto::{
    FlashcardDto, FlashcardReviewDto, FlashcardReviewsDto, FocusSessionDto, FocusSummaryDto,
    MediaBookmarkDto, NewPdfAnnotationDto, PdfAnnotationDto, PlaybackPreferencesDto,
    RecentStudyDto, RewardSummaryDto, StudyFavoritesDto, StudyNoteDto, StudyProgressDto,
    StudyStateDto, StudySummaryDto,
};
use crate::error::IpcError;

/// Fixed local owner of the desktop's study progress.
///
/// The core treats `owner_id` as opaque, canonical and global: the desktop picks a stable
/// value, instead of deriving it from something that may vary between runs, so that
/// progress survives restarts. The value is not a server user identifier
/// and is not accepted from outside Rust.
pub const LOCAL_OWNER_ID: &str = "desktop-local";

/// Default limit of the recent history when the webview gives none.
///
/// It follows the HTTP adapter's default; the real ceiling remains
/// [`learnvault_core::MAX_RECENT_STUDY`], applied by the core, and not a setting.
pub const DEFAULT_RECENT_LIMIT: usize = 20;

/// Default limit of the review history when the webview gives none.
///
/// The real ceiling remains [`learnvault_core::MAX_FLASHCARD_REVIEWS`], applied by the core.
pub const DEFAULT_REVIEW_LIMIT: usize = 20;

/// Reads the local owner's progress on a known media.
///
/// No progress is not an error: a known media with no state returns the neutral
/// record with `recorded: false`. A media unknown to the catalog becomes
/// [`IpcError::media_not_found`].
pub fn progress(database: &Database, media_id: &str) -> Result<StudyProgressDto, IpcError> {
    match learnvault_core::study_progress(database, LOCAL_OWNER_ID, media_id) {
        Ok(Some(found)) => Ok(StudyProgressDto::from(found)),
        Ok(None) => Ok(StudyProgressDto::absent(media_id)),
        Err(error) => Err(study_error(error)),
    }
}

/// Records the local owner's resume position, preserving the recorded completion.
pub fn record_position(
    database: &Database,
    media_id: &str,
    position_seconds: i64,
) -> Result<StudyProgressDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_position(
        database,
        LOCAL_OWNER_ID,
        &device,
        media_id,
        position_seconds,
    )
    .map(StudyProgressDto::from)
    .map_err(study_error)
}

/// Marks or unmarks the local owner's completion, preserving the resume position.
pub fn set_completed(
    database: &Database,
    media_id: &str,
    completed: bool,
) -> Result<StudyProgressDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_completed(database, LOCAL_OWNER_ID, &device, media_id, completed)
        .map(StudyProgressDto::from)
        .map_err(study_error)
}

/// Lists the local owner's progress, from most recently changed to oldest.
///
/// With no limit given, it applies [`DEFAULT_RECENT_LIMIT`]; the core's ceiling still holds
/// for any value received.
pub fn recent(database: &Database, limit: Option<usize>) -> Result<RecentStudyDto, IpcError> {
    learnvault_core::recent_study(
        database,
        LOCAL_OWNER_ID,
        limit.unwrap_or(DEFAULT_RECENT_LIMIT),
    )
    .map(|states| RecentStudyDto {
        items: states.into_iter().map(StudyProgressDto::from).collect(),
    })
    .map_err(study_error)
}

/// Pages through the local owner's whole progress, newest first.
///
/// The cursor is the opaque `next_cursor` of the previous page; the core clamps the limit.
pub fn state(
    database: &Database,
    limit: Option<usize>,
    cursor: Option<&str>,
) -> Result<StudyStateDto, IpcError> {
    let after = match cursor.map(learnvault_core::StudyCursor::parse) {
        None => None,
        Some(Some(cursor)) => Some(cursor),
        Some(None) => return Err(IpcError::study_unavailable()),
    };
    learnvault_core::study_page(
        database,
        LOCAL_OWNER_ID,
        None,
        after.as_ref(),
        limit.unwrap_or(learnvault_core::MAX_STUDY_PAGE),
    )
    .map(|page| StudyStateDto {
        items: page.items.into_iter().map(StudyProgressDto::from).collect(),
        next_cursor: page.next.map(|cursor| cursor.encode()),
    })
    .map_err(study_error)
}

/// Summary of the local owner's streak and weekly goal.
pub fn summary(database: &Database) -> Result<StudySummaryDto, IpcError> {
    learnvault_core::study_summary(database, LOCAL_OWNER_ID)
        .map(StudySummaryDto::from)
        .map_err(study_error)
}

/// Writes a timer session idempotently for the local owner.
pub fn record_focus_session(
    database: &Database,
    input: FocusSessionInput,
) -> Result<FocusSessionDto, IpcError> {
    learnvault_core::record_focus_session(database, LOCAL_OWNER_ID, input)
        .map(FocusSessionDto::from)
        .map_err(focus_error)
}

/// Lists the local owner's persisted focus sessions, from newest to oldest.
pub fn list_focus_sessions(
    database: &Database,
    limit: Option<usize>,
) -> Result<Vec<FocusSessionDto>, IpcError> {
    learnvault_core::list_focus_sessions(database, LOCAL_OWNER_ID, limit.unwrap_or(100))
        .map(|sessions| sessions.into_iter().map(FocusSessionDto::from).collect())
        .map_err(focus_error)
}

/// Reads the local owner's persisted timer statistics.
pub fn focus_summary(database: &Database) -> Result<FocusSummaryDto, IpcError> {
    learnvault_core::focus_summary(database, LOCAL_OWNER_ID)
        .map(FocusSummaryDto::from)
        .map_err(focus_error)
}

/// Reads the local owner's persisted XP and mission progress.
pub fn reward_summary(database: &Database) -> Result<RewardSummaryDto, IpcError> {
    learnvault_core::reward_summary(database, LOCAL_OWNER_ID)
        .map(RewardSummaryDto::from)
        .map_err(focus_error)
}

/// Redeems a daily mission for the local owner.
pub fn claim_daily_mission(database: &Database, mission_id: &str) -> Result<bool, IpcError> {
    learnvault_core::claim_daily_mission_today(database, LOCAL_OWNER_ID, mission_id)
        .map_err(focus_error)
}

/// Reads the local owner's persisted weekly goal.
pub fn weekly_goal(database: &Database) -> Result<i64, IpcError> {
    learnvault_core::weekly_goal(database, LOCAL_OWNER_ID).map_err(focus_error)
}

/// Updates the local owner's persisted weekly goal.
pub fn set_weekly_goal(database: &Database, goal: i64) -> Result<i64, IpcError> {
    learnvault_core::set_weekly_goal(database, LOCAL_OWNER_ID, goal).map_err(focus_error)
}

/// Lists the local owner's notes. When `media_id` is given, it filters by the media;
/// otherwise, it returns all of the user's notes with relative paths for global search.
pub fn list_notes(
    database: &Database,
    media_id: Option<&str>,
) -> Result<Vec<StudyNoteDto>, IpcError> {
    if let Some(id) = media_id.filter(|s| !s.trim().is_empty()) {
        learnvault_core::list_study_notes(database, LOCAL_OWNER_ID, id)
            .map(|notes| notes.into_iter().map(StudyNoteDto::from).collect())
            .map_err(study_error)
    } else {
        learnvault_core::list_all_study_notes(database, LOCAL_OWNER_ID)
            .map(|notes| notes.into_iter().map(StudyNoteDto::from).collect())
            .map_err(study_error)
    }
}

/// Creates a study note linked to a media and timestamp for the local owner.
pub fn create_note(
    database: &Database,
    media_id: &str,
    position_seconds: i64,
    content: &str,
) -> Result<StudyNoteDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_note_create(
        database,
        LOCAL_OWNER_ID,
        &device,
        media_id,
        position_seconds,
        content,
    )
    .map(StudyNoteDto::from)
    .map_err(study_error)
}

/// Updates the text of a note that belongs to the local owner.
pub fn update_note(
    database: &Database,
    note_id: &str,
    content: &str,
) -> Result<StudyNoteDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_note_update(database, LOCAL_OWNER_ID, &device, note_id, content)
        .map(StudyNoteDto::from)
        .map_err(study_error)
}

/// Deletes a study note of the local owner. Returns an error if the note does not exist.
///
/// The deletion enters the outbox in the same transaction as the local delete, so another
/// device stops showing the note instead of pulling it back.
pub fn delete_note(database: &Database, note_id: &str) -> Result<(), IpcError> {
    let device = crate::sync::device(database)?;
    let deleted = learnvault_core::queue_note_delete(database, LOCAL_OWNER_ID, &device, note_id)
        .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::note_not_found())
    }
}

/// Lists the local owner's annotations on one PDF.
pub fn list_pdf_annotations(
    database: &Database,
    media_id: &str,
) -> Result<Vec<PdfAnnotationDto>, IpcError> {
    learnvault_core::list_pdf_annotations(database, LOCAL_OWNER_ID, media_id)
        .map(|items| items.into_iter().map(PdfAnnotationDto::from).collect())
        .map_err(study_error)
}

/// Creates an annotation. An unknown kind or color is refused like any other bad input.
pub fn create_pdf_annotation(
    database: &Database,
    media_id: &str,
    input: NewPdfAnnotationDto,
) -> Result<PdfAnnotationDto, IpcError> {
    let (Some(kind), Some(color)) = (
        learnvault_core::PdfAnnotationKind::parse(&input.kind),
        learnvault_core::PdfAnnotationColor::parse(&input.color),
    ) else {
        return Err(IpcError::invalid_pdf_annotation());
    };
    learnvault_core::create_pdf_annotation(
        database,
        LOCAL_OWNER_ID,
        media_id,
        learnvault_core::NewPdfAnnotation {
            page: input.page,
            kind,
            color,
            rects: input
                .rects
                .into_iter()
                .map(|rect| learnvault_core::PdfRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                })
                .collect(),
            quote: input.quote,
            content: input.content,
        },
    )
    .map(PdfAnnotationDto::from)
    .map_err(study_error)
}

/// Changes an annotation's color and/or text.
pub fn update_pdf_annotation(
    database: &Database,
    annotation_id: &str,
    color: Option<&str>,
    content: Option<&str>,
) -> Result<PdfAnnotationDto, IpcError> {
    let color = match color {
        Some(value) => Some(
            learnvault_core::PdfAnnotationColor::parse(value)
                .ok_or_else(IpcError::invalid_pdf_annotation)?,
        ),
        None => None,
    };
    learnvault_core::update_pdf_annotation(database, LOCAL_OWNER_ID, annotation_id, color, content)
        .map(PdfAnnotationDto::from)
        .map_err(study_error)
}

/// Deletes an annotation; an unknown id is an error, like for notes.
pub fn delete_pdf_annotation(database: &Database, annotation_id: &str) -> Result<(), IpcError> {
    let deleted = learnvault_core::delete_pdf_annotation(database, LOCAL_OWNER_ID, annotation_id)
        .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::pdf_annotation_not_found())
    }
}

/// Lists the local owner's memorization cards for a specific media.
pub fn list_flashcards(database: &Database, media_id: &str) -> Result<Vec<FlashcardDto>, IpcError> {
    learnvault_core::list_flashcards(database, LOCAL_OWNER_ID, media_id)
        .map(|cards| cards.into_iter().map(FlashcardDto::from).collect())
        .map_err(study_error)
}

/// Lists every card of the local owner, soonest due first, for the review hub.
pub fn list_all_flashcards(database: &Database) -> Result<Vec<FlashcardDto>, IpcError> {
    learnvault_core::list_all_flashcards(database, LOCAL_OWNER_ID)
        .map(|cards| cards.into_iter().map(FlashcardDto::from).collect())
        .map_err(study_error)
}

/// Creates a new memorization card for the local owner.
pub fn create_flashcard(
    database: &Database,
    media_id: &str,
    front: &str,
    back: &str,
    position_seconds: Option<i64>,
) -> Result<FlashcardDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_flashcard_create(
        database,
        LOCAL_OWNER_ID,
        &device,
        media_id,
        front,
        back,
        position_seconds,
    )
    .map(FlashcardDto::from)
    .map_err(study_error)
}

/// Updates the front and back of a memorization card of the local owner.
pub fn update_flashcard(
    database: &Database,
    card_id: &str,
    front: &str,
    back: &str,
) -> Result<FlashcardDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_flashcard_update(database, LOCAL_OWNER_ID, &device, card_id, front, back)
        .map(FlashcardDto::from)
        .map_err(study_error)
}

/// Deletes a memorization card that belongs to the local owner.
///
/// The deletion enters the outbox in the same transaction as the local delete, so another
/// device stops showing the card instead of pulling it back.
pub fn delete_flashcard(database: &Database, card_id: &str) -> Result<(), IpcError> {
    let device = crate::sync::device(database)?;
    let deleted =
        learnvault_core::queue_flashcard_delete(database, LOCAL_OWNER_ID, &device, card_id)
            .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::flashcard_not_found())
    }
}

/// Lists the recent history of a card of the local owner, from the newest review to the
/// oldest, together with the full count.
pub fn list_flashcard_reviews(
    database: &Database,
    card_id: &str,
    limit: Option<usize>,
) -> Result<FlashcardReviewsDto, IpcError> {
    let limit = limit.unwrap_or(DEFAULT_REVIEW_LIMIT);
    let items = learnvault_core::list_flashcard_reviews(database, LOCAL_OWNER_ID, card_id, limit)
        .map_err(study_error)?;
    let total = learnvault_core::count_flashcard_reviews(database, LOCAL_OWNER_ID, card_id)
        .map_err(study_error)?;
    Ok(FlashcardReviewsDto {
        items: items.into_iter().map(FlashcardReviewDto::from).collect(),
        total,
    })
}

/// Counts the local owner's cards that are due now.
pub fn count_due_flashcards(database: &Database) -> Result<i64, IpcError> {
    learnvault_core::count_due_flashcards(database, LOCAL_OWNER_ID).map_err(study_error)
}

/// Records a review ("again" or "good") on a memorization card of the local owner.
pub fn review_flashcard(
    database: &Database,
    card_id: &str,
    rating: &str,
) -> Result<FlashcardDto, IpcError> {
    let Some(parsed) = ReviewRating::parse(rating) else {
        return Err(IpcError::invalid_review_rating());
    };
    let device = crate::sync::device(database)?;
    learnvault_core::queue_review(database, LOCAL_OWNER_ID, &device, card_id, parsed)
        .map(FlashcardDto::from)
        .map_err(study_error)
}

/// Lists the courses and lessons favourited by the local owner.
pub fn list_favorites(database: &Database) -> Result<StudyFavoritesDto, IpcError> {
    learnvault_core::list_favorites(database, LOCAL_OWNER_ID)
        .map(StudyFavoritesDto::from)
        .map_err(study_error)
}

/// Marks or unmarks a course as a favourite for the local owner.
pub fn set_course_favorite(
    database: &Database,
    course_id: &str,
    favorite: bool,
) -> Result<bool, IpcError> {
    learnvault_core::set_course_favorite(database, LOCAL_OWNER_ID, course_id, favorite)
        .map_err(study_error)
}

/// Marks or unmarks a lesson (media) as a favourite for the local owner.
pub fn set_lesson_favorite(
    database: &Database,
    media_id: &str,
    favorite: bool,
) -> Result<bool, IpcError> {
    learnvault_core::set_lesson_favorite(database, LOCAL_OWNER_ID, media_id, favorite)
        .map_err(study_error)
}

/// Lists the local owner's timestamp bookmarks for one lesson.
pub fn list_bookmarks(
    database: &Database,
    media_id: &str,
) -> Result<Vec<MediaBookmarkDto>, IpcError> {
    learnvault_core::list_media_bookmarks(database, LOCAL_OWNER_ID, media_id)
        .map(|items| items.into_iter().map(MediaBookmarkDto::from).collect())
        .map_err(study_error)
}

/// Creates a timestamp bookmark for the local owner.
pub fn create_bookmark(
    database: &Database,
    media_id: &str,
    position_seconds: i64,
    label: &str,
) -> Result<MediaBookmarkDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_bookmark_create(
        database,
        LOCAL_OWNER_ID,
        &device,
        media_id,
        position_seconds,
        label,
    )
    .map(MediaBookmarkDto::from)
    .map_err(study_error)
}

/// Deletes one of the local owner's timestamp bookmarks.
///
/// The deletion enters the outbox in the same transaction as the local delete, so another
/// device stops showing the bookmark instead of pulling it back.
pub fn delete_bookmark(database: &Database, bookmark_id: &str) -> Result<(), IpcError> {
    let device = crate::sync::device(database)?;
    let deleted =
        learnvault_core::queue_bookmark_delete(database, LOCAL_OWNER_ID, &device, bookmark_id)
            .map_err(study_error)?;
    if deleted {
        Ok(())
    } else {
        Err(IpcError::bookmark_not_found())
    }
}

/// Reads the local owner's playback preferences, falling back to the neutral defaults.
///
/// Preferences are part of the offline queue: every save is applied locally and enqueued
/// with its own `op_id`, so two devices converge by last write.
pub fn playback_preference(database: &Database) -> Result<PlaybackPreferencesDto, IpcError> {
    learnvault_core::player_preference(database, LOCAL_OWNER_ID)
        .map(PlaybackPreferencesDto::from)
        .map_err(study_error)
}

/// Saves every playback field for the local owner and enqueues the change.
pub fn save_playback_preference(
    database: &Database,
    video_speed: i64,
    audio_speed: i64,
    subtitle_language: &str,
) -> Result<PlaybackPreferencesDto, IpcError> {
    let device = crate::sync::device(database)?;
    learnvault_core::queue_preference(
        database,
        LOCAL_OWNER_ID,
        &device,
        video_speed,
        audio_speed,
        subtitle_language,
    )
    .map(PlaybackPreferencesDto::from)
    .map_err(study_error)
}

/// Translates the core's error into the stable IPC error, without echoing the identifier queried.
///
/// An unknown media and a negative position are input failures with a code of their own,
/// as in the HTTP adapter. Any other error is operational and cannot become "unknown
/// media", so as not to confuse legitimate absence with an unavailable database.
fn study_error(error: learnvault_core::Error) -> IpcError {
    use learnvault_core::Error;

    match error {
        Error::MediaNotFound { .. } => IpcError::media_not_found(),
        Error::InvalidStudyPosition { .. } => IpcError::invalid_position(),
        Error::StudyNoteNotFound { .. } => IpcError::note_not_found(),
        Error::InvalidStudyNoteContent => IpcError::invalid_note(),
        Error::PdfAnnotationNotFound { .. } => IpcError::pdf_annotation_not_found(),
        Error::InvalidPdfAnnotation => IpcError::invalid_pdf_annotation(),
        Error::FlashcardNotFound { .. } => IpcError::flashcard_not_found(),
        Error::InvalidFlashcardContent => IpcError::invalid_flashcard(),
        Error::InvalidFavoriteTarget => IpcError::invalid_favorite_target(),
        Error::BookmarkNotFound { .. } => IpcError::bookmark_not_found(),
        Error::InvalidBookmarkLabel => IpcError::invalid_bookmark_label(),
        Error::InvalidPlayerPreference => IpcError::invalid_player_preference(),
        _ => IpcError::study_unavailable(),
    }
}

fn focus_error(error: learnvault_core::Error) -> IpcError {
    use learnvault_core::Error;

    match error {
        Error::InvalidFocusSession { .. } => IpcError::invalid_focus_session(),
        Error::FocusSessionConflict => IpcError::focus_session_conflict(),
        Error::MissionNotComplete => IpcError::mission_not_complete(),
        Error::InvalidWeeklyGoal { .. } => IpcError::invalid_weekly_goal(),
        _ => IpcError::study_unavailable(),
    }
}
