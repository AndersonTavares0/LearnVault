# Accessibility requirements

> **Superseded in part (2026-11-11).** The palette, themes, navigation and gamification described here were replaced by the direction in [`DESIGN.md`](../../DESIGN.md) and [`PRODUCT.md`](../../PRODUCT.md): light and dark themes, brown and amber, Playfair Display and DM Sans, top navigation, no forest. Where this file names a palette, a theme or a layout, those two files win. It is kept as history and for the research and the accessibility and security rules, which still apply.

LearnVault targets **WCAG 2.2 Level AA**. This checklist applies to the React app (epic #347). Every
phase PR that changes UI ticks the relevant items. The cutover (#340) requires all of them.

Automated checks find only part of the problems. A manual keyboard and screen reader pass is
required before cutover.

## Automated gates (CI)

- [ ] ESLint `jsx-a11y` (recommended rules) with no warnings.
- [ ] Playwright + axe on every route, in both themes and all three palettes: zero violations.
- [ ] Playwright fails on any `securitypolicyviolation` event. CSP regressions often break styling
      that accessibility depends on.

## Structure and navigation

- [ ] A skip link to `main` is the first focusable element (2.4.1).
- [ ] `header`, `nav`, `main`, and `aside` landmarks, with a unique accessible name when repeated.
- [ ] One `h1` per route, and headings in order (1.3.1).
- [ ] The document title changes on every route change, and focus moves to the new page heading (2.4.2).
- [ ] Focus order follows the visual order (2.4.3).
- [ ] The bottom tab bar on mobile and the topbar on desktop expose the same destinations.

## Keyboard

- [ ] Every action works with the keyboard alone (2.1.1), with no keyboard traps (2.1.2).
- [ ] Single-key shortcuts (player, `/` search) are ignored while typing in a field. A
      shortcuts dialog (`?`) lists them all (2.1.4).
- [ ] Menus, dialogs, tabs, and the accordion follow the WAI-ARIA Authoring Practices patterns.
- [ ] Dialogs trap focus while open and return focus to the trigger on close.
- [ ] Course order can be changed with Move up and Move down buttons, not only by dragging (2.5.7).

## Focus and targets

- [ ] Focus is always visible, with a ring of at least 2 px that has ≥ 3:1 contrast against both
      neighbours (2.4.7, 2.4.13).
- [ ] Sticky headers, the sidebar, and toasts never cover the focused element (2.4.11).
- [ ] Interactive targets are at least 44 × 44 px, a project rule stricter than 2.5.8.
- [ ] Text is never set below `text-xs` (12 px) and is sized in `rem`, so it follows the Text size setting; fixed `px` font sizes are not used.
- [ ] Dialogs are capped at the viewport height (`max-h-[calc(100dvh-2rem)]`) and scroll inside it, so a long list never pushes the title or the close button off a short screen.
- [ ] When a dialog replaces the list the keyboard was on (drill-down navigation), focus moves to a stable element of the new level instead of falling back to the page behind.

## Color and text

- [ ] Body text contrast ≥ 4.5:1, and large text and UI components ≥ 3:1 (1.4.3, 1.4.11), checked for
      every palette in both modes.
- [ ] State is never shown by color alone. Completed lessons have an icon and accessible text
      (1.4.1).
- [ ] At 400% zoom (320 CSS px wide) the content reflows, with no horizontal scroll except the
      media stage (1.4.10).
- [ ] Text spacing overrides (line height 1.5, paragraph spacing 2×) do not clip text (1.4.12).
- [ ] The UI stays usable in `forced-colors: active` (Windows high contrast).

## Media player

- [ ] Captions can be selected and are on when the user chose them before (1.2.2). Supported:
      vtt, srt, ass, sub.
- [ ] No autoplay with sound (1.4.2).
- [ ] Every control has a name, role, and state (4.1.2), for example "Pause", "Mute, on", or
      "Subtitles: English".
- [ ] Buffering, errors, missing media, and "marked as completed" are announced through a polite live
      region without moving focus (4.1.3).
- [ ] The next-lesson countdown can be cancelled and paused, and is static with reduced motion (2.2.1).

## Motion and time

- [ ] All animation respects `prefers-reduced-motion: reduce` (2.3.3).
- [ ] The Pomodoro timer announces state changes (start, end) through a live region, not every tick.
- [ ] An expired session never loses typed content. Note drafts are kept and the user returns to
      the same place after signing in (2.2.5).

## Forms and errors

- [ ] Every input has a visible label (3.3.2). Errors are described in text and linked
      with `aria-describedby` (3.3.1).
- [ ] Login fields use `autocomplete="username"` and `autocomplete="current-password"`, and allow
      paste (3.3.8).
- [ ] Destructive actions ask for confirmation and describe the impact (3.3.4).

## Language

- [ ] `<html lang>` follows the selected locale. Course titles and descriptions from the user's files
      keep their original language. Mark them with `lang` when known (3.1.2).
- [ ] Every user-facing string uses `tr(en, ptBr)`.

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

## Manual pass before cutover

- [ ] Keyboard only: sign in, resume a lesson, take a note, mark completed, change palette.
- [ ] Screen reader: NVDA + Firefox (Windows), Orca + Firefox (Linux), the same flow.
- [ ] Mobile: 375 px width, touch only, the same flow.
- [ ] Record the results and known limitations in the cutover PR.
