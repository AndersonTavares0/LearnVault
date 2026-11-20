# API Reference

The Rust server (`crates/server`) exposes the HTTP endpoints below. The React SPA uses the same study and catalog contract over HTTP and over desktop IPC. Endpoints consume and produce JSON unless stated otherwise. Authentication uses an HttpOnly session cookie with `SameSite=Strict`.

The course registry is stored in `courses.json` next to the database. Catalog responses never include the absolute path of a course root: courses have opaque IDs, and files are listed by relative path with their metadata. Rescans revalidate each root and reject detected tampering.

The session cookie has the `Secure` flag by default and travels only over HTTPS. For local development without TLS, pass `--insecure-http`; never use it in production. Guarded write endpoints require an HTTP(S) `Origin` that matches `Host` or, when `Origin` is absent, an HTTP(S) `Referer` whose origin matches. A failed check returns `403 csrf_failed`.

## SPA Delivery in Rust Server

Static serving is off by default. Pass `--web-root` with the web build directory (`crates/server/web/build`), which contains the `200.html` shell. Existing files are served directly, extensionless paths fall back to `200.html`, missing files with an extension return 404, and `/api/*` never falls back to HTML. A missing shell returns 503, and a file or shell larger than 16 MiB returns 413. The shell is served with `Cache-Control: no-cache`, assets under `_app/immutable/` are cached as immutable, and SPA error responses use `no-store`. Errors never reveal local filesystem paths.

## Health & Authentication Endpoints

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/healthz` | Public | Database readiness and running version: `{"status":"ok","version":"..."}` (`dev` on source builds) |
| `POST` | `/api/login` | Public (same origin) | Authenticates `{ "username": "...", "password": "..." }`; returns `{ user_id, username, is_admin, must_change_credentials }` and sets the session cookie. Throttled attempts return `429` |
| `POST` | `/api/logout` | Public (same origin) | Clears the cookie and returns an empty `204`; revokes the session when one is present |
| `GET` | `/api/me` | Session | Returns `{ user_id, username, is_admin, must_change_credentials }`; `401` without a valid session |
| `POST` | `/api/account/credentials` | Session (same origin) | Accepts `{ "current_password": "...", "username": "...", "password": "..." }`, updates credentials, revokes older sessions, and returns the account with a new session cookie; `422 invalid_credentials_format` for a rejected value |

Readiness uses a separate read-only SQLite connection that waits at most 500 ms on a lock, so it does not queue behind the shared database connection that scans and writes hold. A missing, unreadable or incompatible database returns `503 {"status":"unavailable"}`, and so does a server with no spare capacity for blocking work. Responses carry `Cache-Control: no-store`.

When the database has no administrator, first start may create temporary credentials (`admin/admin`), and only when the server listens on a loopback address. The account then has `must_change_credentials: true`, and every protected endpoint except `GET /api/me` and `POST /api/account/credentials` returns `403 credentials_change_required` until the password changes. Creating the first administrator on a non-loopback listener requires explicit bootstrap credentials; an existing eligible administrator starts without them, but one still marked for a credential change blocks network startup.

Login attempts are limited to 5 per 60 seconds per client address. The address is the socket peer or, when that peer is listed in `TRUSTED_PROXY_IPS`, the first untrusted address in `X-Forwarded-For` read right to left. `X-Real-IP` is ignored. Proxy setup is in [Distribution & Operations](../operations/distribution.md#reverse-proxy-and-login-rate-limiting).

## Database Compatibility & Migration

Before bootstrapping or running migrations, the server checks for legacy Go database signatures (`users`, `sessions`, `schema_migrations`). If it finds them, it exits with `legacy_go_database_requires_migration: back up the database and migrate it before using the Rust server`. It does not create `admin/admin` or overwrite data. `learnvault-server migrate-go` converts a legacy database and takes a backup first (see [the migration guide](../operations/migration-go-to-rust.md)).

## Catalog, Courses & Media

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/catalog` | Session | Returns top-level course nodes (`role: course` with opaque `course_id` and `title`), with modules, lessons, and materials. Carries a content `ETag` and `Cache-Control: private, no-cache`; a request with a matching `If-None-Match` gets `304 Not Modified` with an empty body |
| `POST` | `/api/admin/courses` | Admin Session | Accepts `{ "path": "..." }` to register a course folder. Path must exist, be readable, and not be a symlink |
| `DELETE` | `/api/admin/courses/{id}` | Admin Session | Unregisters a course folder without deleting course files or study progress |
| `POST` | `/api/admin/catalog/rescan` | Admin Session | Explicitly rescans all registered courses in a preserving manner: registers new files, confirms known ones, marks missing ones without deleting records |
| `GET` | `/api/indexing` | Session | Returns the background indexing job: `status` (`idle`/`running`/`done`/`cancelled`/`error`), `started_at`, `finished_at`, `progress` (`files_seen`, `added`, `changed`, `removed`, `moved`), course counters, and a stable `last_error` code |
| `POST` | `/api/indexing/cancel` | Admin Session | Cancels the running indexing job. Returns `{ "cancelled": true }`, or `409` when no job is active |
| `PUT` | `/api/admin/courses/{id}/cover` | Admin Session | Saves custom course cover (raw image body up to 4 MiB; PNG/JPEG/WebP). Returns `{ "cover_version": <timestamp> }` |
| `DELETE` | `/api/admin/courses/{id}/cover` | Admin Session | Removes custom cover image atomically without modifying course folder. Returns `{ "removed": bool }` |
| `GET` | `/api/courses/{id}/cover` | Session | Serves custom course cover with appropriate `Content-Type` and `Cache-Control: private, no-store` |
| `GET`, `HEAD` | `/api/media?course=...&path=...` | Session | Streams approved media file with HTTP Range support (`200`/`206`/`416`) |
| `GET` | `/api/admin/media/{id}/impact` | Admin Session | Queries impact of removing media: `{ media_id, relative_path, availability, progress_owners, already_removed }` |
| `DELETE` | `/api/admin/media/{id}` | Admin Session | Explicitly removes media from catalog (permanent tombstone). Requires `{ "confirm_progress": true }` if progress exists |

