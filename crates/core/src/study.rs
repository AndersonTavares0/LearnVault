//! Study progress: resume position and completion, per media and per owner.
//!
//! For each `owner_id` + `media_id` pair, the core keeps where playback stopped and whether the
//! lesson was completed. The `owner_id` is opaque and explicit — on the desktop it is a fixed
//! identifier, on the server it is the authenticated user — and it is the only isolation boundary between
//! the data of different people: no operation crosses owners.
//!
//! # Rules
//!
//! - position and completion are independent: advancing does not undo completion and completing does not
//!   move the resume point;
//! - resending an identical value is not a change and does **not** move `updated_at`, which
//!   represents the last real change;
//! - the media must exist in the catalog, for both reads and writes; being marked
//!   as missing does **not** prevent reading or saving progress, because the media's identity
//!   survives the temporary absence of the file;
//! - the first call for a pair with no state creates the row and counts as the user's
//!   activity, even when the value sent is neutral (position zero or not completed);
//! - each mutation is a single SQL statement: there is no read-modify-write of the state, and
//!   the media's existence is a pre-check whose ultimate guarantee is the foreign key;
//! - no operation writes to `media`, `scan`, `library` or the physical library.
//!
//! # Known limits
//!
//! - `updated_at` has one-second resolution: two real changes in the same second store
//!   the same instant, and the order of recents falls back to the `media_id` tie-break. A clock that
//!   goes backwards also makes the newer change lose priority. Both cases would require
//!   a precision the column does not offer.
//! - the `owner_id` + `media_id` pair is global: the core does not know which library the media
//!   belongs to, so deciding whether an owner may ask for that media is the adapter's
//!   responsibility.
//! - `owner_id` is used exactly as received and must be canonical — the same value for the
//!   same owner on every call. The column compares bytes, so `"a"` and `"a "` would be
//!   different owners.

use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};

/// Ceiling of results of [`recent_study`].
///
/// It is a fixed protective limit against unbounded queries, not a setting: the
/// value travels with the code and changes only with a design decision.
pub const MAX_RECENT_STUDY: usize = 100;

/// Study progress of a media for an owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyProgress {
    /// Identifier of the media, the identity that survives renames and absences.
    pub media_id: String,
    /// Resume point in seconds.
    pub position_seconds: i64,
    /// Tells whether the lesson was completed.
    pub completed: bool,
    /// Instant of the last real change, in seconds since the Unix epoch.
    ///
    /// One-second resolution: two changes in the same second store the same instant.
    pub updated_at: i64,
}

/// Reads the recorded progress, if there is any.
///
/// No progress is not an error: it means the media was never studied by this owner.
/// A media that does not exist in the catalog is reported, so "unknown" is not confused with
/// "never studied".
pub fn study_progress(
    db: &Database,
    owner_id: &str,
    media_id: &str,
) -> Result<Option<StudyProgress>> {
    validate_owner(owner_id)?;

    let conn = db.connection();
    require_media(conn, media_id)?;

    conn.query_row(
        "SELECT media_id, position_seconds, completed, updated_at FROM study_state
             WHERE owner_id = ?1 AND media_id = ?2",
        rusqlite::params![owner_id, media_id],
        progress_from_row,
    )
    .optional()
    .map_err(Error::from)
}

/// Records the resume position, preserving the completion already recorded.
pub fn record_position(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
) -> Result<StudyProgress> {
    record_position_on(db.connection(), owner_id, media_id, position_seconds)
}

/// [`record_position`] over an already-open connection.
///
/// The internal form lets a caller's transaction group the study write and the
/// record of the sync operation.
pub(crate) fn record_position_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
) -> Result<StudyProgress> {
    validate_owner(owner_id)?;

    if position_seconds < 0 {
        return Err(Error::InvalidStudyPosition { position_seconds });
    }

    require_media(conn, media_id)?;

    // `completed` does not appear in the `DO UPDATE`, so the completion is preserved by the statement
    // itself. `updated_at` only advances when the position actually changes: resending the same
    // value is idempotent and does not count as a change.
    conn.query_row(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES (?1, ?2, ?3, 0, ?4)
         ON CONFLICT (owner_id, media_id) DO UPDATE SET
             position_seconds = excluded.position_seconds,
             updated_at = CASE
                 WHEN study_state.position_seconds <> excluded.position_seconds
                 THEN excluded.updated_at
                 ELSE study_state.updated_at
             END
         RETURNING media_id, position_seconds, completed, updated_at",
        rusqlite::params![owner_id, media_id, position_seconds, now_unix_seconds()],
        progress_from_row,
    )
    .map_err(Error::from)
}

/// Marks or unmarks completion, preserving the resume position.
pub fn set_completed(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    completed: bool,
) -> Result<StudyProgress> {
    set_completed_on(db.connection(), owner_id, media_id, completed)
}

/// [`set_completed`] over an already-open connection.
pub(crate) fn set_completed_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    completed: bool,
) -> Result<StudyProgress> {
    validate_owner(owner_id)?;

    require_media(conn, media_id)?;

    // Symmetric to [`record_position`]: `position_seconds` is not touched by the `DO UPDATE`, and
    // the instant only advances when the completion actually changes.
    conn.query_row(
        "INSERT INTO study_state (owner_id, media_id, position_seconds, completed, updated_at)
         VALUES (?1, ?2, 0, ?3, ?4)
         ON CONFLICT (owner_id, media_id) DO UPDATE SET
             completed = excluded.completed,
             updated_at = CASE
                 WHEN study_state.completed <> excluded.completed
                 THEN excluded.updated_at
                 ELSE study_state.updated_at
             END
         RETURNING media_id, position_seconds, completed, updated_at",
        rusqlite::params![owner_id, media_id, i64::from(completed), now_unix_seconds()],
        progress_from_row,
    )
    .map_err(Error::from)
}

