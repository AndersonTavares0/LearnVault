# Product Requirements

**Target audience:** product, QA, engineering, and anyone citing a requirement in an issue or PR review.
New to the project? Start with the [README](../../README.md) and the [roadmap](roadmap.md).

**Conventions:** FR is a functional requirement (what the product does), NFR a non-functional one (how it behaves).
IDs are stable, so cite them in issues and PRs. The [roadmap](roadmap.md) tracks delivery status and GitHub issues
track work in flight.

LearnVault has two deployment modes, defined in [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md):
the **self-hosted server** with WebUI, and the **offline desktop application**.

## Modes of Operation

| Mode | User | Authentication |
| --- | --- | --- |
| Self-hosted server | Administrator and student accounts over the network, through the web UI | Local accounts with a cookie session |
| Offline desktop | Single user on their local machine, offline | No login; fixed local user identity in Rust |

Sources: [architecture overview](../architecture/overview.md) and ADR 0001.

## Functional Requirements

- **FR-01: Read-Only Course Library.** Index an existing course folder without moving, altering, renaming, or deleting any original files.
- **FR-02: Catalog.** Persist and serve the tree of courses, modules, and lessons, with the materials and loose files the scanner finds.
- **FR-03: Media Identity.** Each media item gets an opaque `media_id` that does not depend on its path. A file that reappears at the same relative path keeps its identity.
- **FR-04: Preserving Rescan.** Users can request a rescan, and the server also reconciles registered local courses on startup, after filesystem changes, and periodically. The desktop rescans only on request. A scan registers new items, confirms existing ones, and marks vanished ones as missing without deleting records or progress.
- **FR-05: Explicit Media Removal.** Removing a media item is an explicit, permanent tombstone. Rescans never bring it back and study progress is kept.
- **FR-06: Server Accounts & Sessions.** Support login, logout, reading the current account, and credential updates that invalidate earlier sessions. There are two roles, administrator and student. A temporary initial administrator credential must be changed before the account can use anything else.
- **FR-07: Authenticated Media Streaming.** Stream library media with HTTP `Range` support (`206`/`416`) and reject missing or tombstoned media.
- **FR-08: Study Progress.** Track study progress per user: resume position, completion independent of position, and recent history with bounded limits.
- **FR-09: Offline Desktop.** Pick course folders with the native OS dialog, index them, browse the catalog, and record study progress over local IPC, with no network listener and no cookies.
- **FR-10: Study Interface.** Provide course navigation, a lesson player, resume playback, notes, flashcards, a PDF reader, empty states, and recoverable errors.
- **FR-11: Distribution.** Publish the Windows NSIS installer, Linux desktop Flatpaks (amd64 and arm64), and native Linux server tarballs as GitHub Release assets, and the OCI image to GHCR.
- **FR-12: Course Access.** An administrator sets each student account to all courses or to a chosen list. The new-account form starts with no course selected. Administrators see every course.
- **FR-13: Remote Sources.** An administrator can register WebDAV or SMB sources as read-only libraries and choose which of their folders are courses ([ADR 0004](../architecture/adr/0004-remote-course-roots.md)).
- **FR-14: Desktop Sync.** A desktop user can optionally link a server account. Study writes queue locally and sync over HTTP; the browser's writes reach the desktop on pull ([ADR 0005](../architecture/adr/0005-sync-journal-for-browser-writes.md)).

## Non-Functional Requirements

- **NFR-01: Data Safety.** The course library is external and strictly read-only; SQLite stores only metadata and study progress, never original media.
- **NFR-02: Desktop Trust Boundary.** The webview never sends or receives a filesystem path. Rust opens the native folder dialog, media is addressed by opaque `media_id`, and the Tauri capabilities are an explicit allowlist.
- **NFR-03: Privacy & Network Security.** The desktop app opens no listening port and has no authentication. Its webview CSP allows bundled assets, IPC, and the app's own media and cover schemes, and no remote origin. Its only outbound traffic is the optional sync with a server the user configures and the update check against GitHub Releases. The server sets `HttpOnly`, `SameSite=Strict` and, by default, `Secure` cookies; `--insecure-http` is for local development.
- **NFR-04: Streaming Performance.** Media plays with partial streaming and seeking and never buffers a whole file in memory. Performance-sensitive changes include measurements.
- **NFR-05: Operability.** `/api/healthz` reports health and database readiness (`503` when the database is unavailable). Exposed deployments sit behind an HTTPS reverse proxy. Data and cache directories are separate. Rollbacks use verifiable checksums.
- **NFR-06: Portability.** The core builds and tests without Docker or Podman. Containers are an optional deployment adapter.
- **NFR-07: Quality.** `crates/core` keeps at least 80% line coverage in CI, and clippy warnings fail the build (`-D warnings`).
- **NFR-08: Accessibility.** Keyboard navigation, visible focus, WCAG AA text contrast, reduced-motion support, and screen reader labels. Targets are [WCAG 2.2 AA](../design/accessibility.md); there is no external audit.
- **NFR-09: Language & Internationalization.** Documentation, commit messages, and PRs are in English. The app switches at runtime between English (default) and Brazilian Portuguese (`pt-BR`).

## Related Documentation

- [Roadmap](roadmap.md): delivery status.
- [Architecture overview](../architecture/overview.md): the technical decisions behind these requirements.
- [Governance](../governance/overview.md): contribution and review workflow.