### Catalog Organization

A module directory that contains videos becomes a collapsible module with one lesson per video. Other files in it stay available as materials, and matching subtitles stay with the lesson video. Course paths use natural sorting (`2` before `10`).

With no registered courses, the catalog is empty. `LIBRARY_DIR` / `--library-root` seeds a new deployment once, when no registry exists. Each top-level folder becomes a course, except that a folder containing 2 to 100 nested courses is split into those courses when they register. If no folder registers and a media or material file sits directly in the root, the root becomes one course with the historical ID `library-1`; otherwise an empty root registers nothing. Administrators import folders added later.

The scanner reads directory metadata and does not follow symbolic links. It skips temporary download directories, detects media, subtitles and documents, and treats a `.ts` file as video only if it is at least 1 MiB, so TypeScript files are not mistaken for video.

### Automatic Indexing

The server indexes registered courses in the background, outside the request path. A filesystem watcher debounces bursts of events, and a reconciliation every five minutes covers network mounts where events are unreliable. One scan runs at a time. The background job waits for a running manual rescan, and a manual rescan that overlaps a running scan returns `409`.

Each course is scanned without holding the database and applied in its own transaction. The database lock and the heavy-work permit are released between courses, so API requests are not blocked for the length of the library scan. `GET /api/indexing` exposes the job state, file counts, course progress, timestamps and a stable error code. Cancelling stops before the next course and keeps the courses already committed.

Detection is incremental: a known path is compared by size and mtime, and a sampled SHA-256 (both ends plus the length) separates a real content change from a mere timestamp touch. A file that appears at an unknown path matches a missing row by content hash (or by size and mtime when the hash is unavailable) and keeps its `media_id`, notes, bookmarks, and progress; ambiguous matches are refused instead of guessed. Files that disappear are marked `availability: missing`; no scan deletes rows, notes, bookmarks, or progress history. The read-only guarantee is unchanged: the scanner and the hash only read course files.

The desktop app has no background indexer; it keeps the explicit rescan command.

In the desktop app, `learnvault-media://localhost/<media-id>` serves media and subtitle files by opaque ID. `GET` and `HEAD` support single ranges. On Windows the SPA uses `http://learnvault-media.localhost/<media-id>`.

