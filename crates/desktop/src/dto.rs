//! Serializable DTOs of the desktop's catalog and study progress.
//!
//! The core's types are not serialized directly: each enum becomes a stable string
//! and the snake_case shape (`loose_files`, `media_id`, ...) is identical to the HTTP adapter's,
//! so the SPA's `normalizeCatalog` and `normalizeStudyProgress` accept the desktop's
//! payload unchanged.

use learnvault_core::{
    Availability, Catalog, CatalogFile, CatalogNode, FocusKind, FocusSession, FocusSummary,
    MediaBookmark, MediaKind, NodeRole, PlayerPreference, RescanSummary, RewardSummary,
    StudyFavorites, StudyProgress,
};
use serde::{Deserialize, Serialize};

/// Desktop catalog derived from the local database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogDto {
    /// True when the last scan covered all the courses.
    pub complete: bool,
    /// Loose files of a global library. The desktop aggregates courses and keeps this
    /// list empty: loose files belong to the corresponding course node.
    pub loose_files: Vec<CatalogFileDto>,
    /// Canonical recursive tree, the contract preferred by the SPA.
    pub nodes: Vec<CatalogNodeDto>,
    /// Courses, in import order. Each top-level node has the `course` role.
    pub categories: Vec<CatalogNodeDto>,
}

/// Node of the catalog tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogNodeDto {
    /// Displayed title, without numeric prefixes or ordering separators.
    pub title: String,
    /// Full relative path that identifies the node. Empty on the course's top-level node.
    pub path: String,
    /// Role of the node: `category`, `course`, `module` or `lesson`.
    pub role: &'static str,
    /// Identity of the course that contains the node, present only on the top-level node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub course_id: Option<String>,
    /// Modification seconds of the course's custom cover, when it exists. It is the version
    /// the SPA uses to invalidate the URL (`?v=`); absent when there is no cover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_version: Option<i64>,
    /// Name of the course's source folder on disk (parent directory), without exposing absolute paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_folder: Option<String>,
    /// Files of the lesson; empty outside `lesson` nodes.
    pub files: Vec<CatalogFileDto>,
    /// Files directly on the node, not belonging to a lesson.
    pub loose_files: Vec<CatalogFileDto>,
    /// Child nodes, ordered by path.
    pub children: Vec<CatalogNodeDto>,
}

/// File of the catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CatalogFileDto {
    /// Stable identity of the file, independent of its location.
    pub media_id: String,
    /// Name of the file, without directories.
    pub name: String,
    /// Full relative path, used as a structural reference.
    pub path: String,
    /// Size in bytes at the last observation.
    pub size: u64,
    /// Modification time of the media in seconds since the Unix epoch at the last observation.
    pub mtime_seconds: i64,
    /// Kind of the file: `media`, `subtitle` or `material`.
    pub kind: &'static str,
    /// Availability in the last scan: `available` or `missing`.
    pub availability: &'static str,
    /// True when the file is under an unverified area.
    pub unverified: bool,
}

impl From<CatalogNode> for CatalogNodeDto {
    fn from(node: CatalogNode) -> Self {
        Self {
            title: node.title,
            path: node.path,
            role: node_role(node.role),
            course_id: None,
            cover_version: None,
            source_folder: None,
            files: node.files.into_iter().map(CatalogFileDto::from).collect(),
            loose_files: node
                .loose_files
                .into_iter()
                .map(CatalogFileDto::from)
                .collect(),
            children: node
                .children
                .into_iter()
                .map(CatalogNodeDto::from)
                .collect(),
        }
    }
}

impl CatalogNodeDto {
    /// Top-level node of a registered course: opaque identity, folder title, the custom
    /// cover's version (when it exists) and the subtree derived from the persisted data.
    pub fn course(
        course_id: &str,
        title: &str,
        catalog: Catalog,
        cover_version: Option<i64>,
        source_folder: Option<String>,
    ) -> Self {
        Self {
            title: title.to_owned(),
            path: String::new(),
            role: "course",
            course_id: Some(course_id.to_owned()),
            cover_version,
            source_folder,
            files: Vec::new(),
            loose_files: catalog
                .loose_files
                .into_iter()
                .map(CatalogFileDto::from)
                .collect(),
            children: catalog
                .categories
                .into_iter()
                .map(CatalogNodeDto::from)
                .collect(),
        }
    }
}