/// Lists the owner's progress, from most recently changed to oldest.
///
/// Ties between instants are broken by `media_id`, so the order is deterministic.
/// The limit is reduced to the ceiling of [`MAX_RECENT_STUDY`]; a zero limit queries nothing.
/// Media removed from the catalog do not appear, but their progress stays stored.
pub fn recent_study(db: &Database, owner_id: &str, limit: usize) -> Result<Vec<StudyProgress>> {
    validate_owner(owner_id)?;

    let limit = limit.min(MAX_RECENT_STUDY);
    if limit == 0 {
        return Ok(Vec::new());
    }

    let conn = db.connection();
    let mut statement = conn.prepare(
        "SELECT media_id, position_seconds, completed, updated_at FROM study_state
         WHERE owner_id = ?1
           AND NOT EXISTS (SELECT 1
                             FROM media_removal r
                             JOIN media m
                               ON m.library_id = r.library_id
                              AND m.media_id = r.media_id
                            WHERE m.media_id = study_state.media_id)
         ORDER BY updated_at DESC, media_id ASC
         LIMIT ?2",
    )?;
    let rows = statement.query_map(
        rusqlite::params![owner_id, i64::try_from(limit).unwrap_or(i64::MAX)],
        progress_from_row,
    )?;

    let mut states = Vec::new();
    for state in rows {
        states.push(state?);
    }

    Ok(states)
}

/// Ceiling of items per page of [`study_page`].
pub const MAX_STUDY_PAGE: usize = 500;

/// Position after the last item of a [`study_page`], in the order of [`recent_study`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyCursor {
    updated_at: i64,
    media_id: String,
}

impl StudyCursor {
    /// Opaque text form, `updated_at` first because it never contains a dot.
    pub fn encode(&self) -> String {
        format!("{}.{}", self.updated_at, self.media_id)
    }

    /// Parses [`StudyCursor::encode`]; `None` for any other text.
    pub fn parse(text: &str) -> Option<Self> {
        let (updated_at, media_id) = text.split_once('.')?;
        if media_id.is_empty() {
            return None;
        }
        Some(Self {
            updated_at: updated_at.parse().ok()?,
            media_id: media_id.to_owned(),
        })
    }
}

/// One page of the owner's complete study state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyPage {
    /// Progress items, from most recently changed to oldest.
    pub items: Vec<StudyProgress>,
    /// Where the next page starts; `None` on the last page.
    pub next: Option<StudyCursor>,
}

/// Pages through the owner's whole progress, in the order of [`recent_study`].
///
/// Unlike [`recent_study`] it has no overall ceiling: a caller walks `next` until it is `None`.
/// `allowed_libraries` is the adapter's authorization, applied in the same query so a page is never
/// short because of rows the caller may not see: `None` allows every library. The limit is
/// clamped to `1..=`[`MAX_STUDY_PAGE`]. Media removed from the catalog do not appear.
pub fn study_page(
    db: &Database,
    owner_id: &str,
    allowed_libraries: Option<&[String]>,
    after: Option<&StudyCursor>,
    limit: usize,
) -> Result<StudyPage> {
    validate_owner(owner_id)?;

    let limit = limit.clamp(1, MAX_STUDY_PAGE);
    // A list that cannot be serialized allows nothing: authorization fails closed.
    let allowed =
        allowed_libraries.map(|ids| serde_json::to_string(ids).unwrap_or_else(|_| "[]".to_owned()));
    let (after_at, after_id) =
        after.map_or((i64::MAX, ""), |c| (c.updated_at, c.media_id.as_str()));

    let mut statement = db.connection().prepare(
        "SELECT s.media_id, s.position_seconds, s.completed, s.updated_at
           FROM study_state s
           JOIN media m ON m.media_id = s.media_id
          WHERE s.owner_id = ?1
            AND (s.updated_at < ?2 OR (s.updated_at = ?2 AND s.media_id > ?3))
            AND (?4 IS NULL OR m.library_id IN (SELECT value FROM json_each(?4)))
            AND NOT EXISTS (SELECT 1 FROM media_removal r
                             WHERE r.library_id = m.library_id AND r.media_id = m.media_id)
          ORDER BY s.updated_at DESC, s.media_id ASC
          LIMIT ?5",
    )?;
    let rows = statement.query_map(
        rusqlite::params![
            owner_id,
            after_at,
            after_id,
            allowed,
            i64::try_from(limit + 1).unwrap_or(i64::MAX)
        ],
        progress_from_row,
    )?;
    let mut items = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    let next = (items.len() > limit).then(|| {
        items.truncate(limit);
        items.last().map(|last| StudyCursor {
            updated_at: last.updated_at,
            media_id: last.media_id.clone(),
        })
    });

    Ok(StudyPage {
        items,
        next: next.flatten(),
    })
}

/// Refuses an unidentified owner.
pub(crate) fn validate_owner(owner_id: &str) -> Result<()> {
    if owner_id.trim().is_empty() {
        return Err(Error::InvalidOwnerId);
    }

    Ok(())
}

/// Refuses a media that does not exist in the catalog.
///
/// The file's availability is not consulted: a missing media is still a
/// known media, with its own identity and progress.
pub(crate) fn require_media(conn: &rusqlite::Connection, media_id: &str) -> Result<()> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM media WHERE media_id = ?1",
            [media_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some();

    if exists {
        Ok(())
    } else {
        Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        })
    }
}

/// Translates a `study_state` row into the progress state.
fn progress_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StudyProgress> {
    Ok(StudyProgress {
        media_id: row.get(0)?,
        position_seconds: row.get(1)?,
        completed: row.get::<_, i64>(2)? != 0,
        updated_at: row.get(3)?,
    })
}

/// A day of the current week and whether it had activity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyDay {
    /// Data local no formato `YYYY-MM-DD`.
    pub day: String,
    /// `true` when there was progress or completion on that day.
    pub active: bool,
}