### Custom Covers

The application stores a custom cover itself: on the server in `covers/<id>.cover` next to `courses.json`, on desktop in the application data directory. Course folders stay read-only. PNG, JPEG and WebP are accepted, checked by magic bytes, and written atomically through a temporary file and `rename`.

Top-level course nodes carry `cover_version` when a cover exists, and the SPA appends it as `?v=` to bust its cache. A missing cover or unknown course ID returns `404` with an empty body.

## Remote Course Sources (WebDAV and SMB)

A remote source is a read-only course tree served by WebDAV or SMB. Administrators configure it
with `dav://`, `davs://`, `http://`, `https://` or `smb://`; `dav` maps to `http` and `davs` to
`https`. URLs with embedded credentials, a query string or a fragment are rejected, and every
source URL is limited to 2048 characters. A WebDAV base path always ends in `/`. TLS certificates
are always verified; there is no option to accept an invalid certificate. Redirects are refused so
the `Authorization` header never reaches another host. `dav://` and `http://` send Basic
credentials without transport encryption, so use them only on a trusted private network; prefer
`davs://`/`https://`. Link-local, multicast, broadcast and unspecified addresses
(`169.254.0.0/16`, the IPv4 and IPv6 link-local ranges, `224.0.0.0/4`, `0.0.0.0/8`) are not allowed,
so a source cannot point at a cloud metadata endpoint. A URL whose host is such a literal address is
rejected with `unsafe_source_target`. A hostname is resolved before every connection, and if any
returned address is disallowed the operation fails with the sanitized `denied` code. Loopback and private LAN
addresses stay usable, because a source on the same network is the normal deployment. WebDAV
connections go directly to the validated address, and the server ignores `HTTP_PROXY`,
`HTTPS_PROXY` and `ALL_PROXY` for these requests, so a proxy cannot resolve the hostname outside
the address policy.

SMB sources use `smb://host[:port]/share/path`. The share is mandatory; `.`/`..` segments, encoded
separators and control characters are rejected before they become a UNC path. The server is
contacted on TCP/445 unless the URL names a port, with NTLM authentication and SMB signing support, and SMB3 encryption is used
when the server requires it. Kerberos is not supported in this build. DFS referrals are disabled so
credentials stay on the configured server instead of following a referral to another host.

Discovery uses `PROPFIND` with `Depth: 1` over WebDAV and directory enumeration over SMB; the tree
is walked through the read-only `CourseSource` abstraction (the local filesystem has a reference
implementation parity-tested against the local scanner), with hidden and temporary directories,
symlinks and the media classification rules identical to the local scanner. Media bytes are read
with a single `Range` over WebDAV (a server that ignores `Range` is rejected instead of streaming
the wrong span) or ranged SMB reads. Only `PROPFIND`, `GET` and read-only SMB operations are ever
issued: the remote server is never written, renamed or deleted.

A `CourseSource` walk descends at most 32 directory levels (deeper folders count as inaccessible)
and rejects a scan of more than 200,000 accepted files. A `read_range` call returns at most 8 MiB.
A single WebDAV or SMB directory listing is limited to 100,000 entries. Registered local folders use
the local scanner instead. A connection attempt times out after 10 seconds and each listing or read
block after 30 seconds; the whole remote walk is limited to 120 seconds and a connection test to
20 seconds.