impl From<CatalogFile> for CatalogFileDto {
    fn from(file: CatalogFile) -> Self {
        Self {
            media_id: file.media_id,
            name: file.name,
            path: file.path,
            size: file.size,
            mtime_seconds: file.mtime_seconds,
            kind: media_kind(file.kind),
            availability: availability(file.availability),
            unverified: file.unverified,
        }
    }
}

/// Translates a node's role into the contract's stable string.
fn node_role(role: NodeRole) -> &'static str {
    match role {
        NodeRole::Category => "category",
        NodeRole::Course => "course",
        NodeRole::Module => "module",
        NodeRole::Lesson => "lesson",
    }
}

/// Translates a file's kind into the contract's stable string.
fn media_kind(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Media => "media",
        MediaKind::Subtitle => "subtitle",
        MediaKind::Material => "material",
    }
}

/// Translates a file's availability into the contract's stable string.
fn availability(value: Availability) -> &'static str {
    match value {
        Availability::Available => "available",
        Availability::Missing => "missing",
    }
}

/// Summary of an explicit rescan of the courses, with no file-system path.
///
/// It is the aggregated serializable projection of [`learnvault_core::RescanSummary`]: only the
/// contract's counts, without the history's internal identifier. No field carries
/// a location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RescanSummaryDto {
    /// True when all the scans covered the courses entirely.
    pub complete: bool,
    /// Number of courses considered in the aggregation.
    pub courses: usize,
    /// Number of courses whose folder could not be scanned.
    pub failed: usize,
    /// New files inserted into the catalog.
    pub inserted: usize,
    /// Already-known records that the scan reaffirmed.
    pub updated: usize,
    /// Records marked as missing.
    pub marked_missing: usize,
    /// Records preserved because they are in an unverified area.
    pub preserved: usize,
    /// Number of ambiguous relative keys refused in this scan.
    ///
    /// It is only the count: the keys themselves are relative paths and never cross the IPC.
    pub ambiguous_count: usize,
}

impl From<RescanSummary> for RescanSummaryDto {
    fn from(summary: RescanSummary) -> Self {
        Self {
            complete: summary.complete,
            courses: 1,
            failed: 0,
            inserted: summary.inserted,
            updated: summary.updated,
            marked_missing: summary.marked_missing,
            preserved: summary.preserved,
            ambiguous_count: summary.ambiguous.len(),
        }
    }
}

/// Study progress of the desktop's local owner on a media.
///
/// The shape is the same snake_case contract as the HTTP adapter's, so the SPA's
/// `normalizeStudyProgress` accepts the desktop's payload unchanged. `recorded`
/// tells "never studied" from "studied at position zero": the absence of progress is not
/// a domain error, and the field keeps the interface from confusing the two situations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StudyProgressDto {
    /// Stable identity of the media, never the file's path.
    pub media_id: String,
    /// Resume point in seconds.
    pub position_seconds: i64,
    /// Tells whether the lesson was completed.
    pub completed: bool,
    /// Instant of the last real change, or `None` when nothing was written.
    pub updated_at: Option<i64>,
    /// True when there is persisted state for the media.
    pub recorded: bool,
}

impl From<StudyProgress> for StudyProgressDto {
    fn from(progress: StudyProgress) -> Self {
        Self {
            media_id: progress.media_id,
            position_seconds: progress.position_seconds,
            completed: progress.completed,
            updated_at: Some(progress.updated_at),
            recorded: true,
        }
    }
}

impl StudyProgressDto {
    /// Neutral state of a known media with no progress: nothing has been written yet.
    pub fn absent(media_id: &str) -> Self {
        Self {
            media_id: media_id.to_owned(),
            position_seconds: 0,
            completed: false,
            updated_at: None,
            recorded: false,
        }
    }
}

/// Recent study history of the desktop's local owner.
///
/// It is the serializable projection of the same `{ items: [...] }` contract as the HTTP adapter's, from
/// newest to oldest. Only media present in the local catalog appear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentStudyDto {
    /// Progress items, already ordered by the core.
    pub items: Vec<StudyProgressDto>,
}

