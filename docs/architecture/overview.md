# Architecture

**Target audience:** engineers, reviewers, and maintainers deciding where a change belongs.
If you just want to run LearnVault, start with the [README](../../README.md).

**Summary:** LearnVault reads existing course folders, builds a SQLite catalog, and serves
a single-page application (SPA). The HTTP server and offline desktop application share the
same domain core. Neither ever alters or mutates files in the course library.

## Two Modes, One Core

The product serves two main scenarios:

- **Self-hosted server:** one administrator manages the library and accounts; other users access
  over the network with authenticated sessions.
- **Offline desktop:** a single user runs the app on their local machine without login and without
  opening network ports.

This separation prevents duplicating catalog and study business rules. `crates/core` encapsulates
these rules; `crates/server` and `crates/desktop` connect the core to HTTP or Tauri IPC.
Thus, identical logic runs across both modes, and domain tests require neither network sockets
nor webviews.

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
| `crates/server` | Authentication, sessions, `Range`, security headers, course registry, and SPA delivery | Translates domain to HTTP; never reimplements domain rules |
| `crates/desktop` | Tauri window, course IPC, catalog, covers, rescan, study, and local media streaming | Has no authentication and opens no network ports |
| `apps/web-next` | React 19 + TypeScript + Vite SPA, hash routing, dual transport builds | Chooses transport at compile time; never touches files directly |

## Current State

The Rust workspace is complete: `crates/server` (Axum), `crates/desktop` (Tauri 2), and
`apps/web-next` (React 19 + Vite), with the shared domain in `crates/core`. The legacy Go backend
was removed in Phase 5, and the Svelte SPA was removed after the React cutover.

- **Self-hosted server:** exposes the HTTP API, authenticates users, and serves the SPA. Course
  registrations live in `courses.json` alongside the SQLite database; `LIBRARY_DIR`/`--library-root`
  seeds the registry on first start, registering each top-level folder as a course.
- **Offline desktop:** opens the `main` window defined in `crates/desktop/tauri.conf.json`, with the
  embedded SPA via `frontendDist`. The custom titlebar uses native Tauri window controls for minimize,
  maximize/restore, and close; window permissions are strictly scoped to the `main` window.
  Import, removal, rescan, and study operations run through explicit IPC commands; the webview
  receives opaque identifiers, never raw filesystem paths.
- **Media:** the `learnvault-media://localhost/<media-id>` protocol serves images, materials,
  video, audio, and subtitles with type allowlisting, HTTP range support, and response limits.
  On Windows (WebView2), the webview uses `http://learnvault-media.localhost/<media-id>`.
- **Covers:** each course can have an image up to 4 MiB, with format detected via magic bytes,
  atomic file writing, and `cover_version` tracking in the catalog. Covers are stored outside
  the course folder; in desktop mode, they use the `learnvault-cover://` protocol and dedicated IPC.
- **Study & focus:** playback position, completion status, recents, streaks, Pomodoro sessions,
  and rewards use the shared SQLite domain in `crates/core`, with authenticated HTTP and local
  desktop IPC adapters. Sessions are idempotent by owner and ID; the XP ledger and daily claims
  prevent duplication. Timers and UI preferences remain local to the device. The desktop player
  plays local media, renders VTT subtitles, and offers SRT/ASS/SSA downloads; the HTTP build
  transcodes and remuxes extended media formats on demand via FFmpeg.
- **Distribution:** Windows NSIS installer is available; Linux/Flatpak packaging is in progress.
  See [distribution](../operations/distribution.md).

The frontend is React 19 with Tailwind 4 and shadcn/ui primitives built on Base UI, in
`apps/web-next`. Design tokens live once in `apps/web-next/src/styles/app.css` and are mapped by
Tailwind's `@theme inline`. Project styling maintains light and dark theme support; builds never
load fonts, scripts, or stylesheets over third-party CDNs.

### Focus & Rewards Contract

- The core validates `1 <= planned_seconds <= 7200`, `0 <= actual_seconds <= planned_seconds`,
  and `0 <= ended_at - started_at <= 7200`, with non-negative timestamps. Actual duration cannot
  exceed elapsed time; completing a session requires the full planned duration. Pauses may cause
  elapsed duration to exceed actual focus time. Both HTTP and desktop IPC enforce the same core
  validations. Times are declared by the client, not cryptographic proof of focus.
- Repeating an identical owner, ID, and payload returns the existing session without granting new XP.
  Reusing an ID with a conflicting payload returns HTTP `409 focus_session_conflict`.
  Invalid session parameters return `422 invalid_focus_session`.
- The outbox is local and isolated per account. Conflicting and rejected sessions are dequeued
  only after preserving the payload and rejection reason in the local rejection list shown in the UI.
  If storage is unavailable, sessions remain pending. Transient network errors await explicit retry
  without endless background polling loops.
- Completed activities grant the weekly goal in the week of their local date (`updated_at` for lessons,
  `ended_at` for focus), evaluated Monday through Sunday. Changing goal preferences re-evaluates only
  the current local week. The 50 XP award is granted once per owner/week; subsequently increasing
  the goal does not revoke already granted XP.
- Claiming an unknown mission returns `404 mission_not_found`; claiming an incomplete mission returns
  `409 mission_not_complete`. Repeating a completed claim does not duplicate XP.

```text
Local HTTPS Reverse Proxy
        |
        v
Rust HTTP Server -- SQLite + Course Registry (/data)
        |-- Cache (/cache)
        `-- Course Folders (/courses/*:ro)
```

## Invariants Every Change Must Respect

These invariants reflect architectural principles established through deliberate design. If a change
requires modifying an invariant, discussion must happen on GitHub first, not directly in code.

- The physical course library is external and strictly read-only. Each registered directory is a course.
- SQLite stores metadata and personal study state. Course media remains strictly in the library.
- Rescan never deletes catalog records. It marks missing files and reuses identities when files reappear
  at the same path.
- Removing media is an explicit, permanent tombstone action. Records and progress persist, and rescans
  never resurrect removed media.
- The path to each course is platform configuration, not catalog content. The registry is stored alongside
  the database on the server or in the desktop app's configuration directory.
- Administrative actions are always explicit. There is no silent indexing on startup or unprompted imports.
- Cache and derived media are ephemeral and kept strictly separated from durable database state.
- Custom covers belong to the application, not the course folder. They are stored next to the database
  on the server or in the desktop data directory, limited to 4 MiB, validated via magic bytes, and written
  atomically. No course folder ever receives files.
- Docker and Podman are optional deployment adapters. The runtime never depends on containers.

## Legacy Database Import

The core provides `legacy_import` for migrating from previous Go releases. Import is explicit:
the source is read-only, backed up before execution, and the destination is promoted only after
verifying transaction integrity, foreign keys, and row counts. The physical course library is
never touched during migration.

## Decisions & Alternatives

The architectural rationale for the Rust rewrite and alternative designs considered are documented in
[ADR 0001](adr/0001-reescrita-em-rust.md). See [distribution](../operations/distribution.md)
for binary packages and deployment details.

## Related Documentation

- [Requirements](../product/requirements.md) for numbered functional and non-functional requirements.
- [Governance](../governance/overview.md) for contribution guidelines, CI checks, and release policies.
- [Distribution](../operations/distribution.md) for operations, packaging, and deployment setups.