/// Summary of the streak and of the current week.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudySummary {
    /// Consecutive days up to today (or up to yesterday, when the streak is at risk).
    pub current_streak: i64,
    /// Longest streak ever recorded.
    pub longest_streak: i64,
    /// Local day used as today, in `YYYY-MM-DD` format.
    pub today: String,
    /// Whether there was activity today.
    pub today_active: bool,
    /// `true` when today does not count yet, but yesterday's streak is still alive.
    pub at_risk: bool,
    /// Active days in the current week, from Monday to Sunday.
    pub week_active_days: i64,
    /// Lessons currently marked as completed by the owner.
    pub completed_lessons: i64,
    /// The seven days of the current week, Monday to Sunday.
    pub days: Vec<StudyDay>,
    /// Active days in the last 365 days up to today, in ascending order, in `YYYY-MM-DD` format.
    pub active_days_year: Vec<String>,
}

/// Summary using the database's current local day.
pub fn study_summary(db: &Database, owner_id: &str) -> Result<StudySummary> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let today: String = conn.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))?;
    summary_for_connection(conn, owner_id, &today)
}

/// Summary with an explicit day, for deterministic tests.
///
/// The day must be a canonical ISO date (`YYYY-MM-DD`); ambiguous dates such as
/// `2026-1-5` are refused instead of being silently normalized.
pub fn study_summary_on(db: &Database, owner_id: &str, today: &str) -> Result<StudySummary> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    let valid: i64 = conn.query_row(
        "SELECT CASE WHEN date(?1) = ?1 THEN 1 ELSE 0 END",
        [today],
        |row| row.get(0),
    )?;
    if valid == 0 {
        return Err(Error::InvalidStudyDay {
            day: today.to_owned(),
        });
    }
    summary_for_connection(conn, owner_id, today)
}

fn summary_for_connection(
    conn: &rusqlite::Connection,
    owner_id: &str,
    today: &str,
) -> Result<StudySummary> {
    let mut statement = conn.prepare(
        "SELECT CAST(julianday(day) + 0.5 AS INTEGER) AS day_number
         FROM study_day
         WHERE owner_id = ?1 AND day <= ?2
         ORDER BY day ASC",
    )?;
    let active_days = statement
        .query_map(params![owner_id, today], |row| row.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);

    let active: HashSet<i64> = active_days.iter().copied().collect();
    let today_number: i64 = conn.query_row(
        "SELECT CAST(julianday(?1) + 0.5 AS INTEGER)",
        [today],
        |row| row.get(0),
    )?;
    let today_active = active.contains(&today_number);
    let yesterday_number = today_number - 1;
    let yesterday_active = active.contains(&yesterday_number);
    let at_risk = !today_active && yesterday_active;

    let mut current_streak = 0;
    let mut cursor = if today_active {
        today_number
    } else if yesterday_active {
        yesterday_number
    } else {
        0
    };
    if cursor != 0 {
        while active.contains(&cursor) {
            current_streak += 1;
            cursor -= 1;
        }
    }

    let mut longest_streak = 0;
    let mut run = 0;
    let mut previous = None;
    for day in &active_days {
        run = if previous.is_some_and(|value| value + 1 == *day) {
            run + 1
        } else {
            1
        };
        longest_streak = longest_streak.max(run);
        previous = Some(*day);
    }

    let weekday: i64 = conn.query_row(
        "SELECT CAST(strftime('%w', ?1) AS INTEGER)",
        [today],
        |row| row.get(0),
    )?;
    let monday = today_number - (weekday + 6) % 7;
    let mut days = Vec::with_capacity(7);
    for offset in 0..7 {
        let number = monday + offset;
        let day: String = conn.query_row("SELECT date(?1)", [number], |row| row.get(0))?;
        days.push(StudyDay {
            day,
            active: active.contains(&number),
        });
    }
    let week_active_days = days.iter().filter(|day| day.active).count() as i64;
    let mut active_days_year = Vec::new();
    for number in active_days
        .iter()
        .copied()
        .filter(|number| *number > today_number - 365)
    {
        let day: String = conn.query_row("SELECT date(?1)", [number], |row| row.get(0))?;
        active_days_year.push(day);
    }
    let completed_lessons: i64 = conn.query_row(
        "SELECT COUNT(*) FROM study_state WHERE owner_id = ?1 AND completed = 1",
        [owner_id],
        |row| row.get(0),
    )?;

    Ok(StudySummary {
        current_streak,
        longest_streak,
        today: today.to_owned(),
        today_active,
        at_risk,
        week_active_days,
        completed_lessons,
        days,
        active_days_year,
    })
}

/// Study note associated with a media and a point on the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StudyNote {
    /// Opaque identifier of the note (UUID v7).
    pub note_id: String,
    /// Identifier of the linked media.
    pub media_id: String,
    /// Position in the video player in seconds.
    pub position_seconds: i64,
    /// Text content of the note.
    pub content: String,
    /// Creation instant in seconds since the Unix epoch.
    pub created_at: i64,
    /// Instant of the last edit in seconds since the Unix epoch.
    pub updated_at: i64,
}

/// Study note with media metadata for global navigation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalStudyNote {
    /// Opaque identifier of the note (UUID v7).
    pub note_id: String,
    /// Identifier of the linked media.
    pub media_id: String,
    /// Relative path of the media in the catalog.
    pub relative_path: String,
    /// Position in the video player in seconds.
    pub position_seconds: i64,
    /// Text content of the note.
    pub content: String,
    /// Creation instant in seconds since the Unix epoch.
    pub created_at: i64,
    /// Instant of the last edit in seconds since the Unix epoch.
    pub updated_at: i64,
}

