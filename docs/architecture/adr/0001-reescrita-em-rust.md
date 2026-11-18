# ADR 0001: Rewrite LearnVault in Rust with a shared core

## Status

Accepted — 2026-10-14 (Implemented — 2026-10-17)

This ADR records the **approved architecture**. The Rust rewrite was completed in Phases 1 to 5,
and the legacy Go implementation was fully removed from the repository in PR #206, making Rust
the single backend of LearnVault.

## Context

LearnVault turns local course folders into a personal study platform, used in two ways: as an
offline desktop application and as a self-hosted server with a web UI. The current implementation
is a Go service with SQLite and an embedded SvelteKit SPA.

The maintainer decided to adopt Rust as the long-term stack, with a single domain core shared
between the server and the desktop, and Tauri 2 as the desktop technology. The decision is
**technological and deliberate**: there is no concrete Go limitation that forces the change, and Go
with Wails would serve both modes with less work.

## Decision

LearnVault becomes a Rust workspace:

- `crates/core` — libraries, catalog, scanner, file identity, rescan policy, SQLite persistence
  with a single migration chain, and study state. It exposes typed operations and errors, knows
  nothing about HTTP or Tauri, and receives the already-resolved library root, the `library_id`
  and the study-state `owner_id`.
- `crates/server` — Axum server with Argon2id authentication, sessions, cookies, access
  authorization, media delivery with `Range` (206/416), security headers and SPA delivery.
- `crates/desktop` — Tauri 2 application with IPC commands, file system integration, directory
  selection and a media playback protocol. No HTTP, no cookies, no network ports and no
  authentication.
- `apps/web/` — a single SPA in Svelte 5 + TypeScript + Vite, without SvelteKit, with hash
  routing and two builds that select the transport at compile time.

### State after the migration

On 2026-10-19, the implementation phases of the stack were recorded as complete in issue #41.
The Go service was removed in PR #206; the explicit importer for legacy Go databases remains. The
Tauri window and the IPC slices described in `docs/architecture/overview.md` are implemented.
This does not complete distribution: the Flatpak/Linux desktop build is still pending sandbox
validation, and other pending targets are listed in `docs/product/roadmap.md` and
`docs/operations/distribution.md`.

Domain decisions that the rewrite keeps:

- Media identity (`media_id`) is separate from physical location. `relative_path` is the current
  location, not the key, and comparison is normalized according to the operating system.
- The library root is platform configuration, not domain data.
- Amendment (issue #182): the platform configuration unit becomes the **folder of each course**,
  registered individually — on the server in a file next to the database, on the desktop in
  `library.json` v2 — and the catalog aggregates one top-level node per course. The `library_id`
  remains a domain identity, the folder remains external and read-only, and the root seeded by the
  old deployment keeps the `library-1` identity so that media, scans and study progress already
  recorded are not lost.
- Rescan never deletes records automatically: a missing file is marked as missing, and a file that
  reappears at the same path reuses its `media_id`. Removal happens only through an explicit action.
- The physical library remains external and read-only.
- Study state is identified by an opaque `owner_id`: the desktop uses a fixed local owner and the
  server uses the identifier of the authenticated user.

Distribution: an NSIS installer on Windows; **Flatpak as the preferred Linux format**, subject to
technical validation of permissions, persistent access to libraries and media playback; an OCI
image plus a native binary for self-hosted installation. One version per release.

Execution happens in phases: governance, `crates/core`, `crates/server`, `apps/web/`,
`crates/desktop` and distribution, with the removal of the Go service in its own pull request.

## Alternatives considered

### Stay on Go

- **Pros:** no rewrite work; the current implementation already covers authentication, catalog,
  media with `Range` and deployment.
- **Cons:** it does not meet the maintainer's stack decision nor the reuse of a single core with
  the desktop through Tauri.
- **Why not:** adopting Rust is a deliberate, long-term decision.

### Go with Wails on the desktop

- **Pros:** it would reuse the existing core, tests and release pipeline, with a single
  toolchain.
- **Cons:** it would choose the stack for the friction of the migration, not for the desired
  destination.
- **Why not:** the maintainer prefers Tauri 2 and accepts the larger initial cost.

### Incremental migration of the Go service

- **Pros:** it would reduce risk by keeping both backends running in parallel.
- **Cons:** it would require maintaining two stacks and duplicated test infrastructure for the
  whole transition.
- **Why not:** it was decided to rewrite from scratch, using Go only as a reference.

## Consequences

### Positive

- A single domain core for both modes of use.
- An offline desktop that exposes no network ports.
- A single long-term stack for server and desktop.

### Negative

- A complete rewrite before functional parity exists.
- The domain, persistence and API are not reused from the current implementation.
- The desktop application on Linux depends on the system webview.

### Risks

- **Flatpak packaging:** treated as the preferred format, not the definitive one; permissions,
  persistent access to libraries and media playback need technical validation.
- **Playback on the packaged Linux desktop:** the protocol and the tests exist, but Flatpak still
  needs a real smoke test to validate persistent permissions and playback inside the sandbox.
- **A single version across binaries:** it requires a single source of truth for the version
  number; the server version is already exposed in `--version` and `/api/healthz`.

## Validation and rollback

Planned validation: Rust test coverage of at least 80% measured with `cargo-llvm-cov`,
`cargo fmt --check`, `cargo clippy` without warnings, SPA builds in both modes, and running the
distribution artifacts before each release.

Historical record: during the rewrite, the Go service remained available until its removal in
PR #206, on 2026-10-17. Today the Go runtime is not in the repository. Importing legacy SQLite
databases is explicit and backup-first; the procedure and its limits are in
`docs/operations/migration-go-to-rust.md`. There was no proven cutover/rollback rehearsal with
real volumes; that remains a separate operational task, not a code step still to be implemented.