Configuration lives in `sources.json` next to the database, next to `courses.json`. Passwords are
encrypted at rest with ChaCha20-Poly1305 under a 32-byte key in `sources.key` (`0600`), created on
the first save. The API never returns the password or the ciphertext, only `has_password`. A
missing or replaced key leaves the source listed with `credential_error: true`; the original
ciphertext is preserved until the key is restored.

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/admin/sources` | Admin Session | Returns `{ sources: [{ id, name, url, username, has_password, credential_error }] }`; never returns the secret |
| `POST` | `/api/admin/sources` | Admin Session | Accepts `{ name, url, username?, password? }` and returns `201` with the sanitized source |
| `DELETE` | `/api/admin/sources/{id}` | Admin Session | Removes the source configuration and detaches its roots (their media returns to the source library); returns `{ removed: bool }` and touches no remote file |
| `POST` | `/api/admin/sources/{id}/test` | Admin Session | Validates reachability and authentication with a root listing (`PROPFIND` or SMB directory enumeration); returns `404` (empty) for an unknown source, and otherwise HTTP `200` with `{ ok: true, entries }` or `{ ok: false, error }` for a failed probe too (`unreachable`, `denied`, `not_found`, `invalid_response`, `limit_exceeded`, `credentials_unavailable`, `invalid_source`, `timeout`). Needs the same origin |
| `POST` | `/api/admin/sources/{id}/scan` | Admin Session | Walks the remote tree and applies it to the catalog as a course; returns the same rescan counters as the local rescan |
| `GET` | `/api/admin/sources/{id}/browse?path=` | Admin Session | Lists one folder level of the source (`path` is relative, empty for the source root): `{ path, folders: [{ name, path, state }], media_files, other_files, truncated }`. `state` is `root`, `inside_root`, `contains_root` or `free`. Ignored folders are hidden and at most 2000 folders are listed (`truncated`). A path is relative, `\` is read as `/`, and an unclean one (`..`, absolute, `//`, a drive prefix, NUL, over 1024 characters) returns `400 invalid_path`; a missing folder returns `404 not_found`, unreadable credentials `400 credentials_unavailable`, a source that cannot build a client `400 invalid_source`, and a remote failure `502` or `504` with the same sanitized codes as the connection test |
| `GET` | `/api/admin/sources/{id}/roots` | Admin Session | Lists the folders chosen as courses: `{ roots: [{ id, name, path }] }` |
| `POST` | `/api/admin/sources/{id}/roots` | Admin Session | Accepts `{ path, mode }` with `mode` `course` (the folder is a course) or `children` (every folder directly inside is a course). The folder must exist on the source. `course` on an ignored folder (such as `temp`) or on an empty path returns `400 invalid_path` (use `children` at the source base to create roots below it); more than 100 folders in `children` return `400 too_many_folders`. Returns `201 { created, skipped }` where `skipped` lists `{ path, reason }` (`overlaps_existing_root`, `invalid_path`, `invalid_root` or `unavailable` when a database failure stopped the rest); a single `course` that cannot be created returns `409` with the reason. Changing requests need the same origin |
| `DELETE` | `/api/admin/sources/{id}/roots/{root_id}` | Admin Session | The folder stops being a course of its own and its media returns to the source course; returns `{ removed: true }`, `404` when the root does not belong to that source, and touches no remote file |

A scanned source appears in `GET /api/catalog` as a top-level `course` node with the source's
opaque `id`; `GET`/`HEAD /api/media?course=<source-id>&path=<relative-path>` streams its bytes with
`206` ranges. Only paths already present in the catalog are proxied to the remote server. Remote
courses have no custom cover, manual ordering, search indexing or on-demand transcoding yet.

`POST /api/admin/catalog/rescan` rescans only the local courses (and answers `400` when none is registered).
The library's "Refresh courses" button therefore also calls `POST /api/admin/sources/{id}/scan` for each
source whose password can be read, one request at a time, and treats `400` as fine when remote sources
exist, so a server with only remote sources can still be refreshed from the library page.

The server also reads remote sources by itself: the first round starts 30 seconds after startup, sources are scanned one after another with a 5 second pause after each attempt, and the server sleeps an hour after each round. A source whose credentials cannot be read, or that is skipped because a scan holds the shared lock, waits for the next round. A walk lists up to 8 folders at once and is limited to 120 seconds; the HTTP timeout of both scan endpoints is 130 seconds.

A source that is a whole library (categories, sub-categories, then courses) is split into one course per
course folder by its scan (`course_detect`, ADR 0004). The detector looks at the folders that contain media
and at the numbering of folder names. A folder holds courses, instead of being one, when it has no media of
its own, at least two levels of media-bearing folders below it, and either fewer than half of its media-bearing
subfolders are named like modules ("01 Intro", "Module 15", "Lesson 41") or one of them already holds courses.
Files under a root the administrator chose are left alone. Nothing is promoted when the scan could not read
some folders or when more than 100 courses would be found. A folder removed with `DELETE .../roots/{root_id}`
is remembered (`library_root_dismissed`) and is not found again; choosing it again forgets that. A source that
is one course, or has media at its top level, is not split.