/// Lists the owner's notes for a specific media, in chronological order by the lesson's timestamp.
pub fn list_study_notes(db: &Database, owner_id: &str, media_id: &str) -> Result<Vec<StudyNote>> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    require_media(conn, media_id)?;

    let mut statement = conn.prepare(
        "SELECT note_id, media_id, position_seconds, content, created_at, updated_at
         FROM study_note
         WHERE owner_id = ?1 AND media_id = ?2
         ORDER BY position_seconds ASC, created_at ASC",
    )?;
    let rows = statement.query_map(rusqlite::params![owner_id, media_id], note_from_row)?;

    let mut notes = Vec::new();
    for note in rows {
        notes.push(note?);
    }
    Ok(notes)
}

/// Finds the media linked to a study note without returning the note contents.
pub fn study_note_media_id(db: &Database, note_id: &str) -> Result<Option<String>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT media_id FROM study_note WHERE note_id = ?1",
            [note_id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Lists all of the owner's notes across the whole catalog, from newest to oldest.
pub fn list_all_study_notes(db: &Database, owner_id: &str) -> Result<Vec<GlobalStudyNote>> {
    validate_owner(owner_id)?;
    let conn = db.connection();

    let mut statement = conn.prepare(
        "SELECT n.note_id, n.media_id, m.relative_path, n.position_seconds, n.content, n.created_at, n.updated_at
         FROM study_note n
         JOIN media m ON n.media_id = m.media_id
         WHERE n.owner_id = ?1
           AND NOT EXISTS (SELECT 1
                             FROM media_removal r
                            WHERE r.library_id = m.library_id
                              AND r.media_id = m.media_id)
         ORDER BY n.updated_at DESC, n.created_at DESC",
    )?;
    let rows = statement.query_map(rusqlite::params![owner_id], |row| {
        Ok(GlobalStudyNote {
            note_id: row.get(0)?,
            media_id: row.get(1)?,
            relative_path: row.get(2)?,
            position_seconds: row.get(3)?,
            content: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?;

    let mut notes = Vec::new();
    for note in rows {
        notes.push(note?);
    }
    Ok(notes)
}

/// Creates a new study note with a time marker.
pub fn create_study_note(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    content: &str,
) -> Result<StudyNote> {
    create_study_note_on(
        db.connection(),
        owner_id,
        media_id,
        position_seconds,
        content,
    )
}

/// [`create_study_note`] over an already-open connection.
pub(crate) fn create_study_note_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    content: &str,
) -> Result<StudyNote> {
    validate_owner(owner_id)?;
    if position_seconds < 0 {
        return Err(Error::InvalidStudyPosition { position_seconds });
    }
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 10000 {
        return Err(Error::InvalidStudyNoteContent);
    }

    require_media(conn, media_id)?;

    let note_id = crate::ids::new_note_id();
    let now = now_unix_seconds();

    conn.execute(
        "INSERT INTO study_note (note_id, owner_id, media_id, position_seconds, content, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![note_id, owner_id, media_id, position_seconds, trimmed, now, now],
    )?;

    Ok(StudyNote {
        note_id,
        media_id: media_id.to_owned(),
        position_seconds,
        content: trimmed.to_owned(),
        created_at: now,
        updated_at: now,
    })
}

/// Updates the content of an existing note of the owner.
pub fn update_study_note(
    db: &Database,
    owner_id: &str,
    note_id: &str,
    content: &str,
) -> Result<StudyNote> {
    update_study_note_on(db.connection(), owner_id, note_id, content)
}

/// [`update_study_note`] over an already-open connection.
pub(crate) fn update_study_note_on(
    conn: &Connection,
    owner_id: &str,
    note_id: &str,
    content: &str,
) -> Result<StudyNote> {
    validate_owner(owner_id)?;
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 10000 {
        return Err(Error::InvalidStudyNoteContent);
    }

    let now = now_unix_seconds();

    let updated = conn
        .query_row(
            "UPDATE study_note
         SET content = ?1, updated_at = ?2
         WHERE owner_id = ?3 AND note_id = ?4
         RETURNING note_id, media_id, position_seconds, content, created_at, updated_at",
            rusqlite::params![trimmed, now, owner_id, note_id],
            note_from_row,
        )
        .optional()?;

    updated.ok_or_else(|| Error::StudyNoteNotFound {
        note_id: note_id.to_owned(),
    })
}

/// Removes a study note that belongs to the owner.
///
/// The tombstone is recorded here, not in the sync adapter, so every deletion path —
/// including the server handler, which is the journal itself and has no outbox to drain —
/// blocks a stale offline create from resurrecting the row. Tracked in #472.
pub fn delete_study_note(db: &Database, owner_id: &str, note_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    Ok(delete_study_note_on(db.connection(), owner_id, note_id)?.is_some())
}

/// [`delete_study_note`] over an open connection; returns the delete instant.
pub(crate) fn delete_study_note_on(
    conn: &Connection,
    owner_id: &str,
    note_id: &str,
) -> Result<Option<i64>> {
    let deleted_at = now_unix_seconds();
    let changes = conn.execute(
        "DELETE FROM study_note WHERE owner_id = ?1 AND note_id = ?2",
        rusqlite::params![owner_id, note_id],
    )?;
    if changes == 0 {
        return Ok(None);
    }
    crate::sync::record_tombstone_on(
        conn,
        owner_id,
        crate::sync::TOMBSTONE_NOTE,
        note_id,
        deleted_at,
    )?;
    Ok(Some(deleted_at))
}

fn note_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StudyNote> {
    Ok(StudyNote {
        note_id: row.get(0)?,
        media_id: row.get(1)?,
        position_seconds: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

/// Rating of a flashcard review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewRating {
    /// The learner got it wrong or needs to repeat soon (10 minutes).
    Again,
    /// The learner recalled it with effort; shorter growth step.
    Hard,
    /// The learner got it right and understood the concept.
    Good,
    /// The learner recalled it effortlessly; faster interval growth.
    Easy,
}

impl ReviewRating {
    /// Converts from a string (`"again"`, `"hard"`, `"good"`, or `"easy"`), case-insensitive.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "again" => Some(Self::Again),
            "hard" => Some(Self::Hard),
            "good" => Some(Self::Good),
            "easy" => Some(Self::Easy),
            _ => None,
        }
    }

    /// Returns the static string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Again => "again",
            Self::Hard => "hard",
            Self::Good => "good",
            Self::Easy => "easy",
        }
    }
}

/// Personal study card for spaced repetition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flashcard {
    /// Opaque identifier of the card (UUID v7).
    pub card_id: String,
    /// Owner of the card.
    pub owner_id: String,
    /// Linked media.
    pub media_id: String,
    /// Instant of the lesson at which the card was created, when given.
    pub position_seconds: Option<i64>,
    /// Question or content of the front.
    pub front: String,
    /// Answer or content of the back.
    pub back: String,
    /// Current repetition interval in days (0 to 365).
    pub interval_days: i64,
    /// Instant of the next review in Unix seconds.
    pub next_due: i64,
    /// Creation instant in Unix seconds.
    pub created_at: i64,
    /// Instant of the last edit in Unix seconds.
    pub updated_at: i64,
}

/// Validates the text of the front or back of the card (1 to 4000 Unicode scalars, non-empty).
pub(crate) fn validate_card_text(text: &str) -> Result<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 4000 {
        return Err(Error::InvalidFlashcardContent);
    }
    Ok(trimmed.to_owned())
}