/// Playback preferences of the local owner.
///
/// Speeds use the same contract as the HTTP adapter (percentages of normal speed), so
/// `normalizePlaybackPreferences` in the SPA accepts both payloads unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlaybackPreferencesDto {
    /// Video speed as a percentage of normal speed.
    pub video_speed: i64,
    /// Audio speed as a percentage of normal speed.
    pub audio_speed: i64,
    /// Last chosen subtitle language; empty means no preference.
    pub subtitle_language: String,
    /// Last write in Unix seconds; zero means the defaults were never saved.
    pub updated_at: i64,
}

impl From<PlayerPreference> for PlaybackPreferencesDto {
    fn from(preference: PlayerPreference) -> Self {
        Self {
            video_speed: preference.video_speed,
            audio_speed: preference.audio_speed,
            subtitle_language: preference.subtitle_language,
            updated_at: preference.updated_at,
        }
    }
}

/// Outcome of one sync run shown to the interface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncResultDto {
    /// Operations accepted by the server in this run.
    pub pushed: usize,
    /// Operations applied locally in this run.
    pub pulled: usize,
    /// Operations still waiting in the local outbox.
    pub pending: i64,
    /// Stable failure code, or `None` when the run completed.
    pub error: Option<String>,
}

/// Sync configuration, queue and last result shown to the interface.
///
/// The session cookie is deliberately absent: it never crosses the IPC boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncStateDto {
    /// Stable identity of this device.
    pub device_id: String,
    /// Last server cursor applied locally.
    pub pull_cursor: i64,
    /// Configured server base URL; empty when not configured.
    pub server_url: String,
    /// Whether a session is stored locally.
    pub signed_in: bool,
    /// Operations still waiting in the local outbox.
    pub pending: i64,
    /// Stable failure code of the last automatic or manual run.
    pub last_error: Option<String>,
    /// Summary of the last run, when one already happened.
    pub last_result: Option<SyncResultDto>,
}

/// A rectangle of a PDF annotation, as fractions of the page.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PdfRectDto {
    /// Left edge, 0 to 1.
    pub x: f64,
    /// Top edge, 0 to 1.
    pub y: f64,
    /// Width, up to 1.
    pub width: f64,
    /// Height, up to 1.
    pub height: f64,
}

/// A reader's mark on a PDF page, for the local owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfAnnotationDto {
    /// Stable identity of the annotation.
    pub annotation_id: String,
    /// Identity of the PDF the mark belongs to.
    pub media_id: String,
    /// 1-based page.
    pub page: i64,
    /// `highlight`, `underline` or `note`.
    pub kind: String,
    /// `yellow`, `green`, `blue` or `pink`.
    pub color: String,
    /// Where the mark sits on the page.
    pub rects: Vec<PdfRectDto>,
    /// The marked text.
    pub quote: String,
    /// The reader's own words.
    pub content: String,
    /// Creation time, Unix seconds.
    pub created_at: i64,
    /// Last edit time, Unix seconds.
    pub updated_at: i64,
}

