# LearnVault — Product Context & Invariants

## Purpose & Target Audience

People who study with course folders they already own need a clean, structured workspace to discover lessons, stream video/audio, and resume their studies without reorganizing or altering physical files.

LearnVault serves two primary workflows:
1. **Self-Hosted Server:** Multi-user web application with local session authentication, accessible across home or private networks.
2. **Offline Desktop Application:** Native Tauri 2 desktop vault running entirely offline with zero network listeners, accounts, or cloud dependencies.

---

## Core User Journeys

1. **Import & Discover:** Select or register a course directory and browse categories, courses, and lessons with instant live search (`/`).
2. **Study & Progress:** Stream media with partial-content seeking (`206`), take timestamped notes, review spaced-repetition flashcards, and mark lessons complete independently of seek position.
3. **Resume & Retain:** Pick up immediately from the last studied lesson via the "Continue studying" row, track weekly goals, and view visual study streaks.
4. **Focused Sessions:** Plant trees and grow a 2.5D island grove in the Study Forest while maintaining focus through integrated Pomodoro cycles.

---

## Non-Negotiable Product Invariants

1. **Read-Only Library:** The course library on the host filesystem is strictly read-only. LearnVault never modifies, moves, deletes, renames, or copies original course files.
2. **Explicit, Preserving Rescan:** Catalog scans execute only on explicit user request. Rescans register new items, confirm existing ones, and mark disappeared items as missing without erasing study history or notes.
3. **Zero Cloud Lock-In & Zero Marketplace Mechanics:** No payment systems, subscription gates, course marketplaces, certificate counters, cloud tracking, or mandatory telemetry.
4. **Independent Completion & Playback Position:** Marking a lesson completed is independent of its playback resume position.
5. **Runtime Internationalization (i18n):** User-facing UI, alerts, errors, and menus fully support English (`en`, default) and Brazilian Portuguese (`pt-BR`). Raw untranslated strings are forbidden.
6. **Data Privacy:** SQLite stores metadata, progress, and user notes locally. Passwords use Argon2id with salted hashes; sessions use cryptographically secure tokens.

---

## Technical Architecture Overview

- **Core (`crates/core`):** Shared Rust domain logic, filesystem scanner, media identity resolution, and SQLite database migrations.
- **Server (`crates/server`):** Axum HTTP API, cookie-based authentication, partial media streaming, and embedded SPA delivery.
- **Desktop (`crates/desktop`):** Tauri 2 desktop wrapper with native OS folder picker and local IPC commands.
- **Web SPA (`apps/web`):** Svelte 5 + TypeScript + Vite responsive frontend bundle.

---

## Related Documentation

- Detailed functional and non-functional requirements: [`docs/product/requirements.md`](docs/product/requirements.md)
- Delivery status and planned milestones: [`docs/product/roadmap.md`](docs/product/roadmap.md)
- System architecture and ADRs: [`docs/architecture/overview.md`](docs/architecture/overview.md)
- Design system and visual tokens: [`DESIGN.md`](DESIGN.md) and [`docs/design/DESIGN.md`](docs/design/DESIGN.md)
