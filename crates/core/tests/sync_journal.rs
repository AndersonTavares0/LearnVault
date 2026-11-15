//! Journal of the server's own (browser) writes and its retention (ADR 0005), with a server
//! database and a desktop database talking through push, pull and apply.

use learnvault_core::{
    Database, Error, SERVER_DEVICE_ID, SyncOperationInput, SyncPayload, apply_operations,
    browser_write, create_flashcard, create_media_bookmark, create_study_note, journal_enabled,
    list_flashcards, list_media_bookmarks, list_study_notes, player_preference, prune_sync,
    pull_operations, push_operations, queue_completed, queue_note_create, queue_note_update,
    queue_position, record_position, set_completed, set_player_preference, study_progress,
};

use learnvault_core::queue_preference;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const OWNER: &str = "ana";

fn database() -> Database {
    let db = Database::open_in_memory().unwrap();
    db.connection()
        .execute_batch(
            "INSERT INTO library (library_id, available) VALUES ('lib', 1);
             INSERT INTO media (media_id, library_id, relative_path, relative_path_key, size, mtime, availability)
             VALUES ('m1', 'lib', 'c/a.mp4', 'c/a.mp4', 1, 1, 'available'),
                    ('m2', 'lib', 'c/b.mp4', 'c/b.mp4', 1, 1, 'available');",
        )
        .unwrap();
    db
}

fn count(db: &Database, sql: &str) -> i64 {
    db.connection()
        .query_row(sql, [], |row| row.get(0))
        .unwrap()
}

/// A device that has pulled once is registered, so the server journals the owner's writes.
fn register(server: &Database, device: &str) {
    pull_operations(server, OWNER, device, 0, 500).unwrap();
}

/// Pulls everything new for `device` and applies it to `desktop`; returns the pulled operations.
fn sync(server: &Database, desktop: &Database, device: &str, cursor: &mut i64) -> Vec<SyncPayload> {
    let batch = pull_operations(server, OWNER, device, *cursor, 500).unwrap();
    apply_operations(desktop, OWNER, &batch.items, batch.cursor).unwrap();
    *cursor = batch.cursor;
    batch.items.into_iter().map(|item| item.payload).collect()
}

fn queued_position(server: &Database, media: &str, position: i64) {
    browser_write(
        server,
        OWNER,
        || record_position(server, OWNER, media, position),
        |device| queue_position(server, OWNER, device, media, position),
    )
    .unwrap();
}

#[test]
fn an_owner_without_a_device_writes_plainly_and_nothing_is_journaled() -> TestResult {
    let server = database();
    assert!(!journal_enabled(&server, OWNER)?);
    queued_position(&server, "m1", 30);
    assert_eq!(
        study_progress(&server, OWNER, "m1")?
            .unwrap()
            .position_seconds,
        30
    );
    assert_eq!(count(&server, "SELECT COUNT(*) FROM sync_operation"), 0);
    assert_eq!(count(&server, "SELECT COUNT(*) FROM sync_outbox"), 0);
    Ok(())
}

#[test]
fn a_browser_write_reaches_a_device_once_and_is_not_echoed() -> TestResult {
    let (server, desktop) = (database(), database());
    register(&server, "desktop");
    assert!(journal_enabled(&server, OWNER)?);
    queued_position(&server, "m1", 77);

    // Queued in the outbox, and only moved into the journal when somebody syncs.
    assert_eq!(count(&server, "SELECT COUNT(*) FROM sync_outbox"), 1);
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE device_id = 'server'"
        ),
        0
    );

    let mut cursor = 0;
    let first = sync(&server, &desktop, "desktop", &mut cursor);
    assert_eq!(
        first,
        [SyncPayload::Progress {
            media_id: "m1".into(),
            position_seconds: 77,
            completed: false
        }]
    );
    assert_eq!(
        study_progress(&desktop, OWNER, "m1")?
            .unwrap()
            .position_seconds,
        77
    );
    assert_eq!(count(&server, "SELECT COUNT(*) FROM sync_outbox"), 0);
    assert!(
        sync(&server, &desktop, "desktop", &mut cursor).is_empty(),
        "delivered once"
    );

    // What the desktop pushes is not echoed back to it.
    push_operations(
        &server,
        OWNER,
        "desktop",
        &[SyncOperationInput {
            op_id: "d-1".into(),
            client_at: 1_000,
            payload: SyncPayload::Progress {
                media_id: "m2".into(),
                position_seconds: 9,
                completed: false,
            },
        }],
    )?;
    assert!(sync(&server, &desktop, "desktop", &mut cursor).is_empty());
    Ok(())
}

