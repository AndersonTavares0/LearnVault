# LearnVault Design System

The design record for the React app (`apps/web-next`, epic #347). It describes the system as built; the screen-by-screen spec is [`docs/design/redesign.md`](docs/design/redesign.md). The Svelte app in `apps/web` keeps shipping until the cutover (#340) and follows the older tokens in its own CSS.

**Status.** The visual direction for the next iteration is being re-chosen (Progress, My learning and Notes were judged generic). This file is rewritten from the built result when that work finishes; the palette, type, accessibility and security rules below are settled and do not wait for it.

## Intent

A private course shelf: the familiar structure of a course platform (library, course, lesson, progress) for courses the learner already owns. Calm, durable and content-first, with course covers and the next lesson doing the visual work. Udemy and Coursera are structural references only.

## Colour

Colour is shadcn/ui's theme variables over Tailwind's palette, in OKLCH, with light and dark values. A palette is a `data-palette` value on `<html>` (`emerald` default, `teal`, `indigo`), stored per device in `learnvault-palette`. `data-theme` (`system`, `light`, `dark`) is independent.

| Palette | Neutral | Action, light | Action, dark | Default |
| --- | --- | --- | --- | --- |
| Stone + Emerald | stone | emerald-700 | emerald-400 | yes |
| Zinc + Teal | zinc | teal-700 | teal-400 | |
| Zinc + Indigo | zinc | indigo-600 | indigo-400 | |

Semantic roles (all derived from the palette, defined once in `src/styles/app.css`):

- `background`, `foreground`, `card`, `popover`, `secondary`, `muted`, `accent`, `border`, `input`: neutral steps.
- `primary` (and `ring`): the palette's action colour. It marks what can be done and where progress stands, nothing else.
- `muted-foreground`: neutral-600 in light mode, so secondary text keeps a safe margin above 4.5:1.
- `destructive`: red-700 in light mode, red-400 in dark, with its own `destructive-foreground`; buttons use a solid fill, never a tinted one.
- `ink` / `ink-foreground` / `ink-muted`: a deep green-black band that stays dark in both themes, used for the course header and the resume card.

Rules: one accent colour per screen; no gradients as decoration (the generated cover gradient is content); no decorative shadows. State is never colour alone.

## Typography

| Role | Face | Use |
| --- | --- | --- |
| Display and titles | Literata (variable, optical size) | Page titles, course and lesson titles, module names |
| Interface and body | Geist (variable) | Navigation, controls, metadata, body |
| Technical | JetBrains Mono (variable) | Timer clock, timestamps, shortcuts |

Fonts are bundled with the app; the app makes no font requests to third parties and the server's CSP would block them. Titles use `text-wrap: balance`. Body measure stays under 75 characters.

## Layout

- **Global navigation.** Topbar with the logo, My learning, Saved, Progress and Notes, the search box (`/` focuses it), the Pomodoro timer and the account menu. Below 640 px a bottom tab bar replaces the topbar links. The language switch is in Settings, not here.
- **Course context.** The curriculum is a sidebar next to the player on desktop and sits below the player on small screens.
- **Containers.** Content is at most 72 rem wide with a 16 px gutter; the player page uses a media column plus a 22 rem sidebar.
- **Targets.** Every interactive target is at least 44×44 px, including icon buttons, tabs, menu items and form fields.

## Components

Built on shadcn/ui with **Base UI** primitives (chosen because it is the only candidate that satisfies the server's `style-src` CSP with no violation; see ADR 0003). Components live in `src/components/ui/`. LearnVault changes to the stock set:

- Buttons, inputs, tabs and menu items are sized to 44 px.
- The default button's hover mixes toward the foreground colour instead of fading, to hold label contrast.
- The tab list has a line variant for page sections.

Product components: `CourseCover`, `CourseCard`, `Curriculum`, `MediaPlayer`, `StudyNotes`, `Flashcards`, `FocusMenu`, `DesktopTitlebar`.

- **Course cover.** Order of precedence: uploaded cover, automatic video thumbnail, generated cover (cloth colour from the title, darker spine, inner frame, serif title). Covers are decorative; the text beside them names the course.
- **Progress.** A thin bar plus text ("42% complete · 3 lessons left"); the bar is `role="progressbar"` labelled by that text.

## Screens

My learning (resume, shelves, all courses), Course, Lesson (player), Progress, Notes, Settings and Sign in are built. Saved and Search are being added. Details: [`docs/design/redesign.md`](docs/design/redesign.md).

## Gamification

Habit first: streak, weekly goal, daily missions, XP and level, and quiet achievement badges (lessons completed, courses finished). No leaderboards, coins, certificates or celebratory animations. The 2.5D forest and nursery of the Svelte app are not part of this system.

## Accessibility

WCAG 2.2 AA is the target. Automated gates: `eslint-plugin-jsx-a11y`, axe on every route in all three palettes and both themes, and a check that fails on any CSP violation. Required behaviour: skip link and landmarks, visible focus, `prefers-reduced-motion` honoured, status changes in live regions without moving focus, and single-key shortcuts that act only when the player has focus. A manual keyboard and screen-reader pass is required before cutover. Checklist: [`docs/design/accessibility.md`](docs/design/accessibility.md).

## Security and privacy in the interface

No third-party requests, no HTML from user or file content (`react/no-danger`), nothing sensitive in browser storage, a generic sign-in error with rate-limit feedback, sign-out that clears per-user local data, and destructive dialogs that state their impact. Details: [`docs/design/security.md`](docs/design/security.md).

## Invariants

1. The interface never offers to move, rename, delete or write original course files.
2. Every user-facing string goes through `tr(en, ptBr)`.
3. Motion is short and shows a state change; nothing animates on its own to decorate.
4. Missing media explains how to recover and never discards progress.