/// Lists the owner's cards for a specific media, ordered by due date (`next_due ASC, created_at ASC`).
pub fn list_flashcards(db: &Database, owner_id: &str, media_id: &str) -> Result<Vec<Flashcard>> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    require_media(conn, media_id)?;

    let mut statement = conn.prepare(
        "SELECT card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at
         FROM flashcard
         WHERE owner_id = ?1 AND media_id = ?2
         ORDER BY next_due ASC, created_at ASC",
    )?;
    let rows = statement.query_map(rusqlite::params![owner_id, media_id], card_from_row)?;

    let mut cards = Vec::new();
    for card in rows {
        cards.push(card?);
    }
    Ok(cards)
}

/// Finds the media linked to a flashcard without returning its contents.
pub fn flashcard_media_id(db: &Database, card_id: &str) -> Result<Option<String>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT media_id FROM flashcard WHERE card_id = ?1",
            [card_id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Lists every card of the owner across all lessons, soonest due first. The review hub needs
/// what is due everywhere, not one lesson at a time.
pub fn list_all_flashcards(db: &Database, owner_id: &str) -> Result<Vec<Flashcard>> {
    validate_owner(owner_id)?;
    let mut statement = db.connection().prepare(
        "SELECT card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at
         FROM flashcard
         WHERE owner_id = ?1
           AND NOT EXISTS (
               SELECT 1
               FROM media_removal r
               JOIN media m
                 ON m.library_id = r.library_id
                AND m.media_id = r.media_id
               WHERE m.media_id = flashcard.media_id
           )
         ORDER BY next_due ASC, created_at ASC",
    )?;
    let rows = statement.query_map([owner_id], card_from_row)?;
    let mut cards = Vec::new();
    for card in rows {
        cards.push(card?);
    }
    Ok(cards)
}

/// Creates a new study card linked to the lesson.
///
/// The new card is due immediately (`next_due = now`, `interval_days = 0`).
pub fn create_flashcard(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    front: &str,
    back: &str,
    position_seconds: Option<i64>,
) -> Result<Flashcard> {
    create_flashcard_on(
        db.connection(),
        owner_id,
        media_id,
        front,
        back,
        position_seconds,
    )
}

/// [`create_flashcard`] over an open connection, so the sync queue can reuse the rules
/// inside its own transaction.
pub(crate) fn create_flashcard_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    front: &str,
    back: &str,
    position_seconds: Option<i64>,
) -> Result<Flashcard> {
    validate_owner(owner_id)?;
    let front_trimmed = validate_card_text(front)?;
    let back_trimmed = validate_card_text(back)?;
    if let Some(position) = position_seconds
        && position < 0
    {
        return Err(Error::InvalidStudyPosition {
            position_seconds: position,
        });
    }

    require_media(conn, media_id)?;

    let card_id = crate::ids::new_card_id();
    let now = now_unix_seconds();
    let interval_days = 0_i64;
    let next_due = now;

    conn.execute(
        "INSERT INTO flashcard (card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at, text_updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        rusqlite::params![
            card_id,
            owner_id,
            media_id,
            position_seconds,
            front_trimmed,
            back_trimmed,
            interval_days,
            next_due,
            now,
            now
        ],
    )?;

    Ok(Flashcard {
        card_id,
        owner_id: owner_id.to_owned(),
        media_id: media_id.to_owned(),
        position_seconds,
        front: front_trimmed,
        back: back_trimmed,
        interval_days,
        next_due,
        created_at: now,
        updated_at: now,
    })
}

/// Updates the text of the front and/or back of an existing card.
///
/// Preserves the schedule (`next_due` and `interval_days`) and updates `updated_at`.
pub fn update_flashcard(
    db: &Database,
    owner_id: &str,
    card_id: &str,
    front: &str,
    back: &str,
) -> Result<Flashcard> {
    update_flashcard_on(db.connection(), owner_id, card_id, front, back)
}

/// [`update_flashcard`] over an open connection.
pub(crate) fn update_flashcard_on(
    conn: &Connection,
    owner_id: &str,
    card_id: &str,
    front: &str,
    back: &str,
) -> Result<Flashcard> {
    validate_owner(owner_id)?;
    let front_trimmed = validate_card_text(front)?;
    let back_trimmed = validate_card_text(back)?;

    let now = now_unix_seconds();

    let updated = conn
        .query_row(
            "UPDATE flashcard
             SET front = ?1, back = ?2, updated_at = MAX(updated_at, ?3), text_updated_at = ?3
             WHERE owner_id = ?4 AND card_id = ?5
             RETURNING card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at",
            rusqlite::params![front_trimmed, back_trimmed, now, owner_id, card_id],
            card_from_row,
        )
        .optional()?;

    updated.ok_or_else(|| Error::FlashcardNotFound {
        card_id: card_id.to_owned(),
    })
}