#[test]
fn two_hundred_autosaves_leave_one_progress_operation_and_completion_survives() -> TestResult {
    let server = database();
    register(&server, "desktop");
    for second in 0..200 {
        queued_position(&server, "m1", second);
    }
    browser_write(
        &server,
        OWNER,
        || set_completed(&server, OWNER, "m1", true),
        |device| queue_completed(&server, OWNER, device, "m1", true),
    )?;
    for second in 200..260 {
        queued_position(&server, "m1", second);
    }
    pull_operations(&server, OWNER, "desktop", 0, 500)?;
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE coalesce_key = 'p:m1'"
        ),
        1
    );

    // More autosaves after a drain replace the journaled one instead of adding rows.
    queued_position(&server, "m1", 400);
    let batch = pull_operations(&server, OWNER, "other-device", 0, 500)?;
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE coalesce_key = 'p:m1'"
        ),
        1
    );
    assert_eq!(batch.items.len(), 1);
    let SyncPayload::Progress {
        position_seconds,
        completed,
        ..
    } = &batch.items[0].payload
    else {
        panic!("progress expected")
    };
    assert_eq!(
        (*position_seconds, *completed),
        (400, true),
        "latest position, and the completion is kept"
    );
    Ok(())
}

#[test]
fn a_replacing_progress_always_gets_a_cursor_past_the_one_it_replaced() -> TestResult {
    // `seq` is a plain rowid: if the highest row were deleted before the replacement was inserted, the
    // replacement would reuse its number and a device whose cursor is already there would skip it.
    let (server, desktop) = (database(), database());
    queued_position(&server, "m1", 10);
    let mut cursor = 0;
    let first = sync(&server, &desktop, "desktop", &mut cursor);
    assert_eq!(first.len(), 1);
    for position in [20, 30, 40] {
        queued_position(&server, "m1", position);
        let delivered = sync(&server, &desktop, "desktop", &mut cursor);
        assert_eq!(
            delivered,
            [SyncPayload::Progress {
                media_id: "m1".into(),
                position_seconds: position,
                completed: false
            }]
        );
    }
    assert_eq!(
        study_progress(&desktop, OWNER, "m1")?
            .unwrap()
            .position_seconds,
        40
    );
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE coalesce_key = 'p:m1'"
        ),
        1
    );
    Ok(())
}

#[test]
fn a_note_written_and_edited_in_a_browser_reaches_the_desktop_under_the_existing_version_rule()
-> TestResult {
    let (server, desktop) = (database(), database());
    register(&server, "desktop");
    let note = browser_write(
        &server,
        OWNER,
        || create_study_note(&server, OWNER, "m1", 5, "first"),
        |device| queue_note_create(&server, OWNER, device, "m1", 5, "first"),
    )?;
    let mut cursor = 0;
    sync(&server, &desktop, "desktop", &mut cursor);
    assert_eq!(
        list_study_notes(&desktop, OWNER, "m1")?
            .iter()
            .map(|n| n.content.clone())
            .collect::<Vec<_>>(),
        ["first"]
    );

    browser_write(
        &server,
        OWNER,
        || learnvault_core::update_study_note(&server, OWNER, &note.note_id, "edited"),
        |device| queue_note_update(&server, OWNER, device, &note.note_id, "edited"),
    )?;
    sync(&server, &desktop, "desktop", &mut cursor);
    // The same rule as for an edit made on a desktop: the newest version takes the note and the
    // earlier text is kept as a copy, so nothing is lost.
    let mut texts: Vec<_> = list_study_notes(&desktop, OWNER, "m1")?
        .into_iter()
        .map(|n| n.content)
        .collect();
    texts.sort();
    assert_eq!(texts, ["edited", "first"]);
    let main = list_study_notes(&desktop, OWNER, "m1")?
        .into_iter()
        .find(|n| n.note_id == note.note_id)
        .unwrap();
    assert_eq!(main.content, "edited");

    // Syncing again changes nothing.
    assert!(sync(&server, &desktop, "desktop", &mut cursor).is_empty());
    assert_eq!(list_study_notes(&desktop, OWNER, "m1")?.len(), 2);
    Ok(())
}

