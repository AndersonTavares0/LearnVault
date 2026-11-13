# API Reference

The primary backend implementation is the Rust server (`crates/server`), exposing the HTTP endpoints below. The Svelte web SPA uses the same study and catalog contract across both HTTP and desktop IPC transports. All endpoints consume and produce JSON unless otherwise specified. Authentication is handled via an HttpOnly cookie session with `SameSite=Strict`.

In the Rust server, each registered course directory remains under strict filesystem integrity control between administrative operations. The course registry is stored in `courses.json` alongside the database, and raw filesystem paths never cross the API boundary: clients receive only opaque course identifiers and sanitized titles. Rescans revalidate each root and reject detected tampering; however, concurrent alterations of symlinks or junctions by local users still depend on OS-level permissions.

The session cookie includes the `Secure` flag by default and is only transmitted over HTTPS. For local development without TLS, pass `--insecure-http`; this flag must never be used in production. State-mutating authentication endpoints require matching HTTP(S) Origin headers and `SameSite=Strict`.

## SPA Delivery in Rust Server

Static file serving is disabled by default. Pass `--web-root` pointing to the directory produced by the web build (`crates/server/web/build`), containing the `200.html` shell. Existing asset files are served directly; extensionless paths fall back to `200.html`; missing assets with file extensions return 404; `/api/*` endpoints never fall back to HTML. If the shell file is missing, the server returns 503. The shell is served with `Cache-Control: no-cache`; assets under `_app/immutable/` use immutable caching; SPA error responses use `no-store`. Directory validation never leaks local filesystem paths.

## Health & Authentication Endpoints

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/healthz` | Public | Process and database readiness, including running version |
| `POST` | `/api/login` | Public | Authenticates `{ "username": "...", "password": "..." }` |
| `POST` | `/api/logout` | Session | Invalidates the active session |
| `GET` | `/api/me` | Session | Returns current authenticated account details |
| `POST` | `/api/account/credentials` | Session | Accepts `{ "current_password": "...", "username": "...", "password": "..." }`, updates credentials, revokes older sessions, and issues a new session |

Initial bootstrap allows temporary credentials (`admin/admin`). The response marks `must_change_credentials: true`, and all protected endpoints return `403 credentials_change_required` until the password is changed.

Temporary credentials are created only when the server listens on a loopback address. A non-loopback listener requires explicit bootstrap credentials; a persisted administrator still marked for credential change also prevents network startup. Login rate limiting uses the socket peer address. `X-Real-IP` is honored only when that peer is listed in `TRUSTED_PROXY_IPS`; `X-Forwarded-For` is not trusted.

## Database Compatibility & Migration

The Rust server detects legacy database signatures (`users`, `sessions`, `schema_migrations`) before bootstrapping or running migrations. If detected, it exits with the message:
`legacy_go_database_requires_migration: back up the database and migrate it before using the Rust server`.
It never creates `admin/admin` or overwrites existing data in this scenario. The `learnvault-server migrate-go` subcommand converts legacy databases with backup-first verification.

## Catalog, Courses & Media

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/catalog` | Session | Returns top-level course nodes (`role: course` with opaque `course_id` and `title`), with modules, lessons, and materials |
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

Module directories containing video files are presented as collapsible modules with one lesson per video. Complementary materials remain available in the module and matching subtitles remain associated with the lesson video. Natural sorting is applied to course paths (`2` precedes `10`).

Without registered courses, the catalog returns empty. `LIBRARY_DIR`/`--library-root` is accepted as an initial deployment seed: when no registry exists, the specified directory is registered as the first course with ID `library-1`.

The scanner inspects directory metadata without traversing symbolic links. It ignores temporary download directories, detects media, subtitles, and documents, and only treats `.ts` files as video if their size exceeds 1 MiB (preventing collision with TypeScript files).

### Automatic Indexing

The server indexes registered courses in the background, outside the request path. A filesystem watcher coalesces event bursts with a debounce, and a periodic reconciliation (five minutes) remains the portable fallback for network mounts where events are unreliable. Only one job runs at a time, and a manual rescan is deferred or rejected with `409` while it runs.

Each course is scanned without holding the database and applied in its own transaction; the database lock and the heavy-work permit are released between courses, so ordinary API requests are not blocked for the duration of the library. The job tracks idle/running/done/cancelled/error, file counts, course progress, timestamps, and a stable error code, exposed by `GET /api/indexing`. Cancellation stops before the next course and leaves the already committed courses intact.

Detection is incremental: a known path is compared by size and mtime, and a sampled SHA-256 (both ends plus the length) separates a real content change from a mere timestamp touch. A file that appears at an unknown path matches a missing row by content hash (or by size and mtime when the hash is unavailable) and keeps its `media_id`, notes, bookmarks, and progress; ambiguous matches are refused instead of guessed. Files that disappear are marked `availability: missing`; no scan deletes rows, notes, bookmarks, or progress history. The read-only guarantee is unchanged: the scanner and the hash only read course files.

The desktop application does not run a background indexer; it keeps the explicit rescan command.