A source can be split into several courses by choosing folders as roots (ADR 0004). Choosing a folder
moves its already scanned media to a course of its own and every later scan of the source is applied
per root, so progress, notes and flashcards follow each lesson. In `GET /api/catalog` each root is a
top-level `course` node with its own opaque `id`. Every course that lives on a remote source (a root or
the source itself) carries `remote: true`; local courses have no such key, and the SPA does not offer
them the local-only actions (cover, lesson order, removal from the registry). The tree of a root starts at its own folder, so
the folders above it never appear, while every file keeps its path relative to the source (the path
media requests use). The title is the folder name, with as many parent folders in parentheses as it
takes to tell roots of a source apart (`Redes (Pentest)`), counted only among the roots the user can
see. The rest of the source stays a course with the source's `id` only while it still holds a file,
and an unreadable area outside every root still makes `complete` false. Per-user access is decided
by the id of each course: a user who had the source when a root was chosen receives that root, one
granted only a root never sees the rest of the source, and a user granted the source later must be
granted each root by its own id. `GET`/`HEAD /api/media?course=<root-id>&path=` streams
through the owning source and only finds files inside the root's folder. A scan applies each chosen
folder and the rest of the source as separate libraries, each in its own transaction: if one fails
the others are still applied and the request reports the failure, and the next scan converges.

## Study Progress, Focus & Gamification

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/study/capabilities` | Session | Returns supported study capabilities |
| `GET` | `/api/study/recent` | Session | Returns recently studied lessons |
| `GET` | `/api/study/progress?media_id=<id>` | Session | Returns `{ media_id, position_seconds, completed, updated_at, recorded }` for one lesson (`recorded: false` and a null `updated_at` when none exists); `404 media_not_found` for an unknown or inaccessible lesson |
| `POST` | `/api/study/position` | Session | Records the resume position `{ "media_id", "position_seconds" }`; completion is preserved |
| `POST` | `/api/study/completed` | Session | Sets completion `{ "media_id", "completed" }` |
| `GET` | `/api/study/summary` | Session | Streaks and activity: `{ current_streak, longest_streak, today, today_active, at_risk, week_active_days, completed_lessons, days: [{ day, active }], active_days_year }` |
| `GET` | `/api/focus/sessions` | Session | Lists the user's focus sessions |
| `POST` | `/api/focus/sessions` | Session | Records a focus session `{ id, kind, planned_seconds, actual_seconds, completed, started_at, ended_at }` |
| `GET` | `/api/focus/summary` | Session | Focus totals, completion rate, best day, and per-day minutes |
| `GET` | `/api/rewards/summary` | Session | XP, completed lessons, weekly goal, and daily missions |
| `POST` | `/api/rewards/missions/{mission_id}/claim` | Session | Claims a daily mission reward; returns `{ "claimed": <bool> }` |
| `GET`, `PUT` | `/api/rewards/weekly-goal` | Session | Reads the weekly goal, or sets it with `{ "weekly_goal": <n> }` |
| `GET` | `/api/study/favorites` | Session | Lists favorite courses and lessons |
| `POST` | `/api/study/favorites/course` | Session | Sets or clears a course favorite `{ "course_id", "favorite" }` |
| `POST` | `/api/study/favorites/lesson` | Session | Sets or clears a lesson favorite `{ "media_id", "favorite" }` |
| `GET`, `PUT` | `/api/player/preferences` | Session | Reads or writes the user's playback preferences |

## Notes & Flashcards

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/study/notes` | Session | Lists the user's notes; `?media_id=` filters one lesson |
| `POST` | `/api/study/notes` | Session | Creates a timestamped note `{ media_id, position_seconds, content }` |
| `PUT` | `/api/study/notes/{id}` | Session | Updates note text `{ content }` |
| `DELETE` | `/api/study/notes/{id}` | Session | Deletes a note |
| `GET`, `POST` | `/api/study/bookmarks` | Session | Lists lesson bookmarks (`?media_id=`) or creates one `{ media_id, position_seconds, label }` |
| `DELETE` | `/api/study/bookmarks/{id}` | Session | Deletes a bookmark |
| `GET`, `POST` | `/api/study/pdf-annotations` | Session | Lists (`?media_id=`) or creates PDF highlights, underlines and notes `{ media_id, page, kind, color, rects, quote?, ... }` |
| `PUT`, `DELETE` | `/api/study/pdf-annotations/{id}` | Session | Changes the color or note of an annotation, or deletes it |
| `GET` | `/api/study/flashcards` | Session | Lists the user's flashcards; `?media_id=` filters one lesson |
| `POST` | `/api/study/flashcards` | Session | Creates a spaced repetition flashcard `{ media_id, front, back, position_seconds? }` |
| `PUT` | `/api/study/flashcards/{id}` | Session | Edits `{ front, back }`; the review schedule is kept |
| `GET` | `/api/study/flashcards/due` | Session | Returns `{ "due": <n> }` for the session user's cards due now |
| `GET` | `/api/study/flashcards/{id}/reviews?limit=<n>` | Session | Returns the card's recent reviews, newest first |
| `POST` | `/api/study/flashcards/{id}/review` | Session | Records flashcard review grade and calculates next interval |
| `DELETE` | `/api/study/flashcards/{id}` | Session | Deletes a flashcard |