/// Removes a study card that belongs to the owner.
///
/// Records the tombstone for the same reason as [`delete_study_note`]: a card created
/// offline must not resurrect a deleted one.
pub fn delete_flashcard(db: &Database, owner_id: &str, card_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    Ok(delete_flashcard_on(db.connection(), owner_id, card_id)?.is_some())
}

/// [`delete_flashcard`] over an open connection; returns the delete instant.
pub(crate) fn delete_flashcard_on(
    conn: &Connection,
    owner_id: &str,
    card_id: &str,
) -> Result<Option<i64>> {
    let deleted_at = now_unix_seconds();
    let changes = conn.execute(
        "DELETE FROM flashcard WHERE owner_id = ?1 AND card_id = ?2",
        rusqlite::params![owner_id, card_id],
    )?;
    if changes == 0 {
        return Ok(None);
    }
    crate::sync::record_tombstone_on(
        conn,
        owner_id,
        crate::sync::TOMBSTONE_FLASHCARD,
        card_id,
        deleted_at,
    )?;
    Ok(Some(deleted_at))
}

/// Records a review (Again or Good) on an existing card.
///
/// Scheduling rules:
/// - `Again`: resets the interval (`interval_days = 0`) and schedules for 10 minutes from now (`next_due = now + 600`).
/// - `Good`: if `interval_days == 0`, sets 1 day (`interval_days = 1`), otherwise doubles the interval
///   (`interval_days = min(interval_days * 2, 365)`), and schedules for `now + interval_days * 86400`.
///
/// The schedule and the review log change in the same transaction: either the whole review exists, or
/// no part of it does. The log is append-only and keeps the previous interval.
pub fn review_flashcard(
    db: &Database,
    owner_id: &str,
    card_id: &str,
    rating: ReviewRating,
) -> Result<Flashcard> {
    let conn = db.connection();
    let transaction = conn.unchecked_transaction()?;
    let card = review_flashcard_on(&transaction, owner_id, card_id, rating)?;
    transaction.commit()?;
    Ok(card)
}

/// Next scheduling state of a review: `(interval_days, next_due)`.
///
/// Shared by the local review and by the review that arrives through the sync journal,
/// so the two never diverge.
pub(crate) fn schedule_review(
    prev_interval: i64,
    rating: ReviewRating,
    reviewed_at: i64,
) -> (i64, i64) {
    match rating {
        ReviewRating::Again => (0_i64, reviewed_at + 600),
        ReviewRating::Hard => {
            if prev_interval == 0 {
                (0_i64, reviewed_at + 43_200)
            } else {
                let next_interval = (prev_interval.saturating_mul(6) / 5)
                    .max(prev_interval)
                    .min(365);
                (
                    next_interval,
                    reviewed_at + next_interval.saturating_mul(86_400),
                )
            }
        }
        ReviewRating::Good => {
            let next_interval = if prev_interval == 0 {
                1_i64
            } else {
                prev_interval.saturating_mul(2).min(365)
            };
            (
                next_interval,
                reviewed_at + next_interval.saturating_mul(86_400),
            )
        }
        ReviewRating::Easy => {
            let next_interval = if prev_interval == 0 {
                4_i64
            } else {
                prev_interval.saturating_mul(3).min(365)
            };
            (
                next_interval,
                reviewed_at + next_interval.saturating_mul(86_400),
            )
        }
    }
}

/// [`review_flashcard`] over a connection already inside a transaction.
pub(crate) fn review_flashcard_on(
    conn: &Connection,
    owner_id: &str,
    card_id: &str,
    rating: ReviewRating,
) -> Result<Flashcard> {
    validate_owner(owner_id)?;
    let now = now_unix_seconds();

    // Looks up the current card to get the previous interval_days
    let current_interval: Option<i64> = conn
        .query_row(
            "SELECT interval_days FROM flashcard WHERE owner_id = ?1 AND card_id = ?2",
            rusqlite::params![owner_id, card_id],
            |row| row.get(0),
        )
        .optional()?;

    let Some(prev_interval) = current_interval else {
        return Err(Error::FlashcardNotFound {
            card_id: card_id.to_owned(),
        });
    };

    let (new_interval, next_due) = schedule_review(prev_interval, rating, now);

    let updated = conn.query_row(
        "UPDATE flashcard
         SET interval_days = ?1, next_due = ?2, updated_at = ?3
         WHERE owner_id = ?4 AND card_id = ?5
         RETURNING card_id, owner_id, media_id, position_seconds, front, back, interval_days, next_due, created_at, updated_at",
        rusqlite::params![new_interval, next_due, now, owner_id, card_id],
        card_from_row,
    )?;

    conn.execute(
        "INSERT INTO flashcard_review
             (owner_id, card_id, rating, previous_interval_days, next_due, reviewed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            owner_id,
            card_id,
            rating.as_str(),
            prev_interval,
            next_due,
            now
        ],
    )?;

    Ok(updated)
}

/// A review recorded in a card's history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashcardReview {
    /// Sequential identifier of the review.
    pub review_id: i64,
    /// Card reviewed.
    pub card_id: String,
    /// Rating given by the learner.
    pub rating: ReviewRating,
    /// Interval in days before this review.
    pub previous_interval_days: i64,
    /// Next due date calculated by this review.
    pub next_due: i64,
    /// Instant of the review in Unix seconds.
    pub reviewed_at: i64,
}

/// Ceiling of results of [`list_flashcard_reviews`].
///
/// Travels with the code and changes only with a design decision, like [`MAX_RECENT_STUDY`].
pub const MAX_FLASHCARD_REVIEWS: usize = 100;

