//! Journal of the server's own writes and its retention (ADR 0005).
//!
//! A browser write is queued in `sync_outbox` as device [`SERVER_DEVICE_ID`], exactly like a
//! desktop write, and moved into `sync_operation` before the next push or pull. The journal
//! only exists for an owner who has a registered device, so an owner who never syncs costs
//! nothing, and the first registration backfills the state that was written before.

use rusqlite::{Connection, params};

use super::{SyncPayload, decode, encode};
use crate::db::{Database, now_unix_seconds};
use crate::error::{Error, Result};
use crate::study::validate_owner;

/// Device id under which the server journals its own (browser) writes. A real device cannot use it.
pub const SERVER_DEVICE_ID: &str = "server";

/// How long a tombstone, or an idle device that is not the owner's latest, is kept (180 days).
pub const DEFAULT_RETENTION_SECS: i64 = 180 * 24 * 60 * 60;

/// What a retention pass removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PruneOutcome {
    /// Deletion tombstones older than the window.
    pub tombstones: usize,
    /// Idle devices that were not the latest of their owner.
    pub devices: usize,
}

/// Key under which a newer operation replaces the older ones: one progress per media and one
/// preference per owner. Every other operation is a distinct version and is never replaced.
fn coalesce_key(payload: &SyncPayload) -> Option<String> {
    match payload {
        SyncPayload::Progress { media_id, .. } => Some(format!("p:{media_id}")),
        SyncPayload::Preference { .. } => Some("pref".to_owned()),
        _ => None,
    }
}

/// Records an operation in the journal and removes the operations it supersedes.
///
/// A progress that replaces older ones keeps the `completed` flag they carried (completion is
/// monotonic). An operation that is itself older than one already journaled is not recorded:
/// its effect is decided by last-write-wins when applied, and it would only add a row.
pub(super) fn insert_journal(
    conn: &Connection,
    owner_id: &str,
    device_id: &str,
    op_id: &str,
    client_at: i64,
    payload: &SyncPayload,
) -> Result<()> {
    let key = coalesce_key(payload);
    let mut payload = payload.clone();
    let mut superseded: Vec<i64> = Vec::new();
    if let Some(key) = &key {
        let newer: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sync_operation WHERE owner_id = ?1 AND coalesce_key = ?2 AND client_at > ?3)",
            params![owner_id, key, client_at],
            |row| row.get(0),
        )?;
        if newer {
            return Ok(());
        }
        let mut statement = conn.prepare(
            "SELECT seq, payload FROM sync_operation WHERE owner_id = ?1 AND coalesce_key = ?2",
        )?;
        let older = statement
            .query_map(params![owner_id, key], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        let mut completed = false;
        for (seq, old) in older {
            if let Ok(SyncPayload::Progress {
                completed: done, ..
            }) = decode(&old)
            {
                completed |= done;
            }
            superseded.push(seq);
        }
        if let SyncPayload::Progress {
            completed: flag, ..
        } = &mut payload
        {
            *flag |= completed;
        }
    }
    // The new row goes in BEFORE the old ones are removed. `seq` is a plain rowid: deleting the
    // highest row first would let the next insert reuse that number, and a device whose cursor
    // is already past it would never receive the replacement.
    conn.execute(
        "INSERT INTO sync_operation (op_id, owner_id, device_id, client_at, received_at, payload, coalesce_key)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![op_id, owner_id, device_id, client_at, now_unix_seconds(), encode(&payload)?, key],
    )?;
    for seq in superseded {
        conn.execute("DELETE FROM sync_operation WHERE seq = ?1", [seq])?;
    }
    Ok(())
}

/// Moves the owner's pending server operations from the outbox into the journal.
pub(super) fn drain_server_outbox(conn: &Connection, owner_id: &str) -> Result<usize> {
    let mut statement = conn.prepare(
        "SELECT op_id, client_at, payload FROM sync_outbox
          WHERE owner_id = ?1 AND device_id = ?2 ORDER BY queued_at, op_id",
    )?;
    let pending = statement
        .query_map(params![owner_id, SERVER_DEVICE_ID], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    for (op_id, client_at, payload) in &pending {
        insert_journal(
            conn,
            owner_id,
            SERVER_DEVICE_ID,
            op_id,
            *client_at,
            &decode(payload)?,
        )?;
        conn.execute("DELETE FROM sync_outbox WHERE op_id = ?1", [op_id])?;
    }
    Ok(pending.len())
}

/// Registers (or refreshes) a device of the owner and drains the server outbox. The first device of
/// an owner also backfills the existing state, so a device that connects later converges.
pub(super) fn register_device(
    conn: &Connection,
    owner_id: &str,
    device_id: &str,
    cursor: Option<i64>,
) -> Result<()> {
    if device_id == SERVER_DEVICE_ID {
        return Err(Error::InvalidSyncOperation {
            reason: "reserved device id",
        });
    }
    let known: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_device WHERE owner_id = ?1)",
        [owner_id],
        |row| row.get(0),
    )?;
    if !known {
        backfill(conn, owner_id)?;
    }
    conn.execute(
        "INSERT INTO sync_device (owner_id, device_id, pull_cursor, last_seen_at) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (owner_id, device_id) DO UPDATE SET
             pull_cursor = MAX(sync_device.pull_cursor, excluded.pull_cursor),
             last_seen_at = excluded.last_seen_at",
        params![owner_id, device_id, cursor.unwrap_or(0).max(0), now_unix_seconds()],
    )?;
    drain_server_outbox(conn, owner_id)?;
    Ok(())
}