impl From<learnvault_core::PdfAnnotation> for PdfAnnotationDto {
    fn from(annotation: learnvault_core::PdfAnnotation) -> Self {
        Self {
            annotation_id: annotation.annotation_id,
            media_id: annotation.media_id,
            page: annotation.page,
            kind: annotation.kind.as_str().to_owned(),
            color: annotation.color.as_str().to_owned(),
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

/// What the frontend sends to create a PDF annotation.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct NewPdfAnnotationDto {
    /// 1-based page.
    pub page: i64,
    /// `highlight`, `underline` or `note`.
    pub kind: String,
    /// `yellow`, `green`, `blue` or `pink`.
    pub color: String,
    /// Where the mark sits on the page.
    pub rects: Vec<PdfRectDto>,
    /// The marked text.
    #[serde(default)]
    pub quote: String,
    /// The reader's own words.
    #[serde(default)]
    pub content: String,
}

/// Study note associated with a lesson/media for the local owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyNoteDto {
    /// Stable identity of the note.
    pub note_id: String,
    /// Identity of the media the note belongs to.
    pub media_id: String,
    /// Relative path of the media, present only in the global listing of notes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
    /// Point of the media in seconds where the note was recorded.
    pub position_seconds: i64,
    /// Text of the note.
    pub content: String,
    /// Creation date in Unix seconds.
    pub created_at: i64,
    /// Date of the last change in Unix seconds.
    pub updated_at: i64,
}

impl From<learnvault_core::StudyNote> for StudyNoteDto {
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

impl From<learnvault_core::GlobalStudyNote> for StudyNoteDto {
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

/// Time marker of a lesson for the local owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaBookmarkDto {
    /// Stable identity of the marker.
    pub bookmark_id: String,
    /// Identity of the media the marker belongs to.
    pub media_id: String,
    /// Instant of the media in seconds.
    pub position_seconds: i64,
    /// Short label written by the learner.
    pub label: String,
    /// Creation date in Unix seconds.
    pub created_at: i64,
}

impl From<MediaBookmark> for MediaBookmarkDto {
    fn from(bookmark: MediaBookmark) -> Self {
        Self {
            bookmark_id: bookmark.bookmark_id,
            media_id: bookmark.media_id,
            position_seconds: bookmark.position_seconds,
            label: bookmark.label,
            created_at: bookmark.created_at,
        }
    }
}

/// Memorization card (flashcard) associated with a lesson/media for the local owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlashcardDto {
    /// Stable identity of the card.
    pub card_id: String,
    /// Identity of the media the card belongs to.
    pub media_id: String,
    /// Instant of the lesson at which the card was created, when given.
    pub position_seconds: Option<i64>,
    /// Question or front of the card.
    pub front: String,
    /// Answer or back of the card.
    pub back: String,
    /// Current repetition interval in days (0 to 365).
    pub interval_days: i64,
    /// Instant of the next review in Unix seconds.
    pub next_due: i64,
    /// Creation date in Unix seconds.
    pub created_at: i64,
    /// Date of the last change in Unix seconds.
    pub updated_at: i64,
}

impl From<learnvault_core::Flashcard> for FlashcardDto {
    fn from(card: learnvault_core::Flashcard) -> Self {
        Self {
            card_id: card.card_id,
            media_id: card.media_id,
            position_seconds: card.position_seconds,
            front: card.front,
            back: card.back,
            interval_days: card.interval_days,
            next_due: card.next_due,
            created_at: card.created_at,
            updated_at: card.updated_at,
        }
    }
}

/// A review recorded in the history of a card of the local owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlashcardReviewDto {
    /// Sequential identifier of the review.
    pub review_id: i64,
    /// Card reviewed.
    pub card_id: String,
    /// Rating given by the learner, `again` or `good`.
    pub rating: &'static str,
    /// Interval in days before this review.
    pub previous_interval_days: i64,
    /// Next due date calculated by this review.
    pub next_due: i64,
    /// Instant of the review in Unix seconds.
    pub reviewed_at: i64,
}

impl From<learnvault_core::FlashcardReview> for FlashcardReviewDto {
    fn from(review: learnvault_core::FlashcardReview) -> Self {
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

/// Recent history of a card with the full count of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FlashcardReviewsDto {
    /// Reviews from newest to oldest, up to the requested limit.
    pub items: Vec<FlashcardReviewDto>,
    /// Total reviews recorded, even beyond the limit.
    pub total: i64,
}

/// A day of the current week in the study streak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StudyDayDto {
    /// Data local no formato `YYYY-MM-DD`.
    pub day: String,
    /// Indicates there was real progress on that day.
    pub active: bool,
}

/// Summary of the streak and weekly goal of the desktop's local owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StudySummaryDto {
    /// Consecutive days up to today, or up to yesterday when `at_risk`.
    pub current_streak: i64,
    /// Longest streak ever recorded.
    pub longest_streak: i64,
    /// Local day used as today, in `YYYY-MM-DD` format.
    pub today: String,
    /// There was real progress today.
    pub today_active: bool,
    /// The streak still holds, but depends on studying today.
    pub at_risk: bool,
    /// Active days in the current week (Monday to Sunday).
    pub week_active_days: i64,
    /// Lessons currently completed by the local owner.
    pub completed_lessons: i64,
    /// The seven days of the current week, Monday to Sunday.
    pub days: Vec<StudyDayDto>,
}