In the desktop app, `learnvault-media://localhost/<media-id>` delivers media and subtitle files using only opaque IDs. `GET` and `HEAD` support single ranges. On Windows, the SPA uses `http://learnvault-media.localhost/<media-id>`.

### Custom Covers

A custom cover is stored by the application—on the server in `covers/<id>.cover` (next to `courses.json`), and on desktop in the application data directory. Course folders remain read-only. Accepted formats are PNG, JPEG, and WebP, validated via magic bytes. Covers are written atomically via temporary files and `rename`.

Top-level course nodes include `cover_version` when a cover exists, prompting the SPA to bust client cache via `?v=`. Missing covers or unknown course IDs return `404` with an empty body.

## Remote Course Sources (WebDAV and SMB)

A remote source is a read-only course tree served by WebDAV or SMB. Administrators configure it
with `dav://`, `davs://`, `http://`, `https://` or `smb://`; `dav` maps to `http` and `davs` to
`https`. URLs with embedded credentials, a query string or a fragment are rejected, and every
source URL is limited to 2048 characters. A WebDAV base path always ends in `/`. TLS certificates
are always verified; there is no option to accept an invalid certificate. Redirects are refused so
the `Authorization` header never reaches another host. `dav://` and `http://` send Basic
credentials without transport encryption, so use them only on a trusted private network; prefer
`davs://`/`https://`.

SMB sources use `smb://host[:port]/share/path`. The share is mandatory; `.`/`..` segments, encoded
separators and control characters are rejected before they become a UNC path. The server is
contacted on TCP/445 with NTLM authentication and SMB signing support, and SMB3 encryption is used
when the server requires it. Kerberos is not supported in this build. DFS referrals are disabled so
credentials stay on the configured server instead of following a referral to another host.

Discovery uses `PROPFIND` with `Depth: 1` over WebDAV and directory enumeration over SMB; the tree
is walked through the read-only `CourseSource` abstraction (the local filesystem has a reference
implementation parity-tested against the local scanner), with hidden and temporary directories,
symlinks and the media classification rules identical to the local scanner. Media bytes are read
with a single `Range` over WebDAV (a server that ignores `Range` is rejected instead of streaming
the wrong span) or ranged SMB reads. Only `PROPFIND`, `GET` and read-only SMB operations are ever
issued: the remote server is never written, renamed or deleted.

All transports share the scan limits: at most 32 directory levels, 200,000 files, 8 MiB per
`read_range` call, and 100,000 entries in a single directory listing. Connection setup is limited
to 10 seconds and each listing or read block to 30 seconds; a full remote scan is limited to 120
seconds and a connection test to 20 seconds.

Configuration lives in `sources.json` next to the database, next to `courses.json`. Passwords are
encrypted at rest with ChaCha20-Poly1305 under a 32-byte key in `sources.key` (`0600`), created on
the first save. The API never returns the password or the ciphertext, only `has_password`. A
missing or replaced key leaves the source listed with `credential_error: true`; the original
ciphertext is preserved until the key is restored.

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/admin/sources` | Admin Session | Lists sources as `{ id, name, url, username, has_password, credential_error }`; never returns the secret |
| `POST` | `/api/admin/sources` | Admin Session | Accepts `{ name, url, username?, password? }` and returns `201` with the sanitized source |
| `DELETE` | `/api/admin/sources/{id}` | Admin Session | Removes the source configuration; returns `{ removed: bool }` and touches no remote file |
| `POST` | `/api/admin/sources/{id}/test` | Admin Session | Validates reachability and authentication with a root listing (`PROPFIND` or SMB directory enumeration); returns `{ ok, entries }` or `{ ok: false, error }` (`unreachable`, `denied`, `not_found`, `invalid_response`, `limit_exceeded`, `credentials_unavailable`, `timeout`) |
| `POST` | `/api/admin/sources/{id}/scan` | Admin Session | Walks the remote tree and applies it to the catalog as a course; returns the same rescan counters as the local rescan |

A scanned source appears in `GET /api/catalog` as a top-level `course` node with the source's
opaque `id`; `GET`/`HEAD /api/media?course=<source-id>&path=<relative-path>` streams its bytes with
`206` ranges. Only paths already present in the catalog are proxied to the remote server. Remote
courses have no custom cover, manual ordering, search indexing or on-demand transcoding yet.

## Study Progress, Focus & Gamification

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/study/capabilities` | Session | Returns supported study capabilities |
| `GET` | `/api/study/recent` | Session | Returns recently studied lessons |
| `GET` | `/api/study/progress/{course}/{lesson}` | Session | Returns `{ position_seconds, completed, updated_at }` |
| `PUT` | `/api/study/progress/{course}/{lesson}` | Session | Updates playback position and completion status |
| `GET` | `/api/study/summary` | Session | Returns daily study minutes, active streak, and weekly totals |
| `POST` | `/api/study/focus` | Session | Records an idempotent focus session |
| `POST` | `/api/study/rewards/claim` | Session | Claims daily mission or weekly goal XP rewards |
| `GET` | `/api/study/rewards` | Session | Returns gamification state (XP ledger, level, streak, forest trees) |
| `GET` | `/api/study/favorites` | Session | Lists bookmarked courses and lessons |
| `PUT` | `/api/study/favorites/courses/{id}` | Session | Bookmarks a course |
| `DELETE` | `/api/study/favorites/courses/{id}` | Session | Removes course bookmark |
| `PUT` | `/api/study/favorites/lessons/{course}/{lesson}` | Session | Bookmarks a lesson |
| `DELETE` | `/api/study/favorites/lessons/{course}/{lesson}` | Session | Removes lesson bookmark |

