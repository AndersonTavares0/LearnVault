# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

LearnVault ships two shells around one React single-page app: a self-hosted server build (browser) and an offline Tauri desktop build (webview). Both are `web`; the desktop shell adds a native folder picker and window controls but keeps the same design language.

## Users

Self-directed learners (developers, researchers, hobbyists) who study from course folders they already own: video and audio lessons with subtitles and support files such as PDFs. They study in long evening sessions at a desk, often in short daily visits that must resume exactly where they stopped.

Two setups, both confirmed:
- **Server:** one household or small team on a home or private network. Several local accounts, each with its own progress and per-course access. An administrator imports courses and manages accounts.
- **Desktop:** one person, fully offline, no accounts.

The job: get back into the next lesson in one step, study with timestamped notes and flashcards, and keep a steady habit without reorganizing any files.

## Product Purpose

LearnVault turns folders of course material into a study space with the structure people expect from a course platform: a library, courses with a curriculum, a lesson player, progress, notes and flashcards. Success is a learner who opens the app, resumes the right lesson in one click, and keeps coming back.

## Positioning

The course-platform experience for courses you already own, with nothing leaving your machine. A hosted platform cannot truthfully offer a library it never touches, accounts it does not hold, and an offline desktop build with no network listeners.

## Operating Context

- Source material lives on the host filesystem as folders of modules and lessons. LearnVault reads it and never changes it.
- The catalog is built by an explicit scan. Rescans add new items, confirm existing ones and mark vanished ones as missing without erasing study history.
- Server: Rust (Axum) with SQLite, cookie sessions, media streaming with range requests and optional on-demand transcoding. Desktop: Tauri 2 with local IPC and custom media schemes.
- Docker, Podman and Compose are optional deployment adapters, never a runtime requirement.
- The interface is English by default with Brazilian Portuguese fully supported; the language is chosen in Settings.

## Capabilities and Constraints

Confirmed functionality: library and search; course page with curriculum and materials; lesson player with subtitles, speed, picture-in-picture, shortcuts and a resume position; timestamped notes; spaced-repetition flashcards; favourites; Pomodoro timer; streak, weekly goal, daily missions, XP, levels and achievement badges; accounts with per-course access; backup; update checks.

Non-negotiable constraints:
1. **Read-only library.** No action writes, moves, renames, deletes or copies original course files.
2. **Independent completion and position.** Marking a lesson complete never depends on where playback stopped.
3. **No marketplace mechanics.** No prices, ratings, enrolment, certificates, leaderboards, coins or cloud tracking.
4. **Privacy.** No third-party requests from the app, no telemetry, strict content security policy, hashed passwords, random session tokens, nothing sensitive in browser storage.
5. **Offline desktop.** The desktop bundle makes no network requests.
6. **i18n.** Every user-facing string goes through `tr(en, ptBr)`.
7. **Light gamification.** Habit and visible progress come first. Badges are quiet achievements, not a game layer.

Terminology: *library* (everything), *course* (an imported folder), *module* (a group of lessons), *lesson* (one unit of media), *material* (a non-media file).

Undecided: a yearly study heatmap and rest days need backend changes (issue #341); the final visual direction is still being chosen.

## Brand Commitments

- Name **LearnVault** and the LV monogram (`docs/design/branding/`).
- Default palette **Stone + Emerald**, with **Zinc + Teal** and **Zinc + Indigo** selectable in Settings, each in light and dark. Chosen by the maintainer, 2026-11-09.
- Udemy, Coursera and similar course platforms are a **structural reference only**. Nothing is copied: no code, assets, brand, text or pixel-level layout.
- The language switch lives in Settings, not in the navigation bar.
- The result must not read as a generic SaaS dashboard, a course store, a children's game or a dated site.

## Evidence on Hand

- `docs/design/research.md`, `decision.md` and `final-review.md`: design reviews of earlier directions. No user research and no user testing exist.
- No testimonials, customers or benchmarks exist, and none may be invented.
- Accessibility has automated coverage (axe on every route, in every palette and theme) but no screen-reader or keyboard audit and no WCAG certification.
- Demonstration courses are synthetic and generated locally; the repository holds no real course material.

## Product Principles

1. **Resume first.** The next lesson is one click from every screen that shows a course.
2. **The library is never touched.** Every feature is judged against the read-only rule before anything else.
3. **Habit over points.** Streaks and a weekly goal support a routine; achievements stay quiet.
4. **Private by default.** The app works with no network beyond its own server, and shows who can see what.
5. **One product, two shells.** Server and desktop behave the same wherever the platform allows.

## Accessibility & Inclusion

Target WCAG 2.2 AA: 4.5:1 text contrast in every palette and theme, 44×44 px touch targets, visible focus, reduced-motion support, keyboard operation of the player, and status changes announced without moving focus. Single-key shortcuts only act when the player has focus. Information is never carried by colour alone.
