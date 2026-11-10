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

In the desktop app, `learnvault-media://localhost/<media-id>` delivers media and subtitle files using only opaque IDs. `GET` and `HEAD` support single ranges. On Windows, the SPA uses `http://learnvault-media.localhost/<media-id>`.

### Custom Covers

A custom cover is stored by the application—on the server in `covers/<id>.cover` (next to `courses.json`), and on desktop in the application data directory. Course folders remain read-only. Accepted formats are PNG, JPEG, and WebP, validated via magic bytes. Covers are written atomically via temporary files and `rename`.

Top-level course nodes include `cover_version` when a cover exists, prompting the SPA to bust client cache via `?v=`. Missing covers or unknown course IDs return `404` with an empty body.

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
| `POST` | `/api/study/flashcards/{id}/review` | Session | Records flashcard review grade and calculates next interval |
| `DELETE` | `/api/study/flashcards/{id}` | Session | Deletes a flashcard |
