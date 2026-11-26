# LearnVault Design System

The design record for the app in `apps/web-next`, as built. This file replaces earlier directions (Stone + Emerald, Literata titles, the dark title band). `docs/design/` keeps their research as history.

## Intent

Lamplight gives coursework the depth of a lecture desk: near-black green surfaces, bone-colored text, and a restrained amber emphasis on the next study action. The light theme translates the same hierarchy to sage paper and bronze. The LearnVault wordmark is text only. Use the [reference board](docs/references.md) for structural inspiration, not assets or copied layouts.

Dark Lamplight is the default for new profiles. Light, Dark and Follow the system remain available in Settings; existing saved choices are respected. The inline theme attributes are `data-theme` and `data-surface`.

## Colour

`apps/web-next/src/styles/app.css` owns the semantic surfaces. `lib/appearance.ts` owns accent presets and the swatches shown in Settings. Prefer semantic utilities such as `bg-card` and `text-muted-foreground` over raw colors in screen code.

| Role | Light Lamplight | Dark Lamplight |
| --- | --- | --- |
| Canvas | `#F1F3EF` | `#0D1210` |
| Navigation rail | `#E8EDE6` | `#101814` |
| Card | `#FFFFFF` | `#151E19` |
| Panel | `#E6EBE4` | `#1B2620` |
| Text | `#111914` | `#ECE7DA` |
| Secondary text | `#47534C` | `#AAB4AD` |
| Border | `#DCE2DA` | `#25322B` |
| Default accent (Lamp) | `#8A5A0C` | `#F0BE62` |

Use amber for prominent study actions and active progress. Use semantic status colors and a text or shape cue for state. Use spacing and surface contrast rather than decorative glow to distinguish panels. Other surfaces and accents remain selectable.

## Typography

Geist Variable is the default for titles, labels and body text. Use tabular figures for times and progress where alignment matters. Learners can still select system and reading fonts in Settings. Fonts are bundled for offline use; screens do not fetch typefaces from a third party.

## Brand

The identity is the Academy Portico mark and the *Sapere aude* motto.

**Mark.** The Academy Portico is retained for the app icon, favicon and repository branding. The app interface uses the LearnVault wordmark alone.

| Asset | Use |
| --- | --- |
| `docs/design/branding/learnvault-mark.svg`, `learnvault-mark-dark.svg` | The mark alone, on light and dark surfaces |
| `docs/design/branding/learnvault-icon.svg` | App icon: the full mark on a warm-ink tile. Source of `crates/desktop/icons/icon.png` and the 48 px and larger `icon.ico` sizes |
| `apps/web-next/public/favicon.svg` | The reduced cut on the tile. Source of the 16 px and 32 px `icon.ico` sizes |
| `docs/design/branding/learnvault-banner.svg`, `learnvault-banner-light.svg` | README header, text converted to outlines so it needs no font |
| `docs/design/branding/learnvault-seal.svg`, `learnvault-seal-dark.svg` | The mark inside a ring that reads LEARNVAULT · SAPERE AUDE, for large placements (72 px and up). Usage rules: [`docs/design/branding/README.md`](docs/design/branding/README.md) |

The icon and repository assets keep their own warm ink, cream and brass. In the app, Lamp is the default accent and saved preferences are respected.

**Motto.** *Sapere aude* appears on sign-in and Settings > About, with a translation and a Latin language tag.

## Shape and space

Cards and panels scale their radius with `--radius` (`0.875rem` for `soft`, the default; `square` and `round` in Settings) and surface styling follows `cardStyle` (`bordered` default, subtle `elevated` shadow or borderless `soft` tint). Pages sit inside `--container-max` (Settings -> Page width: `72rem` Focused, `88rem` Standard default, `112rem` Wide) with a 40 px gutter on desktop (`20 px` on phones). Vertical rhythm scales with `--density` (`comfortable` default or `compact` across `[data-panel]`, `[data-card]` and `[data-row]`), and `reduceMotion` switches `[data-motion="reduced"]` to suppress non-essential transitions.

## Layout

- **Navigation.** Desktop uses a left rail for the wordmark, search, primary destinations, and account utilities. Below 1024 px the wordmark and utilities sit above content and the primary destinations move to the bottom bar. The content region owns scrolling while the signed-in shell is active; sign-in and route errors remain scrollable outside it.
- **Library.** Up next leads when an unfinished lesson is available. Its artwork is desktop-only so the lesson action and catalog stay in view on phones. Filters, search, sort and the Grid / List / Compact / Shelf switch sit directly above the catalog. Grid cards use borderless artwork and metadata; habits and bookmarks follow the catalog instead of competing with it.
- **Course.** Course artwork, metadata and the study action lead into a tracklist curriculum. Separate completed lessons from the next unfinished one using both text and an indicator.
- **Lesson.** Give the player priority. Notes, Cards and Contents share one study rail on desktop and the same tabbed tools below the video on phones. Lesson actions stay reachable without interrupting playback.
- **Progress.** Three metrics on top, each with what it counts: lessons completed (in the courses you started), active days (against the weekly goal, with the streak and the busiest day) and focus time (only the Pomodoro timer counts); the This week / All time switch sits in the page header. Below, balanced 2-column rows (`Courses` beside `Today`, `Focus time by day` with its accessible `sr-only` data table beside `Badges` with compact badge tiles) and a full-width year `ActivityHeatmap` with 5 GitHub-style intensity levels (`0`–`4`), `Less ... More` legend and hover/focus tooltips showing date, lesson count, focus minutes and active day state.
- **Notes.** Organize notes by lesson or date, with search and course filtering. Notes use the UI sans and timestamps link back to the lesson. Editing and export stay secondary to the note itself. **Review** groups decks by course and keeps the question, answer and grading actions in one focused session.
- **Sign in.** One centred column at every size: the LearnVault wordmark, the title, username and password fields, the button, the *Sapere aude* motto and the language switch. There is no side panel or marketing text, and errors appear above the fields.
- **Targets.** Every interactive target is at least 44×44 px, including the PDF toolbar fields and colour swatches.
- **Cinema mode** shows its button from 1024 px up (below that the course content already sits under the picture). The Bookmarks panel on Home appears when study tracking is on and `showBookmarks` is enabled.

