# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

LearnVault ships one React single-page app in two shells: a self-hosted server build (browser) and an offline Tauri desktop build (webview). Both count as `web`. The desktop shell adds a native folder picker and window controls and keeps the same design.

## Users

Self-directed learners (developers, researchers, hobbyists) who study from course folders they already own: video and audio lessons with subtitles, plus support files such as PDFs. They study in long evening sessions and in short daily visits that must resume exactly where they stopped.

Two setups:
- **Server:** one household or small team on a home or private network. Several local accounts (administrator or student), each with its own progress and per-course access. An administrator imports courses and manages accounts.
- **Desktop:** one person, no accounts, working offline. The desktop can optionally sync with a server account.

The job: reach the next lesson in one step, study with timestamped notes and flashcards, and keep a habit without reorganizing any files.

## Product Purpose

LearnVault turns folders of course material into a study space with the structure of a course platform: a library, courses with a curriculum, a lesson player, progress, notes and flashcards. It works when a learner opens the app, resumes the right lesson in one click, and comes back the next day.

## Positioning

A course platform for courses you already own, with study data kept on your own machine or server. A hosted platform cannot offer a library it never touches, accounts it does not hold, or an offline desktop build with no listening port.

## Operating Context

- Source material lives on the host filesystem as folders of modules and lessons. LearnVault reads it and never changes it.
- The catalog is built by an explicit scan. A rescan adds new items, confirms existing ones, and marks vanished ones as missing without erasing study history.
- Server: Rust (Axum) with SQLite, cookie sessions, range-request streaming, on-demand FFmpeg transcoding, and WebDAV and SMB sources. Desktop: Tauri 2 with local IPC and custom media schemes.
- Docker, Podman and Compose are optional deployment adapters, never a runtime requirement.
- The interface is English by default, with Brazilian Portuguese supported. The language is set in Settings.

## Capabilities and Constraints

Functionality: library and search; course page with curriculum and materials; lesson player with subtitles, speed, picture-in-picture, cinema mode, shortcuts and a resume position; a PDF reader with page search and highlights, underlines and notes (Markdown export); timestamped notes with a formatting bar and Markdown export; spaced-repetition flashcards; favourites; watch history; Pomodoro timer; streak, weekly goal, daily missions, XP, levels and badges; accounts with per-course access; remote WebDAV and SMB sources; desktop sync; backup download; update checks.

Non-negotiable constraints:
1. **Read-only library.** No action writes, moves, renames, deletes or copies original course files.
2. **Independent completion and position.** Marking a lesson complete never depends on where playback stopped.
3. **No marketplace mechanics.** No prices, ratings, enrolment, certificates, leaderboards, coins or cloud tracking.
4. **Privacy.** No third-party requests from the SPA, no telemetry, strict content security policy, Argon2id password hashes, random session tokens, nothing sensitive in browser storage.
5. **Offline desktop.** The desktop webview makes no network requests. Rust makes two: the update check and, only if the user links a server, sync.
6. **i18n.** Every user-facing string goes through `tr(en, ptBr)`.
7. **Light gamification.** Habit and visible progress come first. Badges are quiet achievements.

Terminology: *library* (everything), *course* (an imported folder), *module* (a group of lessons), *lesson* (one unit of media), *material* (a non-media file).

Undecided: weekly rest days that protect the streak need backend changes (issue #341). The yearly heatmap on Progress is shipped.

The learner can customise the accent colour, surface palette, corners, density, text size, font, lesson sidebar side and which habit cards show (Settings, stored only in this browser). The library view is a separate local preference set from the library toolbar.

## Brand Commitments

- Name **LearnVault**, set as a text wordmark in the display serif. The Academy Portico mark (`docs/design/branding/`) is for icons, the favicon and the header; see `DESIGN.md`.
- **Light and dark**: a warm off-white page, cream and white cards, warm-ink actions, Playfair Display titles and DM Sans text; a warm-black dark theme switched from the header or followed from the system. The learner can change the accent colour and other details. The maintainer chose this direction on 2026-11-11 from their reference dashboard; see `DESIGN.md`.
- Udemy, Coursera and similar platforms are a **structural reference only**. No code, assets, brand, text or pixel-level layout is copied.
- The language switch lives in Settings, not in the navigation bar.
- The result should read as a calm study room. It should not look like a SaaS dashboard, a course store, a children's game or a wall of metrics.

## Evidence on Hand

- `docs/design/research.md`, `decision.md` and `final-review.md` record reviews of earlier directions. No user research or user testing exists.
- There are no testimonials, customers or benchmarks, and none may be invented.
- Accessibility has automated coverage (axe on the main routes with every accent preset), but no screen-reader or keyboard audit and no WCAG certification.
- Demonstration courses are synthetic and generated locally. The repository holds no real course material.

## Product Principles

1. **Resume first.** The next lesson is one click from every screen that shows a course.
2. **The library is never touched.** Every feature is judged against the read-only rule before anything else.
3. **Habit over points.** Streaks and a weekly goal support a routine, and achievements stay quiet.
4. **Private by default.** The app needs no network beyond its own server and shows who can see what.
5. **One product, two shells.** Server and desktop behave the same wherever the platform allows.

## Accessibility & Inclusion

Target WCAG 2.2 AA: 4.5:1 text contrast with every accent preset on the light and dark page, 44×44 px touch targets, visible focus, reduced-motion support, keyboard operation of the player, and status changes announced without moving focus. Single-key shortcuts act only when the player has focus. Colour never carries information alone.
