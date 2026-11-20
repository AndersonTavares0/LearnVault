# Architecture

**Target audience:** engineers, reviewers, and maintainers deciding where a change belongs.
If you just want to run LearnVault, start with the [README](../../README.md).

**Summary:** LearnVault reads existing course folders, builds a SQLite catalog, and serves a
single-page app. The HTTP server and the offline desktop app share one domain core. Neither changes
files in the course library.

## Two Modes, One Core

- **Self-hosted server:** one administrator manages the library and accounts; other users sign in
  over the network with cookie sessions.
- **Offline desktop:** one user, no login, no listening port.

`crates/core` holds the catalog and study rules. `crates/server` and `crates/desktop` connect it to
HTTP or Tauri IPC, so both modes run the same logic and domain tests need neither sockets nor a
webview.

```text
Self-hosted Server                       Offline Desktop
  Browser / WebUI                          Embedded Webview
        |                                       |
        v                                       v
  crates/server (Axum)                 crates/desktop (Tauri 2)
        |  HTTP, cookie, Range                |  IPC, media protocol
        +-------------------+-----------------+
                            v
                    crates/core (domain, SQLite)
                            |
        +-------------------+-------------------+
        v                                       v
  course library (:ro)                    local data & cache
```

## Component Responsibilities

| Component | Responsibility | Boundary |
| --- | --- | --- |
| `crates/core` | Domain, catalog, scanner, media identity, rescan, SQLite, and study state | Has no HTTP or Tauri knowledge; receives pre-resolved course roots |
| `crates/server` | Authentication, sessions, `Range`, security headers, course registry, remote sources, transcoding, sync endpoints, and SPA delivery | Translates domain to HTTP; never reimplements domain rules |
| `crates/desktop` | Tauri window, course IPC, catalog, covers, rescan, study, local media protocols, update check, and the optional sync client | No authentication and no listening port |
| `apps/web-next` | React 19 + TypeScript + Vite SPA, hash routing, dual transport builds | Chooses transport at compile time; never touches files directly |

## Current State

The workspace is `crates/core`, `crates/server` (Axum), `crates/desktop` (Tauri 2), and
`apps/web-next` (React 19 + Vite). The Go backend and the Svelte SPA were removed.

- **Self-hosted server:** serves the HTTP API and the SPA. Course registrations live in
  `courses.json` and remote sources in `sources.json`, both next to the SQLite database.
  `LIBRARY_DIR` / `--library-root` seeds the registry on first start: each top-level folder becomes
  a course (nested courses are detected from the folder shape), or the root itself when it has no
  usable subfolder.
- **Remote sources:** WebDAV and SMB, read-only. An administrator browses a source and confirms which
  folders are courses; each confirmed folder becomes its own library ([ADR 0004](adr/0004-remote-course-roots.md)).
  Source hostnames are resolved and every address is checked before connecting.
- **Offline desktop:** opens the `main` window from `crates/desktop/tauri.conf.json`, with the SPA
  embedded through `frontendDist` and a custom titlebar. Import, removal, rescan, and study run
  through explicit IPC commands; the webview gets opaque identifiers, never filesystem paths. Course
  registrations live in `library.json` in the app configuration directory. The webview CSP allows only
  IPC. Rust makes two kinds of outbound request: the update check against GitHub Releases and, when
  the user links a server, sync ([ADR 0005](adr/0005-sync-journal-for-browser-writes.md)).
- **Sync:** the desktop pushes its outbox and pulls other devices' operations over HTTP. The server
  journals browser writes as device `server`, so they reach desktops too, and a pull applies the
  user's current course grants.
- **Media:** the desktop serves images, materials, video, audio, and subtitles through
  `learnvault-media://localhost/<media-id>` (`http://learnvault-media.localhost/<media-id>` on
  Windows WebView2), with a type allowlist, range support, and response limits. Opening a file in the
  system player goes through the confined opener. The server streams the same content over
  `/api/media`, with on-demand FFmpeg remux and transcode for extended formats.
- **Covers:** one image per course, up to 4 MiB, type checked by magic bytes, written atomically
  outside the course folder, versioned by `cover_version`. The desktop serves it through
  `learnvault-cover://`.
