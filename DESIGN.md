# LearnVault Design System

The design record for the app in `apps/web-next`, as built. This file replaces earlier directions (Stone + Emerald, Literata titles, the dark title band). `docs/design/` keeps their research as history.

## Intent

Use warm off-white surfaces, generous spacing and serif headings around the familiar structure of a course platform (library, course, lesson, progress). The library puts the next lesson in a tinted card and uses outlined pills and compact study statistics, with no decorative graphics. The direction comes from the maintainer's reference dashboard. Course platforms are a structural reference only.

**Light, dark and surface palettes.** Light is the default (`warm` surface). Dark uses tinted charcoal (`#0B0A09` base on `warm`). The header has a sun/moon button. Settings offers Light, Dark and Follow the system, plus eight surface palettes (`warm`, `stone`, `crisp`, `sepia`, `sage`, `nord`, `rose`, `solarized`) with light and dark pairs shown as dual-tone swatches. `<html data-theme>` and `<html data-surface>` carry the active mode and palette while `color-scheme` follows the resolved theme; `dark:` utilities read `data-theme`.

## Colour

Tokens live once in `src/styles/app.css` as shadcn variables, mapped by Tailwind's `@theme inline`.

| Role | Default Light (`warm`) | Use |
| --- | --- | --- |
| `background` | `#FBF7F1` (+ `6%` `--brand`) | Page |
| `card` / `popover` | `#FFFFFF` (+ `3%` `--brand`) | Panels, menus, dialogs |
| `panel` (`secondary`) | `11%` `--brand` over `--background` | The tinted hero, stat cards, selected pills |
| `muted` | `#F3E9DB` (+ `4%` `--brand`) | Quiet fills |
| `foreground` | `#2E2118` | Text |
| `muted-foreground` | `#6A5745` | Secondary text (above 4.5:1 on every surface) |
| `border` | `#E5DDD2` (+ `16%` `--brand`) | Subtle hairlines |
| `input` | `#8C7F6C` | Form-field outline (3:1 non-text contrast) |
| `primary` / `--brand` | `#1C1815` (warm ink) | Buttons, active tabs, progress, the current step; the learner can change it (Ink is the first preset, Brown and the others remain) |
| `reward-fill` | `#A8761F` (brass) | Rewards, accent highlights and the door of the mark (switches to `--brand` when matching accent is selected). Used as a graphic fill, so it needs 3:1, which it has on every surface |
| `reward` | `#795514` (dark brass) | Small reward text on `reward-tint`; uses gold in dark mode and `--brand-ink` when rewards match the accent |
| `destructive` | `#A33A32` | Destructive actions, solid fill |

**Dark values (`warm`):** `background` `#0B0A09`, `card` `#161311`, `panel` `#1E1A16`, `foreground` `#F4EFE6`, `muted-foreground` `#BFB5A7`, `border` `#38322B` (+ `16%` `--brand`). Every accent preset has a lighter twin for the dark page (ink becomes gold `#E0A83E`, brown becomes amber `#E3A857`); a custom colour is lightened until it reaches 4.5:1 on the dark panel colour (`#1C1510`), and text on it is `#1C1209`. Alternative surface palettes (`stone`, `crisp`, `sepia`, `sage`, `nord`, `rose`, `solarized`) override `--background`, `--foreground`, `--card`, `--muted`, `--muted-foreground`, `--panel` and `--border` in both modes. Each surface's muted text is checked against the darkest tint any accent produces on it and stays above 4.5:1. For that reason `rose` and `solarized` use darker muted text on the light page than their source palettes (`#5E5974` and `#4C6067`).

`--brand-tint` (`12%` of `--brand`), `--brand-soft` (`22%` light / `28%` dark) and `--brand-deep` (`55%` `--brand` mixed with black) are mixed from the accent and the page colour, so any preset or custom colour tints navigation, surfaces, rings and borders in both themes. On the light page text on the accent is white and `readableAccent()` darkens a colour until white text reaches 4.5:1. Rules: one accent per screen, no gradients as decoration (the generated cover is content), elevation shadows only when the learner explicitly selects `cardStyle: "elevated"`, state is never colour alone.

## Typography

