# LearnVault Design System

The design record for the React app (`apps/web-next`). It describes the system as built. The Svelte app in `apps/web` keeps shipping until the cutover and follows the older tokens in its own CSS. Earlier directions (Stone + Emerald, Literata titles, the dark title band) are superseded by this file; `docs/design/` keeps their research as history.

## Intent

An editorial study room: warm paper, generous space and a serif voice, around the familiar structure of a course platform (library, course, lesson, progress). Calm and premium, never a generic dashboard. The next lesson and the learner's own habit do the visual work; there are no decorative graphics. Direction taken from the maintainer's reference (a warm off-white study dashboard with a tinted "next up" card, outlined pills and small stat cards). Course platforms are a structural reference only.

**Light and dark.** Light is the default. Dark is a warm black in the same voice (reference: the maintainer's dark study dashboard). The header has a sun/moon button; Settings offers Light, Dark and Follow the system. `<html data-theme>` carries the resolved theme and `color-scheme` follows it; `dark:` utilities read `data-theme`.

## Colour

Tokens live once in `src/styles/app.css` as shadcn variables, mapped by Tailwind's `@theme inline`.

| Role | Value | Use |
| --- | --- | --- |
| `background` | `#FDF9F6` | Page |
| `card` / `popover` | `#FFFFFF` | Panels, menus, dialogs |
| `panel` (`secondary`) | `#FBEFDC` | The tinted hero, stat cards, selected pills |
| `muted` | `#F8EFE3` | Quiet fills |
| `foreground` | `#3A2515` | Text |
| `muted-foreground` | `#735F4A` | Secondary text (above 4.5:1 on every surface) |
| `border` | `#ECE2D6` | Warm hairlines |
| `input` | `#8C7F6C` | Form-field outline (3:1 non-text contrast) |
| `primary` / `--brand` | `#5C4229` (brown) | Buttons, progress, the current step; the learner can change it |
| `reward-fill` | `#BC7F29` (amber) | The accent word in titles and rewards; 3.2:1, used for large text and fills only |
| `destructive` | `#B91C1C` | Destructive actions, solid fill |

**Dark values:** `background` `#090705`, `card` `#13100D`, `panel` `#1C1510`, `foreground` `#F6EEE4`, `muted-foreground` `#B6A694`, `border` `#2F251C`. Every accent preset has a lighter twin for the dark page (brown becomes amber `#E3A857`); a custom colour is lightened until it reaches 4.5:1 on the panel colour, and text on it is `#1C1209`.

`--brand-tint`, `--brand-soft` and `--brand-deep` are mixed from the accent and the page colour, so a custom accent keeps the whole interface consistent in both themes. On the light page text on the accent is white and `readableAccent()` darkens a colour until white text reaches 4.5:1. Rules: one accent per screen, no gradients as decoration (the generated cover is content), no decorative shadows, state is never colour alone.

## Typography

| Role | Face | Use |
| --- | --- | --- |
| Display | Playfair Display (variable) | Page titles, card and panel titles, big numbers |
| Interface and body | DM Sans (variable) | Everything else |
| Technical | JetBrains Mono (variable) | Timer clock, shortcuts |

Page titles carry the accent: the last word is set in italic amber (`<em>`), as in "My *learning*". Small labels above numbers are 12 px, bold, tracked and uppercase (`Eyebrow`). Fonts are bundled with the app; the server's CSP would block third-party fonts and the desktop build is offline. The learner can switch to the system font or to a serif reading face.

## Shape and space

Wide and soft: panels and the hero use a 2rem radius, stat cards 1.75rem, pills are fully round, controls follow `--radius` (0.875rem by default; Corners in Settings changes it). Pages sit in a 72 rem column with a 40 px gutter on desktop and 20 px on phones. Vertical rhythm scales with `--density` (Settings: comfortable or compact).

## Layout

- **Navigation.** A top bar with the wordmark (text, no logo tile), centred destinations (My learning, Review, Progress, Notes) with the current page in a cream pill, the search box (`/` focuses it), the Pomodoro timer, the theme button and the account menu. Below 768 px the destinations move to a bottom tab bar. The language switch is in Settings.
- **Page title.** An optional icon tile, the serif title with its accent word, and an optional line under it (`PageHeader`).
- **Home.** Title and admin actions, outlined filter pills (counts only when above zero; one scrolling row on phones), then two columns: the "Next up" hero (lesson, course, Resume, a progress ring) above the library, and beside them the habit cards (streak, level and XP, weekly goal) and a Bookmarks panel with starred lessons. The library is a grid of course cards or a list.
- **Course.** A tinted header card (title, progress, Continue, favourite, cover), then tabs for Content, Materials and Notes.
- **Lesson.** A title row with Previous, Next and a hide/show button for the course content, one segment per lesson (done brown, current amber), then the player and its tabs beside a compact "Course content" panel (left or right, the learner's choice) whose modules are numbered with a count and a small bar. The player's controls (back and forward 10 s, loop, speed, transcode, picture-in-picture, cinema mode, fullscreen) sit on the picture as a pill that appears on hover or focus and always on touch screens; under audio they are an ordinary toolbar. Cinema mode (`t`) gives the picture the whole width on a black stage and moves the content panel below; both choices are remembered.
- **Progress.** One usage panel: the focus time as a big number with This week / All time, the weekly goal, a smooth area chart of focus minutes by day (with a table for screen readers) and three totals (busiest day, days studied, streak); badges and today's missions in panels below.
- **Notes and Review** start from a course: a title, the pinned (favourite) courses, then every course as a card with its code, a pin and a tinted footer that says what waits inside ("3 notes", "2 due · 8 cards"). A card opens the course's notebook or its deck. A review session is a focused screen: back, deck title, a progress ring with cards left, the card with its number and Question/Answer label, a large Show answer button, then Again and Good.
- **Targets.** Every interactive target is at least 44×44 px, including the PDF toolbar fields and colour swatches.
- **Cinema mode** shows its button from 1024 px up (below that the course content already sits under the picture). The Bookmarks panel on Home appears when study tracking is on.

## Components

shadcn/ui on **Base UI** primitives (the only candidate that satisfies the server's `style-src` CSP; see ADR 0003), in `src/components/ui/`. Product components:

- `Panel`, `Hero`, `Stat`, `Ring`, `Eyebrow` (`components/panel.tsx`): the shared card vocabulary. Use these instead of one-off boxes.
- `CoursePicker` (the course-card grid shared by Notes and Review), `NoteEditor` and `Markdown` (notes with a formatting bar; note text is parsed to data and drawn by React, never as HTML), `CourseCover`, `CourseCard`, `Curriculum`, `MediaPlayer`, `StudyNotes`, `Flashcards`, `FocusMenu`, `DesktopTitlebar`.
- **Course cover.** Uploaded cover, then an automatic video thumbnail, then a generated cover: pale paper (deep warm brown on the dark page) with the course's initials set large. It carries no title or progress, because the card beside it shows both. Covers are decorative.
- **Progress.** A thin bar plus text ("42% complete · 3 lessons left"), `role="progressbar"` labelled by that text; the ring carries its value in `aria-label`.

## PDF reader and editor

A PDF lesson (or a PDF material) opens in the app and is never written. The page renders to a canvas with a selectable text layer; the reader adds:

- page, zoom and fit controls with keyboard shortcuts (arrows, Page Up/Down, Home/End, +/-);
- search across every page, with previous/next result and a spoken summary;
- marks on selected text: highlight (`h`), underline (`u`) and note (`n`), in four colours chosen with a labelled radio group;
- a note is written in a small form before it is saved (the server refuses empty notes);
- a "Marks in this document" panel: show all or this page, edit each note, change its colour, jump to its page, delete it, or export every mark as Markdown.

Marks live in the database per user; the file and the library are untouched.

## Customisation

One JSON object in `localStorage` (`learnvault-appearance`), validated on read and applied to `<html>` before first paint: theme (light, dark or follow the system), accent (8 presets or any colour), reward colour, corners, density, text size, font, lesson sidebar side, library layout, and which habit cards show. Everything has a default and can be reset.

## Gamification

Habit first: streak, weekly goal, daily missions, XP and level, and quiet badges. They appear as small cards, never as pop-ups, and each can be hidden. No leaderboards, coins, certificates or celebratory animation.

## Accessibility

WCAG 2.2 AA. Automated gates: `eslint-plugin-jsx-a11y`, axe with every accent preset on both the light and the dark page (`tests/e2e/web-next.spec.ts`), and a check that fails on any CSP violation. Required behaviour: skip link and landmarks, visible focus, `prefers-reduced-motion`, live regions for status without moving focus, single-key shortcuts only when their widget has focus. A manual keyboard and screen-reader pass is still owed. Checklist: [`docs/design/accessibility.md`](docs/design/accessibility.md).

## Security and privacy in the interface

No third-party requests, no HTML from user or file content (`react/no-danger`), nothing sensitive in browser storage, a generic sign-in error with rate-limit feedback, sign-out that clears per-user local data, destructive dialogs that state their impact. Details: [`docs/design/security.md`](docs/design/security.md).

## Invariants

1. The interface never offers to move, rename, delete or write original course files.
2. Every user-facing string goes through `tr(en, ptBr)`.
3. Motion is short and shows a state change; nothing animates to decorate.
4. Missing media explains how to recover and never discards progress.
5. New screens use `Panel`, `Hero`, `Stat` and `PageHeader` before inventing a new container.