- **Study and focus:** playback position, completion, recents, streaks, Pomodoro sessions, and
  rewards live in the shared SQLite domain behind authenticated HTTP or local IPC. Sessions are
  idempotent by owner and ID; the XP ledger and daily claims cannot double-count. Timers and UI
  preferences stay on the device.
- **Version:** release builds carry the Release Please version. Source builds report `dev`.
  `/api/healthz` returns `{status, version}` when the database is ready and `503` otherwise.
- **Distribution:** see [distribution](../operations/distribution.md).

### SPA

Routes (hash router): `/` (My learning), `/course/:courseId/*` (course and lesson), `/progress`,
`/review`, `/notes`, `/history`, `/settings`, and `/login` (server build only). Old `#/player/...`,
`#/statistics`, and `#/focus` links redirect. The transport is chosen at compile time: HTTP for the
server, Tauri IPC for the desktop. Tailwind 4 and shadcn/ui primitives on Base UI provide the
components. Design tokens live in `apps/web-next/src/styles/app.css`. Builds never load fonts,
scripts, or stylesheets from a third party. See [DESIGN.md](../../DESIGN.md).

### Focus & Rewards Contract

- The core validates `1 <= planned_seconds <= 7200`, `0 <= actual_seconds <= planned_seconds`,
  and `0 <= ended_at - started_at <= 7200`, with non-negative timestamps. Actual duration cannot
  exceed elapsed time, and completing a session requires the full planned duration. Pauses can make
  elapsed time exceed focus time. HTTP and IPC enforce the same checks. Times come from the client
  and prove nothing about actual focus.
- Repeating an identical owner, ID, and payload returns the existing session without granting new XP.
  Reusing an ID with a conflicting payload returns HTTP `409 focus_session_conflict`.
  Invalid session parameters return `422 invalid_focus_session`.
- The outbox is local and isolated per account. A conflicting or rejected session leaves the queue only
  after its payload and rejection reason are saved to the rejection list shown in the UI. If storage
  is unavailable, sessions stay pending. After a transient network error the client waits for an
  explicit retry instead of polling.
- A completed activity counts toward the weekly goal of the week of its local date (`updated_at` for
  lessons, `ended_at` for focus), Monday through Sunday. Changing the goal re-evaluates only the current
  local week. The 50 XP award is granted once per owner and week, and raising the goal later does not
  revoke it.
- Claiming an unknown mission returns `404 mission_not_found` and an incomplete one `409 mission_not_complete`.
  Repeating a completed claim grants no extra XP.

```text
Local HTTPS Reverse Proxy
        |
        v
Rust HTTP Server -- SQLite + Course Registry (/data)
        |-- Cache (/cache)
        `-- Course Folders (/courses/*:ro)
```

## Invariants Every Change Must Respect

Changing an invariant needs a GitHub discussion first, not a code change.

- The physical course library is external and strictly read-only. Each registered directory is a course.
- SQLite stores metadata and personal study state. Course media remains strictly in the library.
- Rescan never deletes catalog records. It marks missing files and reuses identities when files reappear
  at the same path.
- Removing media is an explicit, permanent tombstone action. Records and progress persist, and rescans
  never resurrect removed media.
- The path to each course is platform configuration, not catalog content. The registry is stored alongside
  the database on the server or in the desktop app's configuration directory.
- Administrative actions are explicit. Nothing indexes or imports on its own, except the first-start seeding of an empty registry.
- Cache and derived media are ephemeral and kept strictly separated from durable database state.
- Custom covers belong to the application. They are stored next to the database on the server or in the
  desktop data directory, limited to 4 MiB, checked by magic bytes, and written atomically. No course
  folder receives files.
- Docker and Podman are optional deployment adapters. The runtime never depends on containers.

## Legacy Database Import

`legacy_import` migrates a database from the Go releases. It is explicit: the source is opened
read-only and backed up first, and the destination is promoted only after transaction integrity,
foreign keys, and row counts check out. See [the migration guide](../operations/migration-go-to-rust.md).

## Decisions

- [ADR index](adr/README.md). [ADR 0001](adr/0001-reescrita-em-rust.md) explains the Rust rewrite.
- [Distribution](../operations/distribution.md) covers binary packages and deployment.

## Related Documentation

- [Requirements](../product/requirements.md)
- [Governance](../governance/overview.md): contribution rules, CI checks, release policy.
- [Distribution](../operations/distribution.md)