impl From<learnvault_core::StudySummary> for StudySummaryDto {
    fn from(summary: learnvault_core::StudySummary) -> Self {
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
                .map(|day| StudyDayDto {
                    day: day.day,
                    active: day.active,
                })
                .collect(),
        }
    }
}

/// Persisted Pomodoro session for the local owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FocusSessionDto {
    /// Idempotent key of the timer.
    pub id: String,
    /// Kind: `focus`, `short` or `long`.
    pub kind: &'static str,
    /// Planned duration in seconds.
    pub planned_seconds: i64,
    /// Actual duration in seconds.
    pub actual_seconds: i64,
    /// Whether the timer completed normally.
    pub completed: bool,
    /// Unix start.
    pub started_at: i64,
    /// Fim Unix.
    pub ended_at: i64,
}

impl From<FocusSession> for FocusSessionDto {
    fn from(session: FocusSession) -> Self {
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

/// Focus day within the current week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FocusDayDto {
    /// Data local `YYYY-MM-DD`.
    pub day: String,
    /// Focused minutes completed.
    pub minutes: i64,
    /// Sessions completed.
    pub sessions: i64,
}

/// Persisted Pomodoro statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FocusSummaryDto {
    /// Focus minutes completed today.
    pub today_minutes: i64,
    /// Focus minutes in the week.
    pub week_minutes: i64,
    /// Focus sessions completed in the week.
    pub week_sessions: i64,
    /// Focus minutes accumulated in the history.
    pub total_minutes: i64,
    /// Historical total of completed sessions.
    pub total_sessions: i64,
    /// Completion rate as a percentage.
    pub completion_rate: i64,
    /// Most productive day of this week, if there is one.
    pub best_day: Option<String>,
    /// Monday to Sunday.
    pub days: Vec<FocusDayDto>,
}

impl From<FocusSummary> for FocusSummaryDto {
    fn from(summary: FocusSummary) -> Self {
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
                .map(|day| FocusDayDto {
                    day: day.day,
                    minutes: day.minutes,
                    sessions: day.sessions,
                })
                .collect(),
        }
    }
}

/// Serializable progress of a daily mission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DailyMissionDto {
    /// Stable identifier.
    pub id: String,
    /// Progress of the day.
    pub progress: i64,
    /// Completion target.
    pub target: i64,
    /// Whether the bonus was already redeemed.
    pub claimed: bool,
}

/// Persisted XP, milestones and missions of the local owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RewardSummaryDto {
    /// XP acumulado.
    pub xp: i64,
    /// Lessons completed at least once.
    pub completed_lessons: i64,
    /// Configured weekly goal.
    pub weekly_goal: i64,
    /// Missions of the day.
    pub missions: Vec<DailyMissionDto>,
    /// Streak/activity shared with the study summary.
    pub study: StudySummaryDto,
}

impl From<RewardSummary> for RewardSummaryDto {
    fn from(summary: RewardSummary) -> Self {
        Self {
            xp: summary.xp,
            completed_lessons: summary.completed_lessons,
            weekly_goal: summary.weekly_goal,
            missions: summary
                .missions
                .into_iter()
                .map(|mission| DailyMissionDto {
                    id: mission.id,
                    progress: mission.progress,
                    target: mission.target,
                    claimed: mission.claimed,
                })
                .collect(),
            study: summary.study.into(),
        }
    }
}

/// DTO of the impact of removing a media.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MediaRemovalImpactDto {
    /// Identifier of the media queried.
    pub media_id: String,
    /// Relative path of the media in the catalog.
    pub relative_path: String,
    /// Availability according to the last scan ("available" or "missing").
    pub availability: &'static str,
    /// Number of distinct owners with progress on this media.
    pub progress_owners: usize,
    /// Whether the media had already been removed from the catalog.
    pub already_removed: bool,
}