#[test]
fn the_first_device_receives_what_was_written_before_it_registered() -> TestResult {
    let (server, desktop) = (database(), database());
    record_position(&server, OWNER, "m1", 120)?;
    create_study_note(&server, OWNER, "m1", 3, "earlier note")?;
    create_media_bookmark(&server, OWNER, "m1", 10, "mark")?;
    create_flashcard(&server, OWNER, "m2", "front", "back", None)?;
    set_player_preference(&server, OWNER, 150, 100, "pt")?;
    assert_eq!(
        count(&server, "SELECT COUNT(*) FROM sync_operation"),
        0,
        "nothing journaled before a device exists"
    );

    let mut cursor = 0;
    let first = sync(&server, &desktop, "desktop", &mut cursor);
    assert_eq!(first.len(), 5, "{first:?}");
    assert_eq!(
        study_progress(&desktop, OWNER, "m1")?
            .unwrap()
            .position_seconds,
        120
    );
    assert_eq!(list_study_notes(&desktop, OWNER, "m1")?.len(), 1);
    assert_eq!(list_media_bookmarks(&desktop, OWNER, "m1")?.len(), 1);
    assert_eq!(list_flashcards(&desktop, OWNER, "m2")?.len(), 1);
    assert_eq!(player_preference(&desktop, OWNER)?.video_speed, 150);
    // The note now carries the version of its operation, so a later edit converges.
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM study_note WHERE sync_version IS NULL"
        ),
        0
    );

    // A second device registering later does not backfill again.
    let before = count(&server, "SELECT COUNT(*) FROM sync_operation");
    register(&server, "tablet");
    assert_eq!(
        count(&server, "SELECT COUNT(*) FROM sync_operation"),
        before
    );
    Ok(())
}

#[test]
fn the_backfill_does_not_repeat_a_note_that_already_has_a_synced_version() -> TestResult {
    let server = database();
    // A note that came from a device (it carries that operation's version) before any device registered here.
    apply_operations(
        &server,
        OWNER,
        &[learnvault_core::SyncOperation {
            seq: 1,
            op_id: "op-original".into(),
            device_id: "old".into(),
            client_at: 100,
            payload: SyncPayload::Note {
                note_id: "n1".into(),
                media_id: "m1".into(),
                position_seconds: 1,
                content: "from a device".into(),
                created_at: 100,
                updated_at: 100,
            },
        }],
        1,
    )?;
    create_study_note(&server, OWNER, "m1", 2, "browser only")?;

    let batch = pull_operations(&server, OWNER, "desktop", 0, 500)?;
    let notes: Vec<_> = batch
        .items
        .iter()
        .filter_map(|item| match &item.payload {
            SyncPayload::Note { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        notes,
        ["browser only"],
        "the synced note would become a copy if it were repeated"
    );
    Ok(())
}

#[test]
fn a_real_device_cannot_use_the_server_device_id() {
    let server = database();
    assert!(matches!(
        pull_operations(&server, OWNER, SERVER_DEVICE_ID, 0, 10),
        Err(Error::InvalidSyncOperation { .. })
    ));
    assert!(matches!(
        push_operations(&server, OWNER, SERVER_DEVICE_ID, &[]),
        Err(Error::InvalidSyncOperation { .. })
    ));
}

#[test]
fn pushed_progress_and_preferences_keep_one_operation_each_and_a_late_one_adds_nothing()
-> TestResult {
    let server = database();
    let progress = |position, completed| SyncPayload::Progress {
        media_id: "m1".into(),
        position_seconds: position,
        completed,
    };
    let op = |id: &str, at: i64, payload| SyncOperationInput {
        op_id: id.into(),
        client_at: at,
        payload,
    };
    push_operations(
        &server,
        OWNER,
        "a",
        &[
            op("p1", 100, progress(10, true)),
            op("p2", 200, progress(20, false)),
        ],
    )?;
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE coalesce_key = 'p:m1'"
        ),
        1
    );
    // The newer one says "not completed" but completion is monotonic, so the kept operation still says completed.
    let kept: String = server.connection().query_row(
        "SELECT payload FROM sync_operation WHERE coalesce_key = 'p:m1'",
        [],
        |row| row.get(0),
    )?;
    assert!(kept.contains("\"completed\":true"), "{kept}");
    // An older operation arriving late is not recorded: a newer one is already journaled.
    push_operations(&server, OWNER, "b", &[op("p0", 50, progress(5, false))])?;
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE coalesce_key = 'p:m1'"
        ),
        1
    );

    let pref = |speed| SyncPayload::Preference {
        video_speed: speed,
        audio_speed: 100,
        subtitle_language: String::new(),
    };
    push_operations(
        &server,
        OWNER,
        "a",
        &[
            op("f1", 300, pref(120)),
            op("f2", 310, pref(130)),
            op("f3", 320, pref(140)),
        ],
    )?;
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE coalesce_key = 'pref'"
        ),
        1
    );
    Ok(())
}