A card may carry `position_seconds` (the lesson moment it came from), returned on every card, and stays linked to its lesson through `media_id`. Each review appends a history row with the rating, the previous interval and the next due date, in the same transaction that updates the schedule.

The history endpoint is owner-scoped and read-only. `limit` defaults to 20 and is capped at `MAX_FLASHCARD_REVIEWS` (100). The response is `{ "items": [{ "review_id", "card_id", "rating", "previous_interval_days", "next_due", "reviewed_at" }], "total": <n> }`, where `total` counts the whole append-only log. The due count uses the same predicate as the review queue (`next_due <= now`) and only counts lessons the session user may still open; cards that were never reviewed are included.

## Offline Sync

The desktop keeps an offline queue and reconciles it with the server through an
append-only operation log. The owner is the authenticated session user; no endpoint accepts
an `owner_id`. Media files are never synchronized: only progress, notes, bookmarks,
flashcards (with their reviews) and playback preferences travel.

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `POST` | `/api/sync/push` | Session | Accepts `{ "device_id": "...", "operations": [...] }` and applies each new operation, returning `{ applied, duplicated, cursor }` |
| `GET` | `/api/sync/pull?device=<id>&cursor=<seq>&limit=<n>` | Session | Returns `{ cursor, has_more, items }` (`limit` defaults to 100 and is clamped to 1-500; each item has `seq`, `op_id`, `device_id`, `client_at`, `payload`) with the operations after `cursor`, excluding the requesting device and, for non-admin users, operations on media of courses the user can no longer open (the cursor still advances past them) |

