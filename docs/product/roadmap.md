# LearnVault Roadmap

**Target audience:** anyone wanting to understand the current product status and future direction.
Detailed requirements are in [requirements.md](requirements.md); technical architecture is in
[overview.md](../architecture/overview.md).

## Status Guide

- **Delivered:** merged into `main` and available for use.
- **In Progress:** active engineering focus.
- **Next:** planned for upcoming iterations.
- **Later:** backlog concepts under evaluation.

## Delivered

- **Rust Core:** domain, SQLite, and catalog in `crates/core`; Axum HTTP server in `crates/server`; Svelte 5 + Vite web frontend in `apps/web`. The legacy Go service was fully replaced.
- **Offline Desktop (Tauri 2):** catalog, native library folder picker, per-course import and removal, explicit library rescan, local media protocol by opaque ID, and study progress via internal IPC. The desktop app requires no authentication and opens no network ports.
- **Study Rules Engine:** shared progress tracking, streaks, and gamification across server, web UI, and desktop.
- **Video & Audio Player:** media and subtitle selection per lesson, keyboard shortcuts (`J`/`K`/`L`, `0-9`, `M`, `N`, `F`, `P`, `[`/`]`), persistent playback rate, Picture-in-Picture, interactive timeline with note markers, optional auto-advance, on-demand FFmpeg remuxing/transcoding for extended formats (`.mkv`, `.avi`, `.wmv`, `.ts`, HEVC, AC3), and opening in the default system media player on desktop.
- **Course Study View:** integrated player and lesson playlist, resuming last watched lesson, quick lesson completion toggle in the course tree, active module auto-expansion, module materials separated from the video playlist, and course administration actions.
- **Study Notes & Flashcards:** timestamped study notes with one-click seeking, global note search with course filtering, and spaced repetition flashcards per lesson on both Web and desktop.
- **Focus, Habits & 2.5D Study Forest:** idempotent Pomodoro focus timer with offline queue, study streak tracking, daily missions, weekly focus goals, and a 2.5D isometric forest powered by completed focus sessions.
- **Course Covers:** automatic title-based covers, root image detection (`cover.*`, `folder.*`, `poster.*`, `thumb.*`), and custom cover upload / native picker with PNG/JPEG/WebP validation, 4 MiB limit, and atomic file saving outside course folders.
- **Administrative Backup & Restore:** secure SQLite database download and integrity-checked restore via the server admin interface.
- **Multi-User Accounts & Course Access Permissions:** local student accounts, admin management, and granular course assignment.
- **Internationalization (i18n):** runtime language switching supporting English (default) and Brazilian Portuguese (`pt-BR`) with persistence across reloads.
- **Distribution:** Windows NSIS installer, signed updater (Ed25519), Linux server tarballs, and OCI container images with verifiable `SHA256SUMS`.

## In Progress: Polish & Production Quality

- [x] Streamlined library → lesson → focus → habit loop on Web and desktop.
- [x] "Continue studying" hero section on home screen with status filters.
- [x] Timestamped notes, global notes search, and spaced repetition cards.
- [x] Extended media format support, on-demand server transcoding, and desktop external player.
- [x] Local multi-user student accounts with course access permissions.
- [x] Course and lesson bookmarking / favorites (#304).
- [x] Full i18n support (`en` default, `pt-BR` fully supported) and English canonical documentation (#306).
- [ ] Continuous accessibility improvements: keyboard trap prevention, ARIA announcements, and screen reader testing.

## Next: Study Desk Enhancements

- [ ] On-demand remux persistent cache for incompatible media formats.
- [ ] Full-text search inside course materials and subtitle tracks.

## Distribution & Packaging

- [x] Maintain core compilation without Docker or Podman.
- [x] Publish OCI container image and native server binaries for self-hosted deployment.
- [x] Provide Docker Compose and Podman Quadlet deployment templates.
- [ ] Package Linux Flatpak desktop app using `xdg-portal` file chooser in verified sandbox.
- [ ] Sign Windows desktop binaries with code-signing certificates when available.

## Long-term Explorations

- [ ] Optional local Whisper integration for speech-to-text transcription.
- [ ] Subtitle indexing and searchable transcripts.
- [ ] Automated scheduled database backups with retention policies.
- [ ] Native mobile companion client once desktop/web APIs stabilize.

## Delivery Invariants

- Every deliverable includes automated tests, documentation updates, and clear verification.
- Distribution targets are additive: container build failures do not block native server packaging.
- No feature ever modifies or copies the original physical course library.
- Scope adjustments are coordinated through GitHub issues and pull requests.

## Related Documentation

- [Requirements](requirements.md) for functional and non-functional requirements.
- [Architecture](../architecture/overview.md) for architectural boundaries and design decisions.
