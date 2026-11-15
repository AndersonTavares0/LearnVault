# LearnVault Design System

The design record for the app (`apps/web-next`). It describes the system as built. Earlier directions (Stone + Emerald, Literata titles, the dark title band) are superseded by this file; `docs/design/` keeps their research as history.

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
| Display | Playfair Display (variable) | Page titles (h1), the wordmark and big numbers; panel, card and item titles use DM Sans |
| Interface and body | DM Sans (variable) | Everything else |
| Technical | JetBrains Mono (variable) | Timer clock, shortcuts |

Page titles are plain Playfair, with no icon tile and no accent word. Small labels above numbers are 12 px, bold, tracked and uppercase (`Eyebrow`). Fonts are bundled with the app; the server's CSP would block third-party fonts and the desktop build is offline. The learner can switch to the system font or to a serif reading face.

## Shape and space

Wide and soft: panels and the hero use a 2rem radius, stat cards 1.75rem, pills are fully round, controls follow `--radius` (0.875rem by default; Corners in Settings changes it). Pages sit in a 72 rem column with a 40 px gutter on desktop and 20 px on phones. Vertical rhythm scales with `--density` (Settings: comfortable or compact).

## Layout

- **Navigation.** A top bar with the wordmark (text, no logo tile), centred destinations (My learning, Review, Progress, Notes) with the current page in a cream pill, the search box (`/` focuses it), the Pomodoro timer, the theme button and the account menu. Below the `lg` breakpoint (1024 px) the destinations move to a bottom tab bar. The language switch is in Settings.
- **Page title.** An optional icon tile, the serif title with its accent word, and an optional line under it (`PageHeader`).
- **Home.** Title and admin actions, outlined filter pills (counts only when above zero; one scrolling row on phones), then two columns. Left: a compact continue strip (a play tile, the lesson, its course and where you stopped, the course progress, Resume and Open course) above one toolbar row (search, category, sort and a labelled view switch: Covers, List, Compact, Shelf) and the library. Right: the habit cards (streak, level and XP, weekly goal, badges) and a Bookmarks panel. The library is a grid of title-first cards (a small square cover, the title in up to three lines, the progress bar and the favourite star) or a list, compact or shelf view. List and compact views use the whole width: the habit becomes one strip of five figures above the list, and each list row has columns for the course, its progress (plain text, no empty bar, until the course is started), when it was last studied and an action: Resume is a link straight to the lesson and the saved point, Open opens the curriculum (a course not started, finished or with no lessons is opened first, never started blindly). The course page's breadcrumb returns to the library with the filter and search the learner left.
- **Course.** A breadcrumb (My learning / course), then a tinted header card: a small cover on the left, the title, the meta line and progress in the middle, and the actions stacked on the right (Continue or Start, Favorite, Manage). "Last lesson" sits under the card. Tabs for Content, Materials and Notes; the curriculum opens on the module you are in. Each module row has a numbered circle (a check when finished, a ring for the current one), its title, the number of lessons, and a status on the right: Completed, You are here or Not started.
- **Lesson.** A title row with Previous, Next and a Contents button that hides or shows the course content, one segment per lesson (done brown, current amber), then the player, one row of study actions (Mark as completed, Note, Card, Bookmark at the current time, Autoplay next), the tabs with their counts (Overview, Notes, Timestamps, Cards, Materials) and an Overview of three small panels (your notes, timestamps, materials of this lesson) and an Up next card, beside a compact "Course content" panel (left or right, the learner's choice) whose modules are numbered with a count and a small bar. The panel has one hide/show control (the button in the title row, the only one that can also bring it back). The panel has a "Find a lesson" field that filters the lessons by title and opens the modules that match. The open lesson is the one emphasis in the panel: its module opens by itself, shows a ring on its number and a tinted row, the lesson row is tinted with a filled play icon and heavier text, and the panel scrolls to it; a finished module shows a check in place of its number. Module titles are sans; the panel is one card with flat dividers, no inner box. "Recently watched" lives on Home and History, not under the player. The player's controls (play, back and forward 10 s, volume, subtitles, speed, a settings menu, cinema mode, fullscreen; the settings menu holds Loop a section, picture-in-picture, transcoding and the system player, and an active loop stays visible as a chip) sit on the picture as a pill that appears on hover or focus and always on touch screens; under audio they are an ordinary toolbar. Cinema mode (`t`) gives the picture the whole width on a black stage and moves the content panel below; both choices are remembered.
- **Progress.** Three metrics on top, each with what it counts: lessons completed (in the courses you started), active days (against the weekly goal, with the streak and the busiest day) and focus time (only the Pomodoro timer counts); the This week / All time switch sits in the page header. Below, two columns of equal height. Left: focus time by day, a Courses panel with the courses under way and their bars, and the year heatmap. Right: today's missions and the badges, kept smaller than the study content.
- **Notes** start from the notes: every note of every course as a notebook. Each course is a serif heading with its note count, each lesson under it is a plain section (a hairline above, the lesson title and Open lesson), and each note hangs from its timestamp in the left margin (a link back to that moment), with the text set in Literata and the date under it; Edit and Delete show on hover, on focus and always on touch screens. A search box, a By lesson / By date switch, a course filter when notes come from more than one course, and Export as Markdown share one toolbar. `?course=` opens one course. **Review** starts from a course: a title, the pinned (favourite) courses, then every course as a card with its code, a pin and a tinted footer that says what waits inside ("2 due · 8 cards"). A card opens the course's deck. A review session is a focused screen: back, deck title, a progress ring with cards left, the card with its number and Question/Answer label, a large Show answer button, then Again and Good.
- **Sign in.** From 1024 px a tinted panel on the left (wordmark, the line "Your courses, where you left them." and three plain facts: resume in one click, course folders are read and never changed, data stays on your server) beside the form on the page colour; on a phone the form sits in one card under the wordmark. The language switch stays under the form. The account menu lists Profile, Watch history, Settings and Sign out.
- **Targets.** Every interactive target is at least 44×44 px, including the PDF toolbar fields and colour swatches.
- **Cinema mode** shows its button from 1024 px up (below that the course content already sits under the picture). The Bookmarks panel on Home appears when study tracking is on.

## Components

shadcn/ui on **Base UI** primitives (the only candidate that satisfies the server's `style-src` CSP; see ADR 0003), in `src/components/ui/`. Product components:

- `Panel`, `Hero`, `Stat`, `Ring`, `Eyebrow` (`components/panel.tsx`): the shared card vocabulary. Use these instead of one-off boxes.
- `CoursePicker` (the course-card grid of Review), `NoteEditor` and `Markdown` (notes with a formatting bar; note text is parsed to data and drawn by React, never as HTML), `CourseCover`, `CourseCard`, `Curriculum`, `MediaPlayer`, `StudyNotes`, `Flashcards`, `FocusMenu`, `DesktopTitlebar`.
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

One JSON object in `localStorage` (`learnvault-appearance`), validated on read and applied to `<html>` before first paint: theme (light, dark or follow the system), accent (8 presets or any colour), reward colour, corners, density, text size, font, lesson sidebar side, and which habit cards show. Everything has a default and can be reset. Library view is managed separately by `learnvault-library-view`; the legacy `homeLayout` field remains accepted for storage compatibility but is not a current control.

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