impl From<learnvault_core::MediaRemovalImpact> for MediaRemovalImpactDto {
    fn from(impact: learnvault_core::MediaRemovalImpact) -> Self {
        Self {
            media_id: impact.media_id,
            relative_path: impact.relative_path,
            availability: availability(impact.availability),
            progress_owners: impact.progress_owners,
            already_removed: impact.already_removed,
        }
    }
}

/// DTO of the result of removing a media.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MediaRemovalOutcomeDto {
    /// Identifier of the media removed.
    pub media_id: String,
    /// True when the removal was recorded now; false if it was already removed.
    pub removed_now: bool,
    /// Number of owners with progress preserved.
    pub preserved_owners: usize,
}

impl From<learnvault_core::MediaRemovalOutcome> for MediaRemovalOutcomeDto {
    fn from(outcome: learnvault_core::MediaRemovalOutcome) -> Self {
        Self {
            media_id: outcome.media_id,
            removed_now: outcome.removed_now,
            preserved_owners: outcome.preserved_owners,
        }
    }
}

/// DTO of a course's custom cover.
///
/// It carries only the version (modification seconds) that the SPA uses to invalidate the image's
/// URL. Neither the file's path nor the covers directory crosses the IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CoverDto {
    /// Version of the course's cover; absent when the course does not have a cover yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_version: Option<i64>,
}

/// DTO of the result of removing the cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CoverRemovalDto {
    /// True when there was a cover and it was removed now.
    pub removed: bool,
}

/// Informational DTO about updates of the desktop application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopUpdateInfoDto {
    /// Whether a newer update is available for download.
    pub available: bool,
    /// Version currently running in the application.
    pub current_version: String,
    /// New version available (if any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Publication date of the new version (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    /// Release notes / changelog (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

/// DTO with the media and playback capabilities in the desktop environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopMediaCapabilitiesDto {
    /// Whether local transcoding is supported (e.g. FFmpeg detected).
    pub transcoding: bool,
    /// Whether opening in the operating system's default player is supported.
    pub system_player: bool,
}

/// DTO of the courses and lessons favourited by the local owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudyFavoritesDto {
    /// Identifiers of the favourited courses.
    pub course_ids: Vec<String>,
    /// Identifiers of the favourited lessons (media).
    pub media_ids: Vec<String>,
}

impl From<StudyFavorites> for StudyFavoritesDto {
    fn from(favorites: StudyFavorites) -> Self {
        Self {
            course_ids: favorites.course_ids,
            media_ids: favorites.media_ids,
        }
    }
}

/// A note matched by global search, shaped like the HTTP payload.
///
/// Fields stay snake_case so `normalizeSearchResults` in the SPA accepts both transports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchNoteDto {
    /// Identifier of the note.
    pub note_id: String,
    /// Media (lesson) the note belongs to.
    pub media_id: String,
    /// Course that owns the media.
    pub course_id: String,
    /// Title of that course.
    pub course_title: String,
    /// Lesson path used to open the player at the note timestamp.
    pub lesson_path: String,
    /// Cleaned lesson title.
    pub lesson_title: String,
    /// Player position in seconds.
    pub position_seconds: i64,
    /// Last edit in seconds since the Unix epoch.
    pub updated_at: i64,
    /// Matched fragment; `<mark>` and `</mark>` wrap the highlighted terms.
    pub snippet: String,
}

/// A course whose title matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchCourseDto {
    /// Stable course identity.
    pub course_id: String,
    /// Title presented in the results.
    pub title: String,
}

/// A lesson whose path matched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchLessonDto {
    /// Course that owns the lesson.
    pub course_id: String,
    /// Title of that course.
    pub course_title: String,
    /// Media identity used to open the lesson.
    pub media_id: String,
    /// Lesson path inside the course.
    pub path: String,
    /// Cleaned lesson title.
    pub title: String,
}

/// Grouped global search results.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SearchResultsDto {
    /// Notes of the local owner, best match first.
    pub notes: Vec<SearchNoteDto>,
    /// Courses whose title matched.
    pub courses: Vec<SearchCourseDto>,
    /// Lessons whose path matched.
    pub lessons: Vec<SearchLessonDto>,
}
