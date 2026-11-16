# LearnVault Design System

The design record for the app (`apps/web-next`). It describes the system as built. Earlier directions (Stone + Emerald, Literata titles, the dark title band) are superseded by this file; `docs/design/` keeps their research as history.

## Intent

An editorial study room: warm paper, generous space and a serif voice, around the familiar structure of a course platform (library, course, lesson, progress). Calm and premium, never a generic dashboard. The next lesson and the learner's own habit do the visual work; there are no decorative graphics. Direction taken from the maintainer's reference (a warm off-white study dashboard with a tinted "next up" card, outlined pills and small stat cards). Course platforms are a structural reference only.

**Light, dark and surface palettes.** Light is the default (`warm` surface). Dark uses deep tinted charcoal tones (`#0B0A09` base on `warm`). The header has a sun/moon button; Settings offers Light, Dark and Follow the system alongside eight surface palettes (`warm`, `stone`, `crisp`, `sepia`, `sage`, `nord`, `rose`, `solarized`), each with light and dark pairs shown via dual-tone swatches. `<html data-theme>` and `<html data-surface>` carry the active mode and palette while `color-scheme` follows the resolved theme; `dark:` utilities read `data-theme`.

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
| `primary` / `--brand` | `#5C4229` (brown) | Buttons, active tabs, progress, the current step; the learner can change it |
| `reward-fill` | `#A96F2E` (amber) | Rewards and accent highlights (switches to `--brand` when matching accent is selected) |
| `destructive` | `#A33A32` | Destructive actions, solid fill |

**Dark values (`warm`):** `background` `#0B0A09`, `card` `#161311`, `panel` `#1E1A16`, `foreground` `#F4EFE6`, `muted-foreground` `#BFB5A7`, `border` `#38322B` (+ `16%` `--brand`). Every accent preset has a lighter twin for the dark page (brown becomes amber `#E3A857`); a custom colour is lightened until it reaches 4.5:1 on the dark panel colour (`#1C1510`), and text on it is `#1C1209`. Alternative surface palettes (`stone`, `crisp`, `sepia`, `sage`, `nord`, `rose`, `solarized`) override `--background`, `--foreground`, `--card`, `--muted`, `--muted-foreground`, `--panel` and `--border` cleanly in both light and dark modes.

`--brand-tint` (`12%` of `--brand`), `--brand-soft` (`22%` light / `28%` dark) and `--brand-deep` (`55%` `--brand` mixed with black) are mixed from the accent and the page colour, so any preset or live colour-picker choice tints navigation, surfaces, rings and borders consistently in both themes. On the light page text on the accent is white and `readableAccent()` darkens a colour until white text reaches 4.5:1. Rules: one accent per screen, no gradients as decoration (the generated cover is content), elevation shadows only when the learner explicitly selects `cardStyle: "elevated"`, state is never colour alone.

## Typography

| Role | Face | Use |
| --- | --- | --- |
| Display | Playfair Display (variable) | Page titles (`h1`) and the wordmark in the default editorial pairing (`sans`); panel, card and item titles use DM Sans |
| Interface and body | DM Sans (variable) | Everything else |
| Technical | JetBrains Mono (variable) | Timer clock, shortcuts |

Small labels above numbers are 12 px, bold, tracked and uppercase (`Eyebrow`). Fonts are bundled with the app; the server's CSP would block third-party fonts and the desktop build is offline. In Settings, the learner can choose `DM Sans with serif titles` (`sans`), `Clean sans-serif` (`modern`, DM Sans for headings too), `System font` (`system`), or `Reading serif` (`serif`, Literata for body copy).

## Shape and space