/// Lists the owner's most recent reviews for a card, from newest to oldest.
///
/// The limit is reduced to the ceiling of [`MAX_FLASHCARD_REVIEWS`]; a zero limit queries
/// nothing. Ties between instants are broken by `review_id`, for a deterministic order.
pub fn list_flashcard_reviews(
    db: &Database,
    owner_id: &str,
    card_id: &str,
    limit: usize,
) -> Result<Vec<FlashcardReview>> {
    validate_owner(owner_id)?;

    let limit = limit.min(MAX_FLASHCARD_REVIEWS);
    if limit == 0 {
        return Ok(Vec::new());
    }

    let mut statement = db.connection().prepare(
        "SELECT review_id, card_id, rating, previous_interval_days, next_due, reviewed_at
         FROM flashcard_review
         WHERE owner_id = ?1 AND card_id = ?2
         ORDER BY reviewed_at DESC, review_id DESC
         LIMIT ?3",
    )?;
    let rows = statement.query_map(
        rusqlite::params![owner_id, card_id, i64::try_from(limit).unwrap_or(i64::MAX)],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        },
    )?;

    let mut reviews = Vec::new();
    for row in rows {
        let (review_id, card_id, rating, previous_interval_days, next_due, reviewed_at) = row?;
        let Some(rating) = ReviewRating::parse(&rating) else {
            return Err(Error::InvalidFlashcardRating);
        };
        reviews.push(FlashcardReview {
            review_id,
            card_id,
            rating,
            previous_interval_days,
            next_due,
            reviewed_at,
        });
    }
    Ok(reviews)
}

/// Counts the whole review history of an owner's card, even beyond the listing's ceiling.
pub fn count_flashcard_reviews(db: &Database, owner_id: &str, card_id: &str) -> Result<i64> {
    validate_owner(owner_id)?;
    Ok(db.connection().query_row(
        "SELECT COUNT(*) FROM flashcard_review WHERE owner_id = ?1 AND card_id = ?2",
        rusqlite::params![owner_id, card_id],
        |row| row.get(0),
    )?)
}

/// Counts the owner's cards that are due now, the same predicate as the review queue.
///
/// Cards never reviewed and cards reviewed in the past count; a future `next_due` does not. The owner
/// is the only boundary: filtering by course access is the adapter's job.
pub fn count_due_flashcards(db: &Database, owner_id: &str) -> Result<i64> {
    validate_owner(owner_id)?;
    Ok(db.connection().query_row(
        "SELECT COUNT(*) FROM flashcard
         WHERE owner_id = ?1
           AND next_due <= ?2
           AND NOT EXISTS (
               SELECT 1
               FROM media_removal r
               JOIN media m
                 ON m.library_id = r.library_id
                AND m.media_id = r.media_id
               WHERE m.media_id = flashcard.media_id
           )",
        rusqlite::params![owner_id, now_unix_seconds()],
        |row| row.get(0),
    )?)
}

fn card_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Flashcard> {
    Ok(Flashcard {
        card_id: row.get(0)?,
        owner_id: row.get(1)?,
        media_id: row.get(2)?,
        position_seconds: row.get(3)?,
        front: row.get(4)?,
        back: row.get(5)?,
        interval_days: row.get(6)?,
        next_due: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

/// Character limit of a time marker's label.
pub const MAX_BOOKMARK_LABEL_CHARS: usize = 200;

/// Time marker of a media, visible only to the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaBookmark {
    /// Opaque identifier of the marker (UUID v7).
    pub bookmark_id: String,
    /// Identifier of the linked media.
    pub media_id: String,
    /// Instant of the media in seconds.
    pub position_seconds: i64,
    /// Short label written by the learner.
    pub label: String,
    /// Creation instant in seconds since the Unix epoch.
    pub created_at: i64,
}

/// Lists the owner's markers for a media, in chronological order.
pub fn list_media_bookmarks(
    db: &Database,
    owner_id: &str,
    media_id: &str,
) -> Result<Vec<MediaBookmark>> {
    validate_owner(owner_id)?;
    let conn = db.connection();
    require_media(conn, media_id)?;

    let mut statement = conn.prepare(
        "SELECT bookmark_id, media_id, position_seconds, label, created_at
         FROM media_bookmark
         WHERE owner_id = ?1 AND media_id = ?2
         ORDER BY position_seconds ASC, created_at ASC",
    )?;
    let rows = statement.query_map(rusqlite::params![owner_id, media_id], bookmark_from_row)?;

    let mut bookmarks = Vec::new();
    for bookmark in rows {
        bookmarks.push(bookmark?);
    }
    Ok(bookmarks)
}

/// Creates a time marker for the owner and the media.
pub fn create_media_bookmark(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    label: &str,
) -> Result<MediaBookmark> {
    create_media_bookmark_on(db.connection(), owner_id, media_id, position_seconds, label)
}

/// [`create_media_bookmark`] over an already-open connection.
pub(crate) fn create_media_bookmark_on(
    conn: &Connection,
    owner_id: &str,
    media_id: &str,
    position_seconds: i64,
    label: &str,
) -> Result<MediaBookmark> {
    validate_owner(owner_id)?;
    if position_seconds < 0 {
        return Err(Error::InvalidStudyPosition { position_seconds });
    }
    let trimmed = label.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_BOOKMARK_LABEL_CHARS {
        return Err(Error::InvalidBookmarkLabel);
    }

    require_media(conn, media_id)?;

    let bookmark_id = crate::ids::new_bookmark_id();
    let now = now_unix_seconds();

    conn.execute(
        "INSERT INTO media_bookmark (bookmark_id, owner_id, media_id, position_seconds, label, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![bookmark_id, owner_id, media_id, position_seconds, trimmed, now],
    )?;

    Ok(MediaBookmark {
        bookmark_id,
        media_id: media_id.to_owned(),
        position_seconds,
        label: trimmed.to_owned(),
        created_at: now,
    })
}

/// Removes a marker that belongs to the owner.
///
/// Records the tombstone for the same reason as [`delete_study_note`]: a bookmark created
/// offline must not resurrect a deleted one. Tracked in #472.
pub fn delete_media_bookmark(db: &Database, owner_id: &str, bookmark_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    Ok(delete_media_bookmark_on(db.connection(), owner_id, bookmark_id)?.is_some())
}

/// [`delete_media_bookmark`] over an open connection; returns the delete instant.
pub(crate) fn delete_media_bookmark_on(
    conn: &Connection,
    owner_id: &str,
    bookmark_id: &str,
) -> Result<Option<i64>> {
    let deleted_at = now_unix_seconds();
    let changes = conn.execute(
        "DELETE FROM media_bookmark WHERE owner_id = ?1 AND bookmark_id = ?2",
        rusqlite::params![owner_id, bookmark_id],
    )?;
    if changes == 0 {
        return Ok(None);
    }
    crate::sync::record_tombstone_on(
        conn,
        owner_id,
        crate::sync::TOMBSTONE_BOOKMARK,
        bookmark_id,
        deleted_at,
    )?;
    Ok(Some(deleted_at))
}

/// Finds the media linked to a bookmark without returning its contents.
pub fn media_bookmark_media_id(db: &Database, bookmark_id: &str) -> Result<Option<String>> {
    Ok(db
        .connection()
        .query_row(
            "SELECT media_id FROM media_bookmark WHERE bookmark_id = ?1",
            [bookmark_id],
            |row| row.get(0),
        )
        .optional()?)
}

fn bookmark_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaBookmark> {
    Ok(MediaBookmark {
        bookmark_id: row.get(0)?,
        media_id: row.get(1)?,
        position_seconds: row.get(2)?,
        label: row.get(3)?,
        created_at: row.get(4)?,
    })
}

/// Favourites of courses and lessons of an owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StudyFavorites {
    /// Identifiers of favourited courses, from newest to oldest.
    pub course_ids: Vec<String>,
    /// Identifiers of favourited media (lessons), from newest to oldest.
    pub media_ids: Vec<String>,
}

fn validate_favorite_target(target_id: &str) -> Result<String> {
    let trimmed = target_id.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return Err(Error::InvalidFavoriteTarget);
    }
    Ok(trimmed.to_owned())
}

