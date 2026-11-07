# Product Requirements

**Target audience:** product, QA, engineering, and anyone referencing a requirement in an issue or PR review.
If you are new to the project, start with the [README](../../README.md) and [Roadmap](roadmap.md).

**Conventions:** FR denotes functional requirements (what the product does). NFR denotes non-functional requirements
(how the product behaves). Requirement IDs are stable and intended for cross-referencing. The [roadmap](roadmap.md)
tracks delivery status; GitHub issues track in-flight implementation work.

LearnVault has two deployment modes, defined in [ADR 0001](../architecture/adr/0001-reescrita-em-rust.md):
the **self-hosted server** with WebUI, and the **offline desktop application**.

## Modes of Operation

| Mode | User | Authentication |
| --- | --- | --- |
| Self-hosted server | Administrator and local users over the network or WebUI | Local accounts with cookie session |
| Offline desktop | Single user on their local machine, offline | No login; fixed local user identity in Rust |

Source: [Architecture Overview](../architecture/overview.md); ADR 0001.

## Functional Requirements

- **FR-01: Read-Only Course Library.** Index an existing course folder without moving, altering, renaming, or deleting any original files.
- **FR-02: Catalog.** Persist and serve the hierarchical tree of categories, courses, modules, and lessons, with materials and loose files detected by the scanner.
- **FR-03: Media Identity.** Each media item receives an opaque `media_id` decoupled from physical paths; files reappearing at the same relative path reuse their identity.
- **FR-04: Explicit Preserving Rescan.** Scans execute only on explicit user request; scans register new items, confirm existing ones, and mark disappeared items as missing without deleting records or progress.
- **FR-05: Explicit Media Removal.** Removing a media item is an explicit, permanent tombstone action; rescans never un-remove it, and study progress is preserved.
- **FR-06: Server Accounts & Sessions.** Support login, logout, current account queries, and credential updates with invalidation of previous sessions; initial access requires setting a secure password.
- **FR-07: Authenticated Media Streaming.** Stream approved library media with HTTP `Range` support (`206`/`416`), rejecting missing or tombstoned media.
- **FR-08: Study Progress.** Track study progress per user with playback resume position, completion status decoupled from resume position, and recent history with public limits.
- **FR-09: Offline Desktop.** Choose course library folder via native OS dialog, index courses, browse catalog, and record study progress over local IPC without network listeners or cookies.
- **FR-10: Study Interface.** Provide course navigation, lesson player, resume playback, empty states, and error handling while maintaining consistent design identity.
- **FR-11: Distribution.** Provide Windows NSIS installer, Linux Flatpak desktop package (pending sandbox verification), OCI container image, and native Linux server tarballs.

## Non-Functional Requirements

- **NFR-01: Data Safety.** The course library is external and strictly read-only; SQLite stores only metadata and study progress, never original media.
- **NFR-02: Desktop Trust Boundary.** Webview never sends or receives arbitrary filesystem paths; directory selection uses native OS dialogs in Rust, and media is addressed by opaque `media_id` with minimal Tauri capabilities.
- **NFR-03: Privacy & Network Security.** Desktop app exposes no network ports and uses no HTTP authentication. Self-hosted server uses HttpOnly cookies with `SameSite=Strict` and `Secure` by default (`--insecure-http` is restricted to local development).
- **NFR-04: Streaming Performance.** Media plays with partial streaming and seeking without buffering entire files in memory; performance-sensitive changes include benchmark measurements.
- **NFR-05: Operability.** Server exposes health and database readiness at `/api/healthz`; exposed deployments use HTTPS reverse proxy; data and cache directories are separated; rollbacks use verifiable checksums.
- **NFR-06: Portability.** Core compiles without Docker or Podman; containerization is an optional deployment adapter.
- **NFR-07: Quality.** Rust test coverage maintained with cargo tests and clippy warnings treated as errors (`-D warnings`).
- **NFR-08: Accessibility.** Keyboard navigation, visible focus indicators, high contrast, reduced motion support, and screen reader labels.
- **NFR-09: Language & Internationalization.** Canonical documentation, commit messages, and PRs are maintained in English (`en`); the application provides runtime internationalization (i18n) supporting English (default) and Brazilian Portuguese (`pt-BR`).

## Related Documentation

- [Roadmap](roadmap.md) for delivery status of each requirement.
- [Architecture Overview](../architecture/overview.md) for technical decisions supporting these requirements.
- [Governance](../governance/overview.md) for contribution and review workflows.