| Role | Face | Use |
| --- | --- | --- |
| Display | Playfair Display (variable) | Page titles (`h1`) and the wordmark in the default editorial pairing (`sans`); panel, card and item titles use DM Sans |
| Interface and body | DM Sans (variable) | Everything else |
| Technical | JetBrains Mono (variable) | Timer clock, shortcuts |

`Eyebrow` is a small muted label (`text-sm`, medium weight). Fonts are bundled because the server CSP would block third-party fonts and the desktop build is offline. In Settings, the learner can choose `DM Sans with serif titles` (`sans`), `Clean sans-serif` (`modern`, DM Sans for headings too), `System font` (`system`), or `Reading serif` (`serif`, Literata for body copy).

## Brand

The identity is the Academy Portico mark and the *Sapere aude* motto.

**Mark.** The Academy Portico: a Greek school front (pediment, four columns, steps) with a gold arched door, the vault. The stone is one neutral colour and the door and the pediment dot are the single accent, so the mark never competes with the learner's own accent colour. In the app `BrandMark` (`components/brand.tsx`) draws the stone in `currentColor` (always pass `text-foreground`: cream on the dark page, ink on the light one) and the door in `--reward-fill`. `compact` is the reduced cut (two columns, a wider door) for 24 px and below; the full mark is for 32 px and up.

| Asset | Use |
| --- | --- |
| `docs/design/branding/learnvault-mark.svg`, `learnvault-mark-dark.svg` | The mark alone, on light and dark surfaces |
| `docs/design/branding/learnvault-icon.svg` | App icon: the full mark on a warm-ink tile. Source of `crates/desktop/icons/icon.png` and the 48 px and larger `icon.ico` sizes |
| `apps/web-next/public/favicon.svg` | The reduced cut on the tile. Source of the 16 px and 32 px `icon.ico` sizes |
| `docs/design/branding/learnvault-banner.svg`, `learnvault-banner-light.svg` | README header, text converted to outlines so it needs no font |
| `docs/design/branding/learnvault-seal.svg`, `learnvault-seal-dark.svg` | The mark inside a ring that reads LEARNVAULT · SAPERE AUDE, for large placements (72 px and up). Usage rules: [`docs/design/branding/README.md`](docs/design/branding/README.md) |

Brand colours: warm ink `#1C1815`, cream `#F5EEE2`, gold `#E0A83E` on dark surfaces and `#A8761F` on light ones. The assets use them directly. The app's reward colour uses the same golds, and its default accent (the Ink preset) is the logo's ink. The page, panels and tints are mixed from `--brand-tone`, which is the accent itself except for Ink on the light page, where it stays brown: a near-black accent would darken every tint (the muted text fell to 4.3:1 on panels) and, with almost no chroma, turn them pink in `oklch`. A learner who saved another accent keeps it.

**Motto.** *Sapere aude* ("dare to know", Horace, *Epistles*; Kant, 1784) sits beside the mark on the sign-in page and in Settings > About, never inside the mark. Three other lines appear in context. Each has a translation through `tr(en, ptBr)`, each screen shows one, and none rotates. `Motto` (`components/brand.tsx`) sets the quotation in italic with its `lang` attribute and the translation in the learner's language.

| Where | Line | English | Portuguese |
| --- | --- | --- | --- |
| Sign in, About | *Sapere aude* | Dare to know | Ouse saber |
| Progress | *Nulla dies sine linea* | Not a day without a line | Nenhum dia sem uma linha |
| Import a course | *Ad fontes* | Back to the sources | De volta às fontes |
| Empty library | *ἀρχὴ ἥμισυ παντός* | The beginning is half of the whole | O começo é metade do todo |

Greek is set in Literata (`font-reading`), because Playfair Display has no Greek glyphs. The Portuguese lines are the maintainers' own translations.

## Shape and space

Cards and panels scale their radius with `--radius` (`0.875rem` for `soft`, the default; `square` and `round` in Settings) and surface styling follows `cardStyle` (`bordered` default, subtle `elevated` shadow or borderless `soft` tint). Pages sit inside `--container-max` (Settings -> Page width: `72rem` Focused, `88rem` Standard default, `112rem` Wide) with a 40 px gutter on desktop (`20 px` on phones). Vertical rhythm scales with `--density` (`comfortable` default or `compact` across `[data-panel]`, `[data-card]` and `[data-row]`), and `reduceMotion` switches `[data-motion="reduced"]` to suppress non-essential transitions.