/// Lists all courses and lessons favourited by the owner.
///
/// Lessons that were explicitly removed from the catalog (`media_removal`) do not appear.
pub fn list_favorites(db: &Database, owner_id: &str) -> Result<StudyFavorites> {
    validate_owner(owner_id)?;
    let conn = db.connection();

    let mut course_stmt = conn.prepare(
        "SELECT target_id FROM study_favorite
         WHERE owner_id = ?1 AND target_kind = 'course'
         ORDER BY created_at DESC, target_id ASC",
    )?;
    let course_rows = course_stmt.query_map(params![owner_id], |row| row.get::<_, String>(0))?;
    let mut course_ids = Vec::new();
    for id in course_rows {
        course_ids.push(id?);
    }

    let mut lesson_stmt = conn.prepare(
        "SELECT f.target_id FROM study_favorite f
         WHERE f.owner_id = ?1 AND f.target_kind = 'lesson'
           AND NOT EXISTS (SELECT 1
                             FROM media_removal r
                             JOIN media m
                               ON m.library_id = r.library_id
                              AND m.media_id = r.media_id
                            WHERE m.media_id = f.target_id)
         ORDER BY f.created_at DESC, f.target_id ASC",
    )?;
    let lesson_rows = lesson_stmt.query_map(params![owner_id], |row| row.get::<_, String>(0))?;
    let mut media_ids = Vec::new();
    for id in lesson_rows {
        media_ids.push(id?);
    }

    Ok(StudyFavorites {
        course_ids,
        media_ids,
    })
}

/// Marks or unmarks a course (`course_id`) as a favourite for the owner.
pub fn set_course_favorite(
    db: &Database,
    owner_id: &str,
    course_id: &str,
    favorite: bool,
) -> Result<bool> {
    validate_owner(owner_id)?;
    let target_id = validate_favorite_target(course_id)?;
    let conn = db.connection();

    if favorite {
        conn.execute(
            "INSERT INTO study_favorite (owner_id, target_kind, target_id, created_at)
             VALUES (?1, 'course', ?2, ?3)
             ON CONFLICT (owner_id, target_kind, target_id) DO NOTHING",
            params![owner_id, target_id, now_unix_seconds()],
        )?;
        Ok(true)
    } else {
        conn.execute(
            "DELETE FROM study_favorite WHERE owner_id = ?1 AND target_kind = 'course' AND target_id = ?2",
            params![owner_id, target_id],
        )?;
        Ok(false)
    }
}

/// Marks or unmarks a lesson (`media_id`) as a favourite for the owner.
pub fn set_lesson_favorite(
    db: &Database,
    owner_id: &str,
    media_id: &str,
    favorite: bool,
) -> Result<bool> {
    validate_owner(owner_id)?;
    let target_id = validate_favorite_target(media_id)?;
    let conn = db.connection();

    if favorite {
        require_media(conn, &target_id)?;
        conn.execute(
            "INSERT INTO study_favorite (owner_id, target_kind, target_id, created_at)
             VALUES (?1, 'lesson', ?2, ?3)
             ON CONFLICT (owner_id, target_kind, target_id) DO NOTHING",
            params![owner_id, target_id, now_unix_seconds()],
        )?;
        Ok(true)
    } else {
        conn.execute(
            "DELETE FROM study_favorite WHERE owner_id = ?1 AND target_kind = 'lesson' AND target_id = ?2",
            params![owner_id, target_id],
        )?;
        Ok(false)
    }
}
