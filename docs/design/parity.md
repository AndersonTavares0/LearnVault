# Feature parity checklist

> **Superseded in part (2026-11-11).** The palette, themes, navigation and gamification described here were replaced by the direction in [`DESIGN.md`](../../DESIGN.md) and [`PRODUCT.md`](../../PRODUCT.md): light and dark themes, brown and amber, Playfair Display and DM Sans, top navigation, no forest. Where this file names a palette, a theme or a layout, those two files win. The cutover (#340) is complete and the Svelte app was removed; this checklist is kept as history, and its accessibility and security rules still apply.

Every capability of the current Svelte SPA (`apps/web`) must exist in the React app (`apps/web-next`)
before the cutover (#340), unless it is listed under **Intentionally removed**. Tick an item when it
works in both builds where it applies. The builds are **S** (server, HTTP) and **D** (desktop, IPC).

## Shell

- [ ] Hash routes: `#/`, `#/course/<id>[/<lessonPath>][?t=]`, legacy `#/player/...`, `#/notes`,
      `#/settings`, statistics (now `#/progress`), and the `#/focus` alias. Old links keep working.
- [ ] Theme: system, light, dark (`learnvault-theme`), plus `meta[name=theme-color]`.
- [ ] Locale: en and pt-BR (`learnvault-locale`), with `<html lang>`.
- [ ] Global search with the `/` shortcut.
- [ ] Desktop titlebar with window controls (D).
- [ ] Offline/unavailable states for the API and missing disks.

## Sign-in and account (S)

- [ ] Sign in, sign out, session bootstrap (`/api/me`), and redirect after sign-in.
- [ ] Change username and password.
- [ ] Avatar: upload, show, remove.

## Library

- [ ] Catalog load with limits (`CATALOG_LIMITS`), course tree, and loose files.
- [ ] Continue studying (recent lessons).
- [ ] Filters by progress and category, and search with accent-insensitive matching.
- [ ] Views: grid and list. The four old views are reduced to two, and the `learnvault-library-view`
      value is migrated.
- [ ] Course covers: generated, automatic thumbnail (IndexedDB, ADR 0002), custom upload, change,
      and remove.
- [ ] Import course (S: admin; D: folder picker), remove course, rescan.
- [ ] Course order: set and reset, per parent.
- [ ] Course and lesson favourites.

## Course page

- [ ] Header with progress and Continue.
- [ ] Curriculum by module with lesson state, duration, and missing-media count.
- [ ] Materials (non-playable files) listed with download/open.

## Player

- [ ] Video and audio playback (`mediaPlaybackKind`), and resume from the saved position or `?t=`.
- [ ] Saves the position periodically, and handles mark completed and unmark.
- [ ] Subtitles: vtt, srt (converted to VTT on S), ass, sub.
- [ ] On-demand transcoding when the browser cannot play the media (capabilities endpoint).
- [ ] Picture-in-picture and fullscreen.
- [ ] Open in system player (D).
- [ ] Per-course player preferences (`learnvault.player-preferences.v1:*`).
- [ ] All shortcuts: Space, arrows, `k j l`, `[ ]`, `f`, `p`, `m`, `n`, `0`–`9`.
- [ ] Missing-media state, with recovery when the file returns.
- [ ] Previous/next lesson and auto-advance.

## Study tools

- [ ] Notes per lesson: create, edit, delete, timestamp jump.
- [ ] All notes: search across courses and lessons.
- [ ] Flashcards per lesson: create, edit, delete, review with ratings.

## Focus and rewards

- [ ] Pomodoro: focus, short break, long break, settings, sounds, a timer that persists across reloads.
- [ ] Focus sessions sync, with an offline outbox and rejected-session handling.
- [ ] Study summary: streak, best streak, at risk, week days.
- [ ] Weekly goal: view and set (1–7).
- [ ] Daily missions: view and claim.
- [ ] XP, level, and achievements.

## Settings and admin

- [ ] Pomodoro settings.
- [ ] Update check, force check, install (D), history (S).
- [ ] Users (S): list, create, delete, per-course access.
- [ ] Backup (S).

## Desktop IPC commands not used by the current UI

Decide in Phase 5 whether to expose these or leave them unused:

- [ ] `desktop_weekly_goal`
- [ ] `desktop_check_update`
- [ ] `desktop_media_removal_impact`
- [ ] `desktop_remove_media`

## Intentionally removed

- Forest, 2.5D island, tree species, nursery, drops, guardian messages, ecosystem bands, and the level
  crest illustration. Their data lived only in browser storage.
- Library views from the earlier Svelte cutover are superseded by the current React library. The current
  product intentionally supports Covers, List, Compact and Shelf; see `apps/web-next/src/lib/library-view.ts`.
