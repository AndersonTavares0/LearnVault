# Accessibility checklist (WCAG 2.2 AA)

Applies to `apps/web-next`. A screen is not done until every item below holds in both themes
and every palette. Automated checks (`eslint-plugin-jsx-a11y`, axe in Playwright) do not replace
the manual pass at the end.

## Structure and keyboard

- One `h1` per screen, landmarks (`header`, `nav`, `main`, `footer`), and a skip link to `#main`.
- Every control is reachable and operable by keyboard (2.1.1); no keyboard traps.
- Focus is always visible and never hidden behind sticky bars (2.4.11, 2.4.13).
- Shortcuts never fire while typing (2.1.4): `/` focuses search only outside text fields.
- Reordering has a button alternative to dragging (2.5.7): move up and move down.

## Targets, motion and zoom

- Interactive targets are at least 44×44 px (2.5.8 asks for 24; the project adopts 44).
- `prefers-reduced-motion` removes non-essential motion; nothing animates on its own to
  attract attention.
- Content reflows at 400% zoom / 320 px wide without horizontal scroll (1.4.10).
- Text spacing overrides (1.4.12) do not clip or overlap content.
- `forced-colors` (high contrast) keeps borders, focus rings and progress bars visible.

## Color and content

- Text contrast is at least 4.5:1, UI components and progress bars at least 3:1, in light and
  dark, for each palette (Stone + Emerald, Zinc + Teal, Zinc + Indigo).
- Color is never the only signal: completion also shows a check, status also shows text.
- Every user-facing string goes through `tr(en, ptBr)`; `lang` follows the locale.
- Dates, durations and plurals are formatted for the active locale.

## Player

- Captions are synchronized and selectable (1.2.2); media never autoplays with sound (1.4.2).
- Every player control has an accessible name, role and state (4.1.2).
- Loading, buffering and errors are announced with `role="status"` / `role="alert"` without
  moving focus (4.1.3).
- PDF reader: page changes and annotation saves are announced; marks are not color-only.

## Keyboard shortcut map

| Key | Action |
| --- | --- |
| `/` | Focus library search |
| `Space` / `K` | Play or pause |
| `←` / `J` | Seek back |
| `→` / `L` | Seek forward |
| `↑` / `↓` | Volume |
| `[` / `]` | Playback speed |
| `F` | Fullscreen (video) |
| `M` | Mute |
| `0`–`9` | Jump to a point in the lesson |
| `←` / `→` / `PgUp` / `PgDn` | Previous or next page (PDF reader) |

Keep this table in sync with the player's shortcut handler.

## Manual pass before a release

1. Keyboard only: sign in, open a course, play a lesson, add a note, finish a review.
2. Screen reader (NVDA on Windows, Orca on Linux): same flow; check names and announcements.
3. 400% zoom and a 320 px viewport on My learning, Course and Lesson.
4. Reduced motion and forced colors enabled.
