# Redesign spec (direction L)

> **Superseded in part (2026-11-11).** The palette, themes, navigation and gamification described here were replaced by the direction in [`DESIGN.md`](../../DESIGN.md) and [`PRODUCT.md`](../../PRODUCT.md): light and dark themes, brown and amber, Playfair Display and DM Sans, top navigation, no forest. Where this file names a palette, a theme or a layout, those two files win. It is kept as history and for the research and the accessibility and security rules, which still apply.

This was the target design for the React app (`apps/web-next`, epic #347). The cutover (#340) is
complete, [DESIGN.md](DESIGN.md) describes what ships, and this file is kept as history.

Course platforms are a structural reference only. Do not copy their code, assets, brand, proprietary text,
or pixel-level layout.

## Principles

1. **Resume first.** The next lesson is always one click away from any screen that shows a course.
2. **Progress is visible.** Every course, module, and lesson shows its state.
3. **Quiet by default.** Use one accent color, neutral surfaces, no decorative gradients or shadows,
   and no celebration animations on routine actions.
4. **Private by design.** The app makes no third-party requests, and nothing about one user is
   visible to another.
5. **Read-only library.** No UI affords moving, renaming, writing, or deleting course files.

## Navigation

| Viewport | Global navigation | Course context |
| --- | --- | --- |
| Desktop (≥ 1024px) | Topbar with logo, search (`/`), My learning, Progress, Notes, Pomodoro, language, account menu | Curriculum sidebar on the course page and in the player, collapsible |
| Tablet (640–1023px) | Topbar; secondary items move into the account menu | Curriculum opens as a sheet |
| Mobile (< 640px) | Bottom tab bar: My learning, Progress, Notes, Account | Curriculum appears below the player |

The account menu holds Settings, Admin (admins only), and Sign out. The desktop build has no
Sign out and adds the custom titlebar.

Every page starts with a skip link and uses `header`, `nav`, `main`, and `aside` landmarks.

## Screens

### My learning (`#/`)

- **Continue learning.** The most recent lesson, with course title, lesson title, a progress bar, and a
  resume button. Up to three more recent lessons appear in a compact row.
- **Today.** Streak, weekly goal days, and flashcards due today, each as a small inline status.
  These are not big-number tiles.
- **Courses.** A grid (default) or list of course cards. Each card shows cover, title,
  progress %, lessons left, time left, and the next lesson.
- **Filters.** In progress, Not started, Completed, Favourites, plus category. Search matches
  courses and lessons.
- **Admin actions** (admins only): Import course, Rescan.
- Long catalogs are virtualized.

### Course (`#/course/<id>`)

- **Header.** Cover, title, overall progress, Continue button, and counts (lessons, total duration,
  materials). It also has a favourite toggle.
- **Tabs.** Content, Materials, Notes.
- **Curriculum.** One accordion section per module, showing module progress. Each lesson row has
  completion state, type icon, duration, and favourite. The current lesson is highlighted.
  All modules can be expanded (#321).
- **Milestones.** When the course reaches 25, 50, 75, or 100% complete, or a module is completed,
  a polite live-region message appears once. There is no modal.

### Player (`#/course/<id>/<lessonPath>?t=`)

- A 16:9 media stage, with the curriculum sidebar on the right (desktop) or below the stage (mobile).
- **Controls.** Play/pause, seek, volume, speed, subtitles, transcode (when available), PiP,
  fullscreen, open in system player (desktop). Every existing keyboard shortcut is kept and
  listed in a shortcuts dialog (`?`).
- **Below the stage.** Lesson title, Mark as completed, Previous/Next lesson, and the tabs Overview,
  Notes, Flashcards, Materials.
- **End of media.** A "Next lesson" card with a cancellable countdown. When reduced motion is on,
  there is no countdown animation.
- **Missing media.** Explains that the file is not on disk and keeps progress intact.

### Progress (`#/progress`)

- Streak (current and best), weekly goal (1–7 days), and today's missions.
- Level and XP, plus achievement badges earned and in progress.
- **Yearly heatmap** of study days, built from existing study-day data.
- **Per-course progress**, with time remaining. A finished course earns a quiet achievement
  badge (first course, three courses); there is no certificate.
- **Focus.** Recent Pomodoro sessions and totals.

### Notes (`#/notes`)

Notes are searchable across courses and lessons. Each note links back to its timestamp.

### Settings (`#/settings`)

- **Appearance.** Theme (System, Light, Dark) and palette (Stone + Emerald, Zinc + Teal,
  Zinc + Indigo).
- **Language.** English, Português (Brasil).
- **Profile.** Avatar and password (server only).
- **Focus.** Pomodoro durations and sounds, weekly goal.
- **Updates.** Admins see version, release notes, channel, history, and upgrade instructions (#345).
  The desktop app shows the restart-to-update flow (#346).
- **Admin.** Users and per-course access, course covers, course order (with move up/down buttons),
  and backup.

### Login (`#/login`, server only)

Username and password fields with password-manager attributes, show/hide password, a generic
error message, and rate-limit feedback. See [security.md](security.md).

## Gamification

Kept: streak, weekly goal, daily missions, XP and level, achievements, Pomodoro, flashcards, notes, favourites.

Added without backend changes (Phase 5):

- Course milestones
- Course-completion badges
- Yearly heatmap
- Time remaining
- Today's review
- Focus-block summary

Added with backend changes (Phase 7, #341):

- Weekly rest day that protects the streak
- Personal deadline per course
- Monthly/annual retrospective

Removed: the forest, island, tree species, nursery, drops, and guardian messages. On first load,
the app deletes the orphaned `learnvault-forest-nursery` and `learnvault-forest-session-seeds` keys.

Not planned: leaderboards between users, coins or shops, streaks that reset without warning, and
aggressive notifications.

## Color

The palettes are shadcn/ui themes on the Tailwind palette, in OKLCH, with light and dark variants. Each
palette is a `data-palette` value on `<html>`, stored per device under `learnvault-palette`.

| Palette | Neutral base | Accent (light / dark) | Default |
| --- | --- | --- | --- |
| Stone + Emerald | stone | emerald-700 / emerald-400 | yes |
| Zinc + Teal | zinc | teal-700 / teal-400 | |
| Zinc + Indigo | zinc | indigo-600 / indigo-400 | |

Rules:

- One accent color for actions and progress. Success, warning, and destructive colors are
  separate semantic tokens.
- In light mode, muted text uses the 600 step of the neutral scale, not the theme default, to keep a
  safe margin above 4.5:1.
- Measured button-label contrast. Light: 5.5 (emerald), 5.5 (teal), 6.3 (indigo). Dark: 10.3, 10.7,
  6.7. End-to-end tests check every palette in both modes.

## Type and spacing

- Fonts are bundled with the app (no external font requests): one sans family for the interface and
  one monospace family for timestamps and shortcuts.
- The type scale, spacing scale, and radius come from the Tailwind and shadcn/ui defaults. Do not add
  custom values unless a component needs them.
- Interactive targets are at least 44 × 44 px.

## Motion

Transitions are short (≤ 200 ms) and only show state changes: open, close, expand. With
`prefers-reduced-motion: reduce`, transitions are removed and the next-lesson countdown is static.