/// Emits one operation for each existing row that is safe to repeat. It never repeats a note that
/// already carries a synced version (it would become a copy) nor a card review (it would
/// reschedule the card again); a card therefore reaches a new device without its schedule.
fn backfill(conn: &Connection, owner_id: &str) -> Result<()> {
    let at = |value: i64| value.clamp(0, now_unix_seconds());
    let mut ops: Vec<(String, i64, SyncPayload)> = Vec::new();
    let mut add = |client_at: i64, payload: SyncPayload| {
        ops.push((crate::ids::new_sync_id(), at(client_at), payload))
    };

    let mut statement = conn.prepare("SELECT media_id, position_seconds, completed, updated_at FROM study_state WHERE owner_id = ?1")?;
    for row in statement.query_map([owner_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, bool>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })? {
        let (media_id, position_seconds, completed, updated_at) = row?;
        add(
            updated_at,
            SyncPayload::Progress {
                media_id,
                position_seconds,
                completed,
            },
        );
    }
    let mut statement = conn.prepare("SELECT bookmark_id, media_id, position_seconds, label, created_at FROM media_bookmark WHERE owner_id = ?1")?;
    for row in statement.query_map([owner_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })? {
        let (bookmark_id, media_id, position_seconds, label, created_at) = row?;
        add(
            created_at,
            SyncPayload::Bookmark {
                bookmark_id,
                media_id,
                position_seconds,
                label,
                created_at,
            },
        );
    }
    let mut statement = conn.prepare("SELECT card_id, media_id, position_seconds, front, back, created_at, updated_at FROM flashcard WHERE owner_id = ?1")?;
    for row in statement.query_map([owner_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<i64>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })? {
        let (card_id, media_id, position_seconds, front, back, created_at, updated_at) = row?;
        add(
            updated_at,
            SyncPayload::FlashcardCreated {
                card_id,
                media_id,
                position_seconds,
                front,
                back,
                created_at,
            },
        );
    }
    let mut statement = conn.prepare("SELECT video_speed, audio_speed, subtitle_language, updated_at FROM player_preference WHERE owner_id = ?1")?;
    for row in statement.query_map([owner_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })? {
        let (video_speed, audio_speed, subtitle_language, updated_at) = row?;
        add(
            updated_at,
            SyncPayload::Preference {
                video_speed,
                audio_speed,
                subtitle_language,
            },
        );
    }
    // Notes written in a browser have no synced version yet: give each one its version now, so the
    // operation and the row agree and a later pull converges instead of making a copy.
    let mut statement = conn.prepare("SELECT note_id, media_id, position_seconds, content, created_at, updated_at FROM study_note WHERE owner_id = ?1 AND sync_version IS NULL")?;
    let notes = statement
        .query_map([owner_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for (note_id, media_id, position_seconds, content, created_at, updated_at) in notes {
        let op_id = crate::ids::new_sync_id();
        conn.execute(
            "UPDATE study_note SET sync_version = ?1 WHERE note_id = ?2",
            params![op_id, note_id],
        )?;
        ops.push((
            op_id,
            at(updated_at),
            SyncPayload::Note {
                note_id,
                media_id,
                position_seconds,
                content,
                created_at,
                updated_at,
            },
        ));
    }
    for (op_id, client_at, payload) in &ops {
        insert_journal(conn, owner_id, SERVER_DEVICE_ID, op_id, *client_at, payload)?;
    }
    Ok(())
}

/// True when the owner has a registered device, so the server journals the owner's writes.
pub fn journal_enabled(db: &Database, owner_id: &str) -> Result<bool> {
    validate_owner(owner_id)?;
    Ok(db.connection().query_row(
        "SELECT EXISTS(SELECT 1 FROM sync_device WHERE owner_id = ?1)",
        [owner_id],
        |row| row.get(0),
    )?)
}

/// Runs a browser write: queued in the journal as the server device when the owner syncs, plain
/// otherwise. `queued` receives the device id to pass to the matching `queue_*` function.
pub fn browser_write<T>(
    db: &Database,
    owner_id: &str,
    plain: impl FnOnce() -> Result<T>,
    queued: impl FnOnce(&str) -> Result<T>,
) -> Result<T> {
    if journal_enabled(db, owner_id)? {
        queued(SERVER_DEVICE_ID)
    } else {
        plain()
    }
}

/// Removes what is safe to remove: deletion tombstones and idle devices older than `retention_secs`
/// (an owner's latest device is always kept, so the owner stays journaled). Content operations are
/// kept on purpose: a device that connects later replays them to converge.
pub fn prune_sync(db: &Database, retention_secs: i64) -> Result<PruneOutcome> {
    let cutoff = now_unix_seconds() - retention_secs.max(0);
    let conn = db.connection();
    let tombstones = conn.execute("DELETE FROM sync_tombstone WHERE deleted_at < ?1", [cutoff])?;
    let devices = conn.execute(
        "DELETE FROM sync_device
          WHERE last_seen_at < ?1
            AND EXISTS (SELECT 1 FROM sync_device newer
                         WHERE newer.owner_id = sync_device.owner_id
                           AND (newer.last_seen_at > sync_device.last_seen_at
                                OR (newer.last_seen_at = sync_device.last_seen_at AND newer.device_id > sync_device.device_id)))",
        [cutoff],
    )?;
    Ok(PruneOutcome {
        tombstones,
        devices,
    })
}