Writes made in a browser reach the journal too (ADR 0005). Once an owner has registered a device (any push
or pull registers it), the server records the owner's own study writes as operations of the reserved device
`server`, so a pull delivers them to the desktop; `server` is refused as a `device_id`. The first
registration also backfills what was written before (progress, bookmarks, flashcards without their review
schedule, preferences and notes that have no synced version yet). A newer `progress` replaces the older
ones of the same media and a newer `preference` the older ones, with `completed` staying set, so autosaves do
not grow the log; notes, bookmarks, flashcards, reviews and deletions are kept. Deletion tombstones and
idle devices older than 180 days are pruned on a best-effort basis, at most once per process per day (the owner's latest device is kept), and registering a device beyond 32 per owner forgets the least recently seen ones.

Each operation is `{ "op_id": "...", "client_at": <unix-seconds>, "payload": { "kind": ... } }`
with one of the payload kinds below:

| Kind | Fields |
| --- | --- |
| `progress` | `media_id`, `position_seconds`, `completed` |
| `note` | `note_id`, `media_id`, `position_seconds`, `content`, `created_at`, `updated_at` |
| `bookmark` | `bookmark_id`, `media_id`, `position_seconds`, `label`, `created_at` |
| `note_deleted` | `note_id` |
| `bookmark_deleted` | `bookmark_id` |
| `flashcard_created` | `card_id`, `media_id`, `position_seconds` (nullable), `front`, `back`, `created_at` |
| `flashcard_updated` | `card_id`, `front`, `back`, `updated_at` |
| `flashcard_deleted` | `card_id` |
| `flashcard_review` | `card_id`, `rating` (`again`/`good`), `reviewed_at` |
| `preference` | `video_speed`, `audio_speed` (percent of normal), `subtitle_language` |

`op_id` is stable and globally unique: replaying a pushed operation is a duplicate and does
not repeat its effect. `seq` is the monotonic pull cursor, so a
device resumes from the last cursor it applied. A newer `progress` or `preference` operation replaces the older rows it supersedes under a new `seq`, inserted before the old rows are deleted, so incremental cursors keep working. Pulling excludes operations produced by the
requesting `device_id`, because that device already applied them locally.

Conflict rules:

- **progress**: last write wins by `client_at`, with the position clamped to
  `[0, 86400]` seconds; completion is monotonic: no incoming sync operation clears it, even a newer one;
- **notes**: each synchronized version is identified by its `op_id`. The largest `op_id`
  (UUID v7, so the latest version) owns the note row and every other version is kept as a
  conflict copy with the deterministic id `<note_id>~<op_id>`. No content is silently lost,
  and devices that saw the same versions converge on the same texts. Notes created before
  schema version 17 keep their row and never overwrite it;
- **flashcards**: last write wins by `updated_at`, where a create counts as its
  `created_at`; an older create or edit never overwrites a newer one, and an edit always
  preserves the review schedule (`interval_days`, `next_due`);
- **flashcard reviews**: append-only, applied in cursor order. A review whose card is still
  unknown stays journaled and is applied once the card arrives;
- **preferences**: last write wins by `client_at`; speeds are clamped to 50–300 and an
  invalid subtitle language is skipped.

Every pushed `client_at` is clamped to the server receive time, so a device with a clock
far ahead cannot keep winning last-write-wins until the real clock catches up. Payload
instants (`created_at`, `updated_at`, `reviewed_at`) are reported by the device as-is.

Deleting a note, bookmark or flashcard creates a `note_deleted` / `bookmark_deleted` /
`flashcard_deleted` operation and a tombstone holding the delete instant; a create or
update whose own instant is not newer than the tombstone is skipped, so a row deleted on
one device is not resurrected by an older operation that arrives later.

An operation that references media the server does not know is still recorded in the log
and applied when the media appears; media that exists but is outside the user's course access
is refused as `404 media_not_found`. The push body accepts up to 200 operations and 4 MiB.

## Users, Avatars, Backup & Updates

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET`, `POST` | `/api/admin/users` | Admin Session | Lists users, or creates one `{ username, password, allowed_course_ids? }` |
| `PUT` | `/api/admin/users/{id}/courses` | Admin Session | Sets the courses a student may open |
| `DELETE` | `/api/admin/users/{id}` | Admin Session | Deletes a user |
| `GET`, `PUT`, `DELETE` | `/api/user/avatar` | Session | Reads, replaces (raw image body up to 2 MiB) or removes the user's avatar |
| `PUT`, `DELETE` | `/api/admin/courses/{id}/order` | Admin Session | Saves or resets a manual lesson order for a course |
| `GET` | `/api/admin/backup` | Admin Session | Downloads a transactional backup of the database; non-administrators get a plain-text `403` |
| `GET` | `/api/update` | Session | Update check result `{ current, enabled, available, latest, releasedAt, url, changelog, checkedAt, error }`; `?force=1` or `?force=true` bypasses the interval for administrators only |
| `GET` | `/api/update/history` | Admin Session | Entries the Compose updater wrote to `update-history.json` on the data volume |
| `GET` | `/api/media/capabilities` | Session | Reports whether on-demand transcoding is available (`ffmpeg` found) |
| `GET`, `HEAD` | `/api/media/transcode?course=...&path=...` | Session | Streams a transcoded lesson video; `start` seeks and `reencode=true` forces H.264 encoding |

## Search

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/search?q=<text>&limit=<n>` | Session | Returns the session user's notes and shared course/lesson title matches, grouped as `notes`, `courses`, and `lessons`. Notes include a highlighted `snippet` and the lesson position; queries shorter than two characters return empty groups. |