## Notes & Flashcards

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/study/notes` | Session | Lists study notes with optional course or lesson filters |
| `POST` | `/api/study/notes` | Session | Creates a timestamped study note |
| `PUT` | `/api/study/notes/{id}` | Session | Updates note text |
| `DELETE` | `/api/study/notes/{id}` | Session | Deletes a note |
| `GET` | `/api/study/flashcards` | Session | Lists flashcard review deck |
| `POST` | `/api/study/flashcards` | Session | Creates a spaced repetition flashcard |
| `GET` | `/api/study/flashcards/due` | Session | Returns `{ "due": <n> }` for the session user's cards due now |
| `GET` | `/api/study/flashcards/{id}/reviews?limit=<n>` | Session | Returns the card's recent reviews, newest first |
| `POST` | `/api/study/flashcards/{id}/review` | Session | Records flashcard review grade and calculates next interval |
| `DELETE` | `/api/study/flashcards/{id}` | Session | Deletes a flashcard |

Creating a card accepts an optional `position_seconds` (the lesson moment it came from), returned on every card as `position_seconds`. A card stays linked to its lesson through `media_id`. Each review appends a row to the review history with the rating, the previous interval and the next due date, so the schedule and its history change in one transaction.

The history endpoint is owner-scoped and read-only: `limit` defaults to 20 and is capped at `MAX_FLASHCARD_REVIEWS` (100). The response is `{ "items": [{ "review_id", "card_id", "rating", "previous_interval_days", "next_due", "reviewed_at" }], "total": <n> }`, where `total` counts the whole append-only log. The due count uses the same predicate as the review queue (`next_due <= now`) and only counts lessons the session user may still open; cards that were never reviewed are included.

## Offline Sync

The desktop keeps an offline queue and reconciles it with the server through an
append-only operation log. The owner is the authenticated session user; no endpoint accepts
an `owner_id`. Media files are never synchronized: only progress, notes, bookmarks,
flashcard reviews and playback preferences travel.

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `POST` | `/api/sync/push` | Session | Accepts `{ "device_id": "...", "operations": [...] }` and applies each new operation, returning `{ applied, duplicated, cursor }` |
| `GET` | `/api/sync/pull?device=<id>&cursor=<seq>&limit=<n>` | Session | Returns `{ cursor, has_more, items }` with the operations after `cursor`, excluding the requesting device |

Each operation is `{ "op_id": "...", "client_at": <unix-seconds>, "payload": { "kind": ... } }`
with one of the payload kinds below:

| Kind | Fields |
| --- | --- |
| `progress` | `media_id`, `position_seconds`, `completed` |
| `note` | `note_id`, `media_id`, `position_seconds`, `content`, `created_at`, `updated_at` |
| `bookmark` | `bookmark_id`, `media_id`, `position_seconds`, `label`, `created_at` |
| `flashcard_review` | `card_id`, `rating` (`again`/`good`), `reviewed_at` |
| `preference` | `video_speed`, `audio_speed` (percent of normal), `subtitle_language` |

`op_id` is stable and globally unique: replaying a pushed operation is a duplicate and does
not repeat its effect. `seq` is the monotonic pull cursor; rows are never deleted, so a
device resumes from the last cursor it applied. Pulling excludes operations produced by the
requesting `device_id`, because that device already applied them locally.

Conflict rules:

- **progress**: last write wins by `client_at`, with the position clamped to
  `[0, 86400]` seconds; completion is monotonic and is never undone by an older write;
- **notes**: each synchronized version is identified by its `op_id`. The largest `op_id`
  (UUID v7, so the latest version) owns the note row and every other version is kept as a
  conflict copy with the deterministic id `<note_id>~<op_id>`. No content is silently lost,
  and devices that saw the same versions converge on the same texts. Notes created before
  schema version 17 keep their row and never overwrite it;
- **flashcard reviews**: append-only, applied in cursor order;
- **preferences**: last write wins by `client_at`; speeds are clamped to 50–300 and an
  invalid subtitle language is skipped.

An operation that references media the server does not know is still recorded in the log
and applied when the media appears; media that exists but is outside the user's course access
is refused as `404 media_not_found`. The push body accepts up to 200 operations and 4 MiB.

## Search

| Method | Path | Auth | Purpose |
| --- | --- | --- | --- |
| `GET` | `/api/search?q=<text>&limit=<n>` | Session | Returns the session user's notes and shared course/lesson title matches, grouped as `notes`, `courses`, and `lessons`. Notes include a highlighted `snippet` and the lesson position; queries shorter than two characters return empty groups. |
