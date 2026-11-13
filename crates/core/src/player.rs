//! Per-user playback preferences.
//!
//! The player remembers the chosen speed and subtitle language across devices. The owner
//! is opaque, as in study state, and no operation crosses owners. Speeds are stored as
//! whole percentages of normal speed, so the database never stores a float.
//!
//! The desktop build has no accounts and keeps its preferences in the webview storage;
//! this module is used by the HTTP adapter.

use rusqlite::{OptionalExtension, params};

use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};
use crate::study::validate_owner;

/// Speed used when the owner has no saved preference.
pub const DEFAULT_PLAYBACK_SPEED_PERCENT: i64 = 100;
/// Fastest speed accepted by the domain (3×).
pub const MAX_PLAYBACK_SPEED_PERCENT: i64 = 300;
/// Slowest speed accepted by the domain (0.5×).
pub const MIN_PLAYBACK_SPEED_PERCENT: i64 = 50;
/// Longest accepted subtitle language tag (`pt-BR`, `zh-Hans`, `es-419`).
pub const MAX_SUBTITLE_LANGUAGE_CHARS: usize = 32;

/// Playback preferences of one owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerPreference {
    /// Video speed as a percentage of normal speed.
    pub video_speed: i64,
    /// Audio speed as a percentage of normal speed.
    pub audio_speed: i64,
    /// Last chosen subtitle language; empty means no preference.
    pub subtitle_language: String,
    /// Last write, in Unix seconds. Zero means the defaults were never saved.
    pub updated_at: i64,
}

impl Default for PlayerPreference {
    fn default() -> Self {
        Self {
            video_speed: DEFAULT_PLAYBACK_SPEED_PERCENT,
            audio_speed: DEFAULT_PLAYBACK_SPEED_PERCENT,
            subtitle_language: String::new(),
            updated_at: 0,
        }
    }
}

/// Reads the owner's preferences, falling back to neutral defaults.
pub fn player_preference(db: &Database, owner_id: &str) -> Result<PlayerPreference> {
    validate_owner(owner_id)?;
    let found = db
        .connection()
        .query_row(
            "SELECT video_speed, audio_speed, subtitle_language, updated_at
               FROM player_preference
              WHERE owner_id = ?1",
            [owner_id],
            |row| {
                Ok(PlayerPreference {
                    video_speed: row.get(0)?,
                    audio_speed: row.get(1)?,
                    subtitle_language: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()?;
    Ok(found.unwrap_or_default())
}

/// Saves every field for the owner and returns the stored values.
pub fn set_player_preference(
    db: &Database,
    owner_id: &str,
    video_speed: i64,
    audio_speed: i64,
    subtitle_language: &str,
) -> Result<PlayerPreference> {
    set_player_preference_on(
        db.connection(),
        owner_id,
        video_speed,
        audio_speed,
        subtitle_language,
    )
}

/// [`set_player_preference`] sobre uma conexão já aberta.
///
/// Lets the caller's transaction group the preference write with its sync operation.
pub(crate) fn set_player_preference_on(
    conn: &rusqlite::Connection,
    owner_id: &str,
    video_speed: i64,
    audio_speed: i64,
    subtitle_language: &str,
) -> Result<PlayerPreference> {
    validate_owner(owner_id)?;
    if !valid_speed(video_speed) || !valid_speed(audio_speed) || !valid_language(subtitle_language)
    {
        return Err(Error::InvalidPlayerPreference);
    }

    conn.query_row(
        "INSERT INTO player_preference
                 (owner_id, video_speed, audio_speed, subtitle_language, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (owner_id) DO UPDATE SET
                 video_speed = excluded.video_speed,
                 audio_speed = excluded.audio_speed,
                 subtitle_language = excluded.subtitle_language,
                 updated_at = excluded.updated_at
             RETURNING video_speed, audio_speed, subtitle_language, updated_at",
        params![
            owner_id,
            video_speed,
            audio_speed,
            subtitle_language,
            now_unix_seconds()
        ],
        |row| {
            Ok(PlayerPreference {
                video_speed: row.get(0)?,
                audio_speed: row.get(1)?,
                subtitle_language: row.get(2)?,
                updated_at: row.get(3)?,
            })
        },
    )
    .map_err(Error::from)
}

fn valid_speed(speed: i64) -> bool {
    (MIN_PLAYBACK_SPEED_PERCENT..=MAX_PLAYBACK_SPEED_PERCENT).contains(&speed)
}

/// A language tag is ASCII letters/digits with `-` or `_`; empty means no preference.
pub(crate) fn valid_language(language: &str) -> bool {
    language.chars().count() <= MAX_SUBTITLE_LANGUAGE_CHARS
        && language
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}