## Layout

- **Navigation.** A top bar with the Academy Portico mark and the wordmark, centred destinations (My learning, Review, Progress, Notes) with the active link tinted, the search box (`/` focuses it), the Pomodoro timer, the theme button and the account menu (Settings, Watch history, sign out). Below the `lg` breakpoint (1024 px) the destinations move to a frosted bottom tab bar (`bg-background/95 backdrop-blur-xs`). The language switch sits in Settings. The bar is pinned and the region under it owns the page scroll (`app-scroll`), so the scrollbar starts below the bar. The update and indexing banners stay visible with it. The document stops scrolling only while the shell is on screen (`data-app-shell` on `<html>`). The sign-in and route-error pages sit outside the shell and keep a scrollable document, so controls stay reachable on short or zoomed viewports.
- **Page title.** `PageHeader`: an optional tinted icon tile, the title, an optional description, and right-aligned actions.
- **Home.** Title and admin actions, outlined filter pills (counts only when above zero; one scrolling row on phones), then two columns in Grid view (`lg:grid-cols-[minmax(0,1fr)_18.5rem] xl:grid-cols-[minmax(0,1fr)_19.5rem]`). Left: a compact continue strip (a play tile, the lesson, its course and where you stopped, the course progress, Resume and Open course) above one toolbar row (search, folder/custom category filter when categories exist, sort and a labelled view switch: Grid, List, Compact, Shelf) and the library. Right: a 2×2 grid of compact habit cards (streak, level and XP, weekly goal, badges) and an optional Bookmarks panel (`showBookmarks`). Grid course cards keep metadata quiet: small square cover, title, hover/focus `...` menu (including category assignment), subtle category pill when `showCategories` is enabled, and the progress bar + Resume link shown when started and `showProgress` is enabled. List and compact views use the full width with a summary habit strip above flat rows. The course page's breadcrumb returns to the library with the filter and search the learner left.
- **Course.** A breadcrumb (My learning / course), then a tinted header card: a small cover on the left, the title, clickable category pill, meta line and progress in the middle, and actions on the right (Continue or Start, Favorite, Manage). "Last lesson" sits under the card. Tabs for Content, Materials and Notes; the curriculum opens on the module you are in, clipped cleanly (`overflow-hidden rounded-[1.25rem]`). Each module row has a numbered circle (a check when finished, a ring for the current one), its title, the number of lessons, and a status on the right: Completed, You are here or Not started.
- **Lesson.** A title row with Previous, Next and a Contents button that hides or shows the course content, one segment per lesson (done brown, current amber), then the player, one row of study actions (Mark as completed, Note, Card, Bookmark at the current time, Autoplay next), the tabs with their counts (Overview, Notes, Timestamps, Cards, Materials) and an Overview of three compact panels (your notes as plain text without Markdown markers, timestamps, and this lesson's materials with counts) and an Up next card, beside a compact "Course content" panel (left or right, the learner's choice) whose modules are numbered with a count and a small bar. The panel has one hide/show control (the button in the title row, the only one that can also bring it back). The panel has a "Find a lesson" field that filters the lessons by title and opens the modules that match. The open lesson is the panel's one emphasis: its module opens by itself and shows a ring on its number, its row is tinted with a filled play icon and heavier text, and the panel scrolls to it. A finished module shows a check in place of its number. Module titles are sans, and the panel is one card with flat dividers. "Recently watched" lives on Home and History, not under the player. The player's controls (play, back and forward 10 s, volume, subtitles, speed, a settings menu, cinema mode, fullscreen; the settings menu holds Loop a section, picture-in-picture, transcoding and the system player, and an active loop stays visible as a chip) sit on the picture as a pill that appears on hover or focus and always on touch screens; under audio they are an ordinary toolbar. Cinema mode (`t`) gives the picture the whole width on a black stage and moves the content panel below; both choices are remembered.
- **Progress.** Three metrics on top, each with what it counts: lessons completed (in the courses you started), active days (against the weekly goal, with the streak and the busiest day) and focus time (only the Pomodoro timer counts); the This week / All time switch sits in the page header. Below, balanced 2-column rows (`Courses` beside `Today`, `Focus time by day` with its accessible `sr-only` data table beside `Badges` with compact badge tiles) and a full-width year `ActivityHeatmap` with 5 GitHub-style intensity levels (`0`–`4`), `Less ... More` legend and hover/focus tooltips showing date, lesson count, focus minutes and active day state.
- **Notes** start from the notes: every note of every course as a notebook. Each course is a serif heading with its note count, each lesson under it is a plain section (a hairline above, the lesson title and Open lesson), and each note hangs from its timestamp in the left margin (a link back to that moment), with the text set in Literata and the date under it; Edit and Delete show on hover, on focus and always on touch screens. A search box, a By lesson / By date switch, a course filter when notes come from more than one course, and Export as Markdown share one toolbar. `?course=` opens one course. **Review** starts from a course: a title, the pinned (favourite) courses, then every course as a card with `CourseCover`, a pin and a tinted footer that says what waits inside ("2 due · 8 cards"). A card opens the course's deck. A review session is a focused screen: back, deck title, a progress ring with cards left, the card with its number and Question/Answer label, a large Show answer button, then Again and Good.
- **Sign in.** One centred column at every size: the mark and wordmark, the title, username and password fields, the button, the *Sapere aude* motto and the language switch. There is no side panel or marketing text, and errors appear above the fields.
- **Targets.** Every interactive target is at least 44×44 px, including the PDF toolbar fields and colour swatches.
- **Cinema mode** shows its button from 1024 px up (below that the course content already sits under the picture). The Bookmarks panel on Home appears when study tracking is on and `showBookmarks` is enabled.