Wide and soft: cards and panels scale radius with `--radius` (`0.875rem` soft default; `square` and `round` overrides in Settings) and surface styling follows `cardStyle` (`bordered` default, subtle `elevated` shadow or borderless `soft` tint). Pages sit inside `--container-max` (Settings -> Page width: `72rem` Focused, `88rem` Standard default, `112rem` Wide) with a 40 px gutter on desktop (`20 px` on phones). Vertical rhythm scales with `--density` (`comfortable` default or `compact` across `[data-panel]`, `[data-card]` and `[data-row]`), and `reduceMotion` switches `[data-motion="reduced"]` to suppress non-essential transitions.

## Layout

- **Navigation.** A top bar with the wordmark (text, no logo tile), centred destinations (My learning, Review, Progress, Notes) with the active link tinted in `bg-brand-soft text-primary`, the search box (`/` focuses it), the Pomodoro timer, the theme button and the account menu. Below the `lg` breakpoint (1024 px) the destinations move to a frosted bottom tab bar (`bg-background/95 backdrop-blur-xs`). The language switch sits in Settings.
- **Page title.** An optional tinted `bg-brand-soft` icon tile (`ring-primary/20`), the heading title, optional subtitle description, and right-aligned actions (`PageHeader`).
- **Home.** Title and admin actions, outlined filter pills (counts only when above zero; one scrolling row on phones), then two columns in Grid view (`lg:grid-cols-[minmax(0,1fr)_18.5rem] xl:grid-cols-[minmax(0,1fr)_19.5rem]`). Left: a compact continue strip (a play tile, the lesson, its course and where you stopped, the course progress, Resume and Open course) above one toolbar row (search, folder/custom category filter when categories exist, sort and a labelled view switch: Grid, List, Compact, Shelf) and the library. Right: a 2×2 grid of compact habit cards (streak, level and XP, weekly goal, badges) and an optional Bookmarks panel (`showBookmarks`). Grid course cards keep metadata quiet: small square cover, title, hover/focus `...` menu (including category assignment), subtle category pill when `showCategories` is enabled, and the progress bar + Resume link shown when started and `showProgress` is enabled. List and compact views use the full width with a summary habit strip above flat rows. The course page's breadcrumb returns to the library with the filter and search the learner left.
- **Course.** A breadcrumb (My learning / course), then a tinted header card: a small cover on the left, the title, clickable category pill, meta line and progress in the middle, and actions on the right (Continue or Start, Favorite, Manage). "Last lesson" sits under the card. Tabs for Content, Materials and Notes; the curriculum opens on the module you are in, clipped cleanly (`overflow-hidden rounded-[1.25rem]`). Each module row has a numbered circle (a check when finished, a ring for the current one), its title, the number of lessons, and a status on the right: Completed, You are here or Not started.
- **Lesson.** A title row with Previous, Next and a Contents button that hides or shows the course content, one segment per lesson (done brown, current amber), then the player, one row of study actions (Mark as completed, Note, Card, Bookmark at the current time, Autoplay next), the tabs with their counts (Overview, Notes, Timestamps, Cards, Materials) and an Overview of three compact panels (your notes — rendered as clean plain text via `plainTextMarkdown()` without raw Markdown markers — timestamps, materials of this lesson with tabular counts) and an Up next card, beside a compact "Course content" panel (left or right, the learner's choice) whose modules are numbered with a count and a small bar. The panel has one hide/show control (the button in the title row, the only one that can also bring it back). The panel has a "Find a lesson" field that filters the lessons by title and opens the modules that match. The open lesson is the one emphasis in the panel: its module opens by itself, shows a ring on its number and a tinted row, the lesson row is tinted with a filled play icon and heavier text, and the panel scrolls to it; a finished module shows a check in place of its number. Module titles are sans; the panel is one card with flat dividers, no inner box. "Recently watched" lives on Home and History, not under the player. The player's controls (play, back and forward 10 s, volume, subtitles, speed, a settings menu, cinema mode, fullscreen; the settings menu holds Loop a section, picture-in-picture, transcoding and the system player, and an active loop stays visible as a chip) sit on the picture as a pill that appears on hover or focus and always on touch screens; under audio they are an ordinary toolbar. Cinema mode (`t`) gives the picture the whole width on a black stage and moves the content panel below; both choices are remembered.
- **Progress.** Three metrics on top, each with what it counts: lessons completed (in the courses you started), active days (against the weekly goal, with the streak and the busiest day) and focus time (only the Pomodoro timer counts); the This week / All time switch sits in the page header. Below, balanced 2-column rows (`Courses` beside `Today`, `Focus time by day` with its accessible `sr-only` data table beside `Badges` with compact badge tiles) and a full-width year `ActivityHeatmap` with 5 GitHub-style intensity levels (`0`–`4`), `Less ... More` legend and hover/focus tooltips showing date, lesson count, focus minutes and active day state.
- **Notes** start from the notes: every note of every course as a notebook. Each course is a serif heading with its note count, each lesson under it is a plain section (a hairline above, the lesson title and Open lesson), and each note hangs from its timestamp in the left margin (a link back to that moment), with the text set in Literata and the date under it; Edit and Delete show on hover, on focus and always on touch screens. A search box, a By lesson / By date switch, a course filter when notes come from more than one course, and Export as Markdown share one toolbar. `?course=` opens one course. **Review** starts from a course: a title, the pinned (favourite) courses, then every course as a card with `CourseCover`, a pin and a tinted footer that says what waits inside ("2 due · 8 cards"). A card opens the course's deck. A review session is a focused screen: back, deck title, a progress ring with cards left, the card with its number and Question/Answer label, a large Show answer button, then Again and Good.
- **Sign in.** One centred column on every screen size: the wordmark, the "Sign in" title, the username and password fields, the button and the language switch. No side panel, no marketing text; errors appear above the fields.
- **Targets.** Every interactive target is at least 44×44 px, including the PDF toolbar fields and colour swatches.
- **Cinema mode** shows its button from 1024 px up (below that the course content already sits under the picture). The Bookmarks panel on Home appears when study tracking is on and `showBookmarks` is enabled.

## Components

shadcn/ui on **Base UI** primitives (the only candidate that satisfies the server's `style-src` CSP; see ADR 0003), in `src/components/ui/`. Product components:

- `Panel`, `Hero`, `Stat`, `Ring`, `Eyebrow` (`components/panel.tsx`): the shared card vocabulary. Use these instead of one-off boxes.
- `CoursePicker` (the course-card grid of Review), `NoteEditor` and `Markdown` (notes with a formatting bar; note text is parsed to data and drawn by React, never as HTML), `CourseCover`, `CourseCard`, `CourseCategoryDialog`, `ActivityHeatmap`, `SwitchCard`, `Curriculum`, `MediaPlayer`, `StudyNotes`, `Flashcards`, `FocusMenu`, `DesktopTitlebar`.
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

One JSON object in `localStorage` (`learnvault-appearance`), validated on read and applied to `<html>` before first paint: `theme` (light, dark or follow the system), `surface` (8 surface palettes with light/dark swatches), `accent` (8 presets or live custom hex colour), `reward` colour, `radius` (`square`, `soft`, `round`), `cardStyle` (`bordered`, `elevated`, `soft`), `density` (`comfortable`, `compact`), `textSize` (`small`, `medium`, `large`), `pageWidth` (`focused`, `standard`, `wide`), `font` (`sans`, `modern`, `system`, `serif`), `reduceMotion`, `sidebar` (lesson sidebar side), `homeLayout` (default My learning view: `grid` or `list`), and visibility flags (`showContinue`, `showGoal`, `showStreak`, `showXp`, `showBadges`, `showBookmarks`, `showProgress`, `showCategories`). Everything has a default and can be reset. Course category overrides are persisted separately in `localStorage` (`learnvault-course-categories`, overriding `sourceFolder`), and the active library view toolbar state is managed by `learnvault-library-view`.

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