#[test]
fn retention_removes_old_tombstones_and_idle_devices_but_keeps_the_latest_device_and_content()
-> TestResult {
    let server = database();
    push_operations(
        &server,
        OWNER,
        "old-phone",
        &[SyncOperationInput {
            op_id: "n1".into(),
            client_at: 10,
            payload: SyncPayload::Note {
                note_id: "n".into(),
                media_id: "m1".into(),
                position_seconds: 0,
                content: "kept".into(),
                created_at: 10,
                updated_at: 10,
            },
        }],
    )?;
    pull_operations(&server, OWNER, "laptop", 0, 10)?;
    let conn = server.connection();
    conn.execute_batch(
        "UPDATE sync_device SET last_seen_at = 1000 WHERE device_id = 'old-phone';
         UPDATE sync_device SET last_seen_at = 2000 WHERE device_id = 'laptop';
         INSERT INTO sync_tombstone (owner_id, kind, target_id, deleted_at) VALUES ('ana', 'note', 'old', 1000), ('ana', 'note', 'recent', strftime('%s', 'now'));",
    )?;

    let pruned = prune_sync(&server, 100 * 24 * 3600)?;
    assert_eq!(
        (pruned.tombstones, pruned.devices),
        (1, 1),
        "the idle phone goes, the latest device stays even when it is idle too"
    );
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_tombstone WHERE target_id = 'recent'"
        ),
        1
    );
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_device WHERE device_id = 'laptop'"
        ),
        1
    );
    assert!(
        journal_enabled(&server, OWNER)?,
        "an owner who ever synced keeps being journaled"
    );
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE payload LIKE '%kept%'"
        ),
        1,
        "content operations are never pruned"
    );

    // A single device is never removed, however idle.
    let again = prune_sync(&server, 0)?;
    assert_eq!(again.devices, 0);
    assert_eq!(count(&server, "SELECT COUNT(*) FROM sync_device"), 1);
    Ok(())
}

fn progress_op(id: &str, at: i64, completed: bool) -> SyncOperationInput {
    SyncOperationInput {
        op_id: id.into(),
        client_at: at,
        payload: SyncPayload::Progress {
            media_id: "m1".into(),
            position_seconds: at,
            completed,
        },
    }
}