## Components

shadcn/ui on **Base UI** primitives in `src/components/ui/`. Base UI was the only candidate that satisfied the server's `style-src` CSP ([ADR 0003](docs/architecture/adr/0003-react-frontend.md)). Product components:

- `Panel`, `Hero`, `Stat`, `Ring`, `Eyebrow` (`components/panel.tsx`): the shared card vocabulary. Use these instead of one-off boxes.
- `CoursePicker` (the Review course grid), `NoteEditor` and `Markdown` (note text is parsed to data and drawn by React, never as HTML), `CourseCover`, `CourseCard`, `CourseCategoryDialog`, `ActivityHeatmap`, `SwitchCard`, `Curriculum`, `MediaPlayer`, `StudyNotes`, `Flashcards`, `FocusMenu`, `DesktopTitlebar`. `MediaPlayer`, `StudyNotes` and `Flashcards` live in `components/player/`, and `SwitchCard` in `pages/settings/`.
- **Course cover.** The order is uploaded cover, then automatic video thumbnail, then a generated cover: pale paper (deep warm brown on the dark page) with the course initials set large. A cover shows no title or progress, since the card around it does. Covers are decorative.
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

One JSON object in `localStorage` (`learnvault-appearance`), validated on read and applied to `<html>`. `public/theme-init.js` sets the theme before first paint and the app applies the rest on load. Keys: `theme` (light, dark or follow the system), `surface` (8 surface palettes with light/dark swatches), `accent` (9 presets, Ink by default, or a live custom hex colour), `reward` colour, `radius` (`square`, `soft`, `round`), `cardStyle` (`bordered`, `elevated`, `soft`), `density` (`comfortable`, `compact`), `textSize` (`small`, `medium`, `large`), `pageWidth` (`focused`, `standard`, `wide`), `font` (`sans`, `modern`, `system`, `serif`), `reduceMotion`, `sidebar` (lesson sidebar side), `homeLayout` (default My learning view: `grid` or `list`), and visibility flags (`showContinue`, `showGoal`, `showStreak`, `showXp`, `showBadges`, `showBookmarks`, `showProgress`, `showCategories`). Everything has a default and can be reset. Course category overrides are persisted separately in `localStorage` (`learnvault-course-categories`, overriding `sourceFolder`), and the active library view toolbar state is managed by `learnvault-library-view`.

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
