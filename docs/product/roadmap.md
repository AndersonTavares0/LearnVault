# LearnVault Roadmap

**Target audience:** anyone who wants the current product status and direction.
Requirements are in [requirements.md](requirements.md); architecture is in
[overview.md](../architecture/overview.md).

## Status Guide

- **Delivered:** merged into `main` and available for use.
- **In Progress:** active engineering focus.
- **Next:** planned for upcoming iterations.
- **Later:** backlog concepts under evaluation.

## Delivered

- **Rust Core:** domain, SQLite, and catalog in `crates/core`; Axum HTTP server in `crates/server`; React 19 + Vite web frontend in `apps/web-next`. The Go service and the Svelte SPA are gone.
- **Offline Desktop (Tauri 2):** catalog, native folder picker, per-course import and removal, explicit rescan, a local media protocol addressed by opaque ID, and study progress over IPC. It has no authentication and opens no listening port.
- **Study Rules:** progress, streaks, XP, missions, and badges, shared by the server and the desktop.
- **Video & Audio Player:** media and subtitle selection per lesson, keyboard shortcuts (`J`/`K`/`L`, `0-9`, `M`, `N`, `F`, `P`, `[`/`]`), persistent playback rate, Picture-in-Picture, interactive timeline with note markers, optional auto-advance, on-demand FFmpeg remuxing/transcoding for extended formats (`.mkv`, `.avi`, `.wmv`, `.ts`, HEVC, AC3), and opening in the default system media player on desktop.
- **Course Study View:** integrated player and lesson playlist, resuming last watched lesson, quick lesson completion toggle in the course tree, active module auto-expansion, module materials separated from the video playlist, and course administration actions.
- **Study Notes & Flashcards:** timestamped study notes with one-click seeking, global note search with course filtering, Markdown export, and spaced repetition flashcards on Web and desktop.
- **PDF Reader:** continuous or single-page reading, search within the open document, highlights, underlines, notes, and annotated PDF export. The original PDF stays unchanged.
- **Focus & Habits:** Pomodoro timer with an offline queue, study streaks, daily missions, and a weekly study-day goal.
- **Course Covers:** automatic title-based covers, root image detection (`cover.*`, `folder.*`, `poster.*`, `thumb.*`), and custom cover upload / native picker with PNG/JPEG/WebP validation, 4 MiB limit, and atomic file saving outside course folders.
- **Backup & Restore:** an admin downloads a consistent, integrity-checked SQLite copy from Settings. Restore is the `learnvault-server restore` command, with the service stopped.
- **Multi-User Accounts & Course Access:** local student accounts, admin management, and per-course grants.
- **Remote Sources:** WebDAV and SMB sources with administrator-chosen course folders ([ADR 0004](../architecture/adr/0004-remote-course-roots.md)), including automatic course detection.
- **Desktop Sync:** optional link to a server account with an offline outbox; browser writes reach the desktop on pull, and a pull applies the user's current course grants ([ADR 0005](../architecture/adr/0005-sync-journal-for-browser-writes.md)).
- **i18n:** runtime switch between English (default) and Brazilian Portuguese (`pt-BR`), remembered across reloads.
- **Distribution:** Windows NSIS installer with a signed updater, Linux Flatpak desktop bundles (amd64, arm64; since v0.9.5, GitHub Releases only), Linux server tarballs, and OCI images, with verifiable `SHA256SUMS`.

## In Progress: Polish & Production Quality

- [x] Streamlined library → lesson → focus → habit loop on Web and desktop.
- [x] Resume strip on the home screen with status filters.
- [x] Timestamped notes, global notes search, and spaced repetition cards.
- [x] Extended media format support, on-demand server transcoding, and desktop external player.
- [x] Local multi-user student accounts with course access permissions.
- [x] Course and lesson bookmarking / favorites (#304).
- [x] i18n (`en` default, `pt-BR`) and English documentation (#306).
- [ ] Accessibility: a manual keyboard and screen reader pass is still owed ([checklist](../design/accessibility.md)).

## Next: Study Desk Enhancements

- [ ] On-demand remux persistent cache for incompatible media formats.
- [ ] Full-text search inside course materials and subtitle tracks.

## Distribution & Packaging

- [x] Maintain core compilation without Docker or Podman.
- [x] Publish OCI container image and native server binaries for self-hosted deployment.
- [x] Provide Docker Compose and Podman Quadlet deployment templates.
- [x] Package the Linux desktop app as a Flatpak with the `xdg-desktop-portal` file chooser ([details](../operations/flatpak.md)).
- [ ] Sign Windows binaries with a code-signing certificate. The updater payload is already signed; the installer is not.

## Long-term Explorations

- [ ] Optional local Whisper integration for speech-to-text transcription.
- [ ] Subtitle indexing and searchable transcripts.
- [ ] Automated scheduled database backups with retention policies.
- [ ] Native mobile companion client once desktop/web APIs stabilize.

## Delivery Invariants

- Every deliverable ships with automated tests and documentation updates.
- Distribution targets are independent: a container build failure does not block native server packaging.
- No feature modifies or copies the original course library.
- Scope changes go through GitHub issues and pull requests.

## Related Documentation

- [Requirements](requirements.md)
- [Architecture](../architecture/overview.md)
