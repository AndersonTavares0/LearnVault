# ADR: Journal browser writes and bound the sync log

## Status

Accepted (2026-11-14). Refs #498.

## Context

Verified in the code (`crates/core/src/sync.rs`, `crates/server/src/study.rs`, migrations 17 and 18):

- The sync contract is an append-only operation journal (`sync_operation`, cursor `seq`). A device pushes the operations it queued in `sync_outbox` and pulls the operations of the owner's other devices after its cursor.
- The server applies what devices push, but its own study writes (`record_position`, `create_study_note`, `review_flashcard` and the other ten calls in `study.rs` and `player.rs`) never reach the journal. A browser edit is therefore invisible to a desktop, and a desktop only ever receives desktop-originated operations.
- The server keeps no per-device state: `sync_state` holds the local cursor of the desktop and is unused on the server, and a pull is stateless.
- Nothing prunes `sync_operation` or `sync_tombstone`, and a desktop that connects later pulls from cursor 0, so it also never sees history written in a browser before it connected.
- Notes are versioned by the `op_id` of the operation that wrote them (the highest `op_id` wins, older versions become copies), and a flashcard review reschedules the card when applied. Re-emitting an old note or review as a new operation would therefore duplicate notes or reschedule cards.

## Decision

1. **Reserved device.** The server journals its own writes as device `server`. A browser write calls the existing `queue_*` function with that device id, so the write and its operation are recorded in one transaction in `sync_outbox`, exactly like the desktop. No new write path is invented.
2. **Drain.** Before it serves a push or a pull, the server moves the owner's `server` outbox rows into `sync_operation`. They are not applied again (the effect already happened). A crash between the write and the drain loses nothing, because the outbox is durable.
3. **Journal only when someone consumes it.** A new table `sync_device(owner_id, device_id, pull_cursor, last_seen_at)` is updated by every push and pull. An owner with no registered device has plain writes and no journal growth. The first registration of a device for an owner runs a **backfill**, in the same transaction.
4. **Backfill.** It emits one operation per existing row that is safe to repeat: progress, bookmarks, flashcards (front, back and identity, not the schedule), player preferences, and notes whose `sync_version` is null (browser-created; they receive the new version so a later pull converges). It never emits notes that already carry a synced version, nor card reviews, because repeating them would duplicate notes or reschedule cards. Known limit: the review schedule of an existing flashcard does not reach a new device.
5. **Bounded growth by coalescing.** Whenever a progress or a preference operation enters the journal (pushed or drained), older journal operations of the same owner and key that it supersedes are deleted, and the `completed` flag stays monotonic across the deleted ones. The journal then holds at most one progress operation per owner and media and one preference per owner, instead of one per 5-second autosave.
6. **Conservative retention.** Content operations (notes, bookmarks, flashcards, reviews, deletions) are kept: each one is a user action, and removing it would break the convergence of a device that connects later. Retention removes what is safe: `sync_tombstone` rows older than the window (default 180 days), and `sync_device` rows not seen within the window. A device offline longer than the window can resurrect a note, bookmark or card that it created while offline before a deletion made elsewhere; this trade-off is the one tombstones always had.
7. **Contract unchanged.** `POST /api/sync/push`, `GET /api/sync/pull` and the desktop outbox keep their semantics; a pull never returns a device's own operations, so the browser's operations are delivered to desktops and not echoed.

## Consequences

- Browser progress, notes, bookmarks, flashcards and preferences reach a desktop after it pulls, which closes the one-way gap.
- A user who never connects a desktop pays nothing.
- The log no longer scales with autosaves. It still grows with human actions (notes, reviews, deletions); that is deliberate, and the ADR does not claim a hard cap.
- Rejected: pruning operations older than the cursor of every device (a new device would lose notes and card state, since it replays the journal from 0 and there is no snapshot endpoint); a per-user snapshot (a new endpoint and a second convergence path to keep in step with the journal); journaling every owner from the start (growth for users who never sync).

## Validation and rollback

- Core tests with two databases (server and desktop): a browser write reaches a device once and is not echoed; 200 autosaves leave one progress operation; completion survives coalescing; backfill converges without duplicating a synced note; the first device registration and the owner with no device; tombstone and device retention; the outbox survives a crash before the drain.
- Server HTTP tests: a study write followed by a pull returns the operation for another device and none for the writing device.
- Independent review of persistence and sync semantics before merge.
- Rollback: the migration is additive (`sync_device`). Without it the server writes plainly and the journal stops growing.