fn journaled_progress(db: &Database) -> (i64, String, bool) {
    let (count, device, payload): (i64, String, String) = db
        .connection()
        .query_row("SELECT COUNT(*), MAX(device_id), MAX(payload) FROM sync_operation WHERE coalesce_key = 'p:m1'", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap();
    (count, device, payload.contains("\"completed\":true"))
}

#[test]
fn a_late_completion_still_reaches_every_device() -> TestResult {
    let server = database();
    // Device B journals a newer, not completed progress; device A, offline until now, pushes an older completion.
    push_operations(&server, OWNER, "b", &[progress_op("b-1", 200, false)])?;
    push_operations(&server, OWNER, "a", &[progress_op("a-1", 100, true)])?;
    // Still a single row, now saying completed, and a device that never saw either gets it.
    assert_eq!(journaled_progress(&server), (1, "server".to_owned(), true));
    let batch = pull_operations(&server, OWNER, "c", 0, 500)?;
    assert_eq!(batch.items.len(), 1);
    assert!(matches!(
        batch.items[0].payload,
        SyncPayload::Progress {
            completed: true,
            position_seconds: 200,
            ..
        }
    ));
    Ok(())
}

#[test]
fn a_newer_progress_that_merges_a_completion_is_delivered_to_its_own_device_too() -> TestResult {
    let server = database();
    push_operations(&server, OWNER, "a", &[progress_op("a-1", 100, true)])?;
    let seen = pull_operations(&server, OWNER, "b", 0, 500)?.cursor;
    // B pushes a newer progress that does not say completed; the merged row says it, and B must learn it.
    push_operations(&server, OWNER, "b", &[progress_op("b-1", 200, false)])?;
    assert_eq!(journaled_progress(&server), (1, "server".to_owned(), true));
    let batch = pull_operations(&server, OWNER, "b", seen, 500)?;
    assert_eq!(
        batch.items.len(),
        1,
        "B does not get its own row, but gets the merged completion"
    );
    assert!(matches!(
        batch.items[0].payload,
        SyncPayload::Progress {
            completed: true,
            ..
        }
    ));
    Ok(())
}

#[test]
fn a_progress_without_a_completion_to_merge_keeps_the_device_that_wrote_it() -> TestResult {
    let server = database();
    push_operations(&server, OWNER, "a", &[progress_op("a-1", 100, false)])?;
    push_operations(&server, OWNER, "b", &[progress_op("b-1", 200, false)])?;
    assert_eq!(journaled_progress(&server), (1, "b".to_owned(), false));
    Ok(())
}

#[test]
fn a_client_that_invents_device_ids_cannot_grow_the_device_table() {
    let server = database();
    for index in 0..60 {
        pull_operations(&server, OWNER, &format!("device-{index:02}"), 0, 1).unwrap();
    }
    assert_eq!(count(&server, "SELECT COUNT(*) FROM sync_device"), 32);
    // The most recently seen ones are the ones kept.
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_device WHERE device_id = 'device-59'"
        ),
        1
    );
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_device WHERE device_id = 'device-00'"
        ),
        0
    );
}

#[test]
fn two_writes_in_the_same_second_leave_the_later_one_in_the_journal() -> TestResult {
    let server = database();
    register(&server, "desktop");
    for speed in [120, 150] {
        browser_write(
            &server,
            OWNER,
            || set_player_preference(&server, OWNER, speed, 100, ""),
            |device| queue_preference(&server, OWNER, device, speed, 100, ""),
        )?;
    }
    let batch = pull_operations(&server, OWNER, "desktop", 0, 500)?;
    let speeds: Vec<i64> = batch
        .items
        .iter()
        .filter_map(|item| match item.payload {
            SyncPayload::Preference { video_speed, .. } => Some(video_speed),
            _ => None,
        })
        .collect();
    assert_eq!(
        speeds,
        [150],
        "the journal ends on the last write, as the database does"
    );
    assert_eq!(player_preference(&server, OWNER)?.video_speed, 150);
    Ok(())
}

#[test]
fn owners_never_touch_each_others_journal() -> TestResult {
    let server = database();
    let other = |id: &str, at: i64| SyncOperationInput {
        op_id: id.into(),
        client_at: at,
        payload: SyncPayload::Progress {
            media_id: "m1".into(),
            position_seconds: at,
            completed: false,
        },
    };
    push_operations(&server, "bia", "pc", &[other("b-1", 100)])?;
    record_position(&server, OWNER, "m1", 5)?;
    // Registering OWNER's first device backfills only OWNER's rows; coalescing OWNER's progress leaves bia's alone.
    pull_operations(&server, OWNER, "laptop", 0, 500)?;
    push_operations(&server, OWNER, "laptop", &[other("a-2", 300)])?;
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE owner_id = 'bia' AND coalesce_key = 'p:m1'"
        ),
        1
    );
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_operation WHERE owner_id = 'ana' AND coalesce_key = 'p:m1'"
        ),
        1
    );
    assert_eq!(
        count(
            &server,
            "SELECT COUNT(*) FROM sync_device WHERE owner_id = 'bia'"
        ),
        1
    );
    Ok(())
}