## Components

shadcn/ui on **Base UI** primitives in `src/components/ui/`. Base UI was the only candidate that satisfied the server's `style-src` CSP ([ADR 0003](docs/architecture/adr/0003-react-frontend.md)). Product components:

- `Panel`, `Hero`, `Stat`, `Ring`, `Eyebrow` (`components/panel.tsx`): the shared card vocabulary. Use these instead of one-off boxes.
- `CoursePicker` (the Review course grid), `NoteEditor` and `Markdown` (note text is parsed to data and drawn by React, never as HTML), `CourseCover`, `CourseCard`, `CourseCategoryDialog`, `ActivityHeatmap`, `SwitchCard`, `Curriculum`, `MediaPlayer`, `StudyNotes`, `Flashcards`, `FocusMenu`, `DesktopTitlebar`. `MediaPlayer`, `StudyNotes` and `Flashcards` live in `components/player/`, and `SwitchCard` in `pages/settings/`.
- **Course cover.** Show the available cover or detailed video frame when possible. A missing or near-uniform image falls back to a dark title card with a stable course hue. Small thumbnails use a single initial. The surrounding card supplies the accessible course name.
- **Progress.** A thin bar plus text ("42% complete · 3 lessons left"), `role="progressbar"` labelled by that text; the ring carries its value in `aria-label`.

## PDF reader and editor

A PDF lesson (or a PDF material) opens in the app and is never written. The page renders to a canvas with a selectable text layer; the reader adds:

- page, zoom and fit controls with keyboard shortcuts (arrows, Page Up/Down, Home/End, +/-);
- search across every page, with previous/next result and a spoken summary;
- marks on selected text: `h` highlights, `u` underlines, and the Note button adds a note. A labelled radio group offers four colours. `n` and `p` move between search results, or between marked pages when there are no results;
- a note is written in a small form before it is saved (the server refuses empty notes);
- a "Marks in this document" panel: show all or this page, edit each note, change its colour, jump to its page, delete it, or export every mark as Markdown.

Marks live in the database per user; the file and the library are untouched.

## Customisation

One JSON object in `localStorage` (`learnvault-appearance`), validated on read and applied to `<html>`. `public/theme-init.js` sets the theme before first paint and the app applies the rest on load. Keys: `theme` (light, dark or follow the system), `surface` (nine surface palettes with light/dark swatches), `accent` (ten presets, Lamp by default, or a live custom hex colour), `reward` colour, `radius` (`square`, `soft`, `round`), `cardStyle` (`bordered`, `elevated`, `soft`), `density` (`comfortable`, `compact`), `textSize` (`small`, `medium`, `large`), `pageWidth` (`focused`, `standard`, `wide`), `font` (`sans`, `modern`, `system`, `serif`), `reduceMotion`, `sidebar` (lesson sidebar side), `homeLayout` (default My learning view: `grid` or `list`), and visibility flags (`showContinue`, `showGoal`, `showStreak`, `showXp`, `showBadges`, `showBookmarks`, `showProgress`, `showCategories`). Everything has a default and can be reset. Course category overrides are persisted separately in `localStorage` (`learnvault-course-categories`, overriding `sourceFolder`), and the active library view toolbar state is managed by `learnvault-library-view`.

## Gamification

Streak, weekly goal, daily missions, XP and level, and quiet badges. They appear as small cards, never pop-ups, and each can be hidden. There are no leaderboards, coins, certificates or celebratory animation.

## Accessibility

Target: WCAG 2.2 AA. Automated gates are `eslint-plugin-jsx-a11y`, axe with every accent preset on the light and dark page (`tests/e2e/web-next.spec.ts`), and a check that fails on any CSP violation. Required behaviour: skip link and landmarks, visible focus, `prefers-reduced-motion`, live regions for status without moving focus, and single-key shortcuts only while their widget has focus. A manual keyboard and screen-reader pass is still owed. Checklist: [`docs/design/accessibility.md`](docs/design/accessibility.md).

## Security and privacy in the interface

No third-party requests, no HTML from user or file content (ESLint rejects `dangerouslySetInnerHTML`), nothing sensitive in browser storage, a generic sign-in error with rate-limit feedback, sign-out that clears per-user local data, and destructive dialogs that state their impact. Details: [`docs/design/security.md`](docs/design/security.md).

## Invariants

1. The interface never offers to move, rename, delete or write original course files.
2. Every user-facing string goes through `tr(en, ptBr)`.
3. Motion is short and shows a state change; nothing animates to decorate.
4. Missing media explains how to recover and never discards progress.
5. New screens use `Panel`, `Hero`, `Stat` and `PageHeader` before inventing a new container.
