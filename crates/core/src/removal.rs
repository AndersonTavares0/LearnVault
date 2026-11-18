//! Explicit removal of media from the catalog.
//!
//! The removal exists so the item leaves the catalog by the decision of whoever uses the product. It is
//! deliberate and persistent: no scan creates or undoes a removal, and finding the
//! file again at the same path does not return the media to the catalog.
//!
//! The removal applies to the media's current identity (`media_id`), not to the content of the
//! file. Moving or renaming the file makes the next scan create a new identity
//! — that is the approved rescan policy, in which the same path reuses the identity and a
//! new path generates another —, so the item reappears in the catalog as a
//! distinct record, without the earlier removal.
//!
//! # What the removal does and does not do
//!
//! - the catalog stops assembling the media (`crate::build_catalog` filters the removed ones);
//! - the media stops being delivered (`crate::open_media` returns [`Error::MediaRemoved`]);
//! - the recents stop listing it (`crate::recent_study` filters the removed ones);
//! - the `media` row, the progress in `study_state` and the physical file stay
//!   exactly as they were.
//!
//! The distinction between missing media and removed media is explicit: `availability` keeps
//! two states and the removal is a separate fact, stored in `media_removal`. A
//! media can be missing, removed, or both, without one condition
//! contaminating the other.
//!
//! # Confirmation
//!
//! Taking a media with associated progress out of the catalog requires explicit confirmation,
//! because the progress is preserved and the media leaves the view of whoever was studying. The refusal
//! returns [`Error::MediaRemovalNeedsConfirmation`] without changing anything.
//!
//! # Authorization
//!
//! The core does not authorize: it does not know who is calling. Deciding who may take a
//! media out of the catalog — and who may query the impact, which reveals how many owners have
//! progress — is the platform adapter's responsibility.
//!
//! # Consistency and concurrency
//!
//! All operations receive `library_id` and `media_id`; the pair is never trusted to the
//! caller. The insert selects the pair straight from `media`, so the stored `library_id`
//! is that of the media itself, and an inconsistent combination simply does not find
//! the row. The confirmation and the progress check happen in the same statement, inside
//! an `IMMEDIATE` transaction, so progress written by another process
//! cannot be removed without confirmation.

use rusqlite::{OptionalExtension, Transaction, TransactionBehavior};

use crate::catalog::{Availability, availability_of};
use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};

/// What removing a media affects, so the interface can decide whether to ask for confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRemovalImpact {
    /// Identificador consultado.
    pub media_id: String,
    /// Current relative path of the media, to confirm the right item.
    pub relative_path: String,
    /// Availability of the media in the last scan.
    pub availability: Availability,
    /// Number of distinct owners with progress on this media.
    pub progress_owners: usize,
    /// The media was already removed from the catalog.
    pub already_removed: bool,
}

/// Result of a removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRemovalOutcome {
    /// Identificador removido.
    pub media_id: String,
    /// The removal was recorded now; false when the media was already removed.
    pub removed_now: bool,
    /// Number of distinct owners whose progress was preserved.
    pub preserved_owners: usize,
}

/// Reports the effect of removing a media from the catalog.
///
/// It changes nothing and does not require confirmation: it is the data the interface uses to build the
/// question. The media must exist in the library, even if it is missing on disk.
pub fn media_removal_impact(
    db: &Database,
    library_id: &str,
    media_id: &str,
) -> Result<MediaRemovalImpact> {
    let impact = db
        .connection()
        .query_row(
            "SELECT m.relative_path,
                    m.availability,
                    (SELECT COUNT(DISTINCT s.owner_id)
                       FROM study_state s
                      WHERE s.media_id = m.media_id),
                    EXISTS (SELECT 1
                              FROM media_removal r
                             WHERE r.library_id = m.library_id
                               AND r.media_id = m.media_id)
               FROM media m
              WHERE m.library_id = ?1
                AND m.media_id = ?2",
            rusqlite::params![library_id, media_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()?;

    let Some((relative_path, availability, owners, removed)) = impact else {
        return Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        });
    };

    Ok(MediaRemovalImpact {
        media_id: media_id.to_owned(),
        relative_path,
        availability: availability_of(&availability),
        progress_owners: as_usize(owners),
        already_removed: removed != 0,
    })
}

/// Removes a media from the catalog, preserving the progress and the file.
///
/// When there is associated progress, the removal only happens with `confirm_progress`; without
/// it, it returns [`Error::MediaRemovalNeedsConfirmation`] and nothing is written. Repeating the
/// removal of a media already removed is harmless: it returns `removed_now` false, with no error.
pub fn remove_media(
    db: &mut Database,
    library_id: &str,
    media_id: &str,
    confirm_progress: bool,
) -> Result<MediaRemovalOutcome> {
    let tx = db
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)?;

    if !media_exists(&tx, library_id, media_id)? {
        return Err(Error::MediaNotFound {
            media_id: media_id.to_owned(),
        });
    }

    if removal_exists(&tx, library_id, media_id)? {
        let preserved_owners = owners_with_progress(&tx, media_id)?;
        tx.commit()?;
        return Ok(MediaRemovalOutcome {
            media_id: media_id.to_owned(),
            removed_now: false,
            preserved_owners,
        });
    }

    // The confirmation and the progress check live in the same statement: the stored pair comes
    // from the `media` row itself, and progress that appears before the commit prevents
    // the removal without confirmation.
    let inserted = tx.execute(
        "INSERT INTO media_removal (library_id, media_id, removed_at)
         SELECT m.library_id, m.media_id, ?3
           FROM media m
          WHERE m.library_id = ?1
            AND m.media_id = ?2
            AND (?4 = 1 OR NOT EXISTS (SELECT 1
                                         FROM study_state s
                                        WHERE s.media_id = m.media_id))
         ON CONFLICT (library_id, media_id) DO NOTHING",
        rusqlite::params![library_id, media_id, now_unix_seconds(), confirm_progress],
    )?;

    if inserted == 0 {
        let progress_owners = owners_with_progress(&tx, media_id)?;
        return Err(Error::MediaRemovalNeedsConfirmation {
            media_id: media_id.to_owned(),
            progress_owners,
        });
    }

    let preserved_owners = owners_with_progress(&tx, media_id)?;
    tx.commit()?;

    Ok(MediaRemovalOutcome {
        media_id: media_id.to_owned(),
        removed_now: true,
        preserved_owners,
    })
}

/// The media exists in the library, regardless of availability on disk.
fn media_exists(tx: &Transaction<'_>, library_id: &str, media_id: &str) -> rusqlite::Result<bool> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM media WHERE library_id = ?1 AND media_id = ?2",
            rusqlite::params![library_id, media_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// The media is already recorded as removed.
fn removal_exists(
    tx: &Transaction<'_>,
    library_id: &str,
    media_id: &str,
) -> rusqlite::Result<bool> {
    Ok(tx
        .query_row(
            "SELECT 1 FROM media_removal WHERE library_id = ?1 AND media_id = ?2",
            rusqlite::params![library_id, media_id],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Number of distinct owners with progress preserved on this media.
fn owners_with_progress(tx: &Transaction<'_>, media_id: &str) -> rusqlite::Result<usize> {
    let owners = tx.query_row(
        "SELECT COUNT(DISTINCT owner_id) FROM study_state WHERE media_id = ?1",
        rusqlite::params![media_id],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(as_usize(owners))
}

/// Converts a SQLite count, saturating instead of failing.
fn as_usize(value: i64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}
