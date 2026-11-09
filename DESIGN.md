# LearnVault Design System

## Vision & Creative Direction

**North Star: "A private course shelf — calm, durable, and content-first."**

LearnVault is a self-hosted study space and offline workstation designed for developers, researchers, and self-directed learners who study with course directories they already own. The experience combines familiar course discovery and clear resume affordances with workstation density and ergonomic navigation.

The interface prioritizes content clarity, swift curriculum navigation, and the strict integrity of physical course files—which remain 100% read-only on disk.

---

## Brand Identity

The official LearnVault brand mark is the architectural geometric **LV Monogram**, representing durable, content-first study of personal knowledge.

- **Monochromatic & Quiet:** Rendered in monochromatic tones in `currentColor`, following theme ink color and integrating harmoniously into desktop titlebars, web headers, and dark mode surfaces.
- **Orientation Anchor:** Anchored at the top-left of global navigation (`apps/web/src/Brand.svelte`) with proportional optical spacing.
- **Official Vector Assets:** Maintained in clean SVG vector format at `docs/design/branding/learnvault-mark.svg`, `docs/design/branding/learnvault-icon-light.svg`, and `docs/design/branding/learnvault-icon-dark.svg`.

---

## Production Color Tokens

Color tokens are defined in `apps/web/src/redesign.css` and use semantic roles:

### Dark Theme (Default)
- **Canvas (Background):** `#090d13` (Deep night canvas, minimizing eye fatigue during long sessions)
- **Surface (Panels, Cards):** `#111722` (Stratified graphite surface)
- **Surface Raised (Active & Hover):** `#182230` (Elevated surface for selection and active tabs)
- **Borders & Dividers:** `#2a3a52` (Calm structural boundary lines)
- **Primary Ink (Headings & Content):** `#f1f5f9` (High-contrast readable text, > 14:1 contrast ratio)
- **Muted Ink (Secondary Metadata):** `#8e9ea9` (Clean technical typography, > 4.5:1 ratio)
- **Accent (Primary Action):** `#d97706` (Amber tone for resume badges, study streaks, and primary actions)
- **Blue (Info & Secondary):** `#0284c7` (Clean technical highlights and links)
- **Success:** `#059669` (Completed lessons and active streak markers)
- **Danger:** `#e11d48` (Missing media or unreadable paths)
- **Focus Ring:** `#0284c7` (2px accessible focus outline)

### Light Theme
- **Canvas:** `#f8fafc` (Soft neutral canvas, non-glaring)
- **Surface:** `#ffffff` (Pure white structured by 1px neutral lines)
- **Surface Raised:** `#f1f5f9` (Subtle selection highlight)
- **Borders & Dividers:** `#e2e8f0`
- **Primary Ink:** `#0f172a` (Deep navy slate text)
- **Muted Ink:** `#475569`
- **Accent:** `#d97706`
- **Blue:** `#0284c7`
- **Success:** `#059669`
- **Danger:** `#e11d48`
- **Focus Ring:** `#0284c7`

---

## Typography

| Role | Family | Weight | Purpose |
|---|---|---|---|
| **Headings & Display** | `system-ui`, `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `Roboto`, `Inter`, sans-serif | 400 / 600 / 700 | Library headings, course titles, prominent section entrypoints |
| **Interface & Body** | `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `Roboto`, `Inter`, system-ui, sans-serif | 400 / 500 / 600 | Navigation, metadata, buttons, labels, readable descriptions |
| **Technical & Mono** | `Cascadia Code`, `Cascadia Mono`, `JetBrains Mono`, `Consolas`, `SFMono-Regular`, monospace | 500 / 600 | Video timestamps, seek indicators, durations, keyboard shortcuts |

---

## Key Workspaces & Components

### 1. Library & Browsing Modes
- **Continue Studying:** Direct resume row with 16:9 cards, current lesson title, and instant playback trigger. Deduplicated by course so each course appears at most once (showing its most recent lesson, up to 4 distinct courses). On mobile viewports, cards maintain a 76vw width to ensure the next item peeks out as a visible scroll affordance.
- **Filter Groups:** Organized into distinct Progress filters (All, In progress, Not started, Completed, Favorites) and Category filters. Category names automatically strip noisy numeric folder prefixes (e.g., `01 - `, `02. `) while preserving item counts.
- **Course Covers:** Neutral, consistent treatment for courses without uploaded covers or video posters, using deterministic earthy gradients with a subtle brand monogram watermark rather than loud letter tiles.
- **Four Layout Views:** User-selectable and persisted in `localStorage`:
  - *Covers (Grid):* 16:9 cards with video thumbnails, custom covers, or quiet geometric generative art.
  - *List:* Full-width horizontal rows with course metadata, estimated durations, and lesson counts.
  - *Compact:* Maximum density for extensive multi-hundred course collections.
  - *Shelf:* Spine-inspired visual layout reminiscent of a technical library.
- **Global Search:** Instant live filter accessible via `/` shortcut matching course titles and lesson names.

### 2. Media Player & Lesson Workspace
- **Split View:** 16:9 video or audio player on the main stage, contextual curriculum sidebar with independent scrolling.
- **Lesson Controls:**
  - Auto-advance toggle for continuous sequential study.
  - Explicit completion toggle decoupled from seek position.
  - Picture-in-Picture (PiP) and on-demand transcoding toggles when supported.
- **Keyboard Shortcuts:** `Space` (play/pause), arrow keys (seek ±5s, volume), `m` (mute), `f` (fullscreen), `p` (PiP), documented visually with accessible ARIA keyshortcut declarations.

### 3. Study Forest & Focus Gamification
- **Pomodoro Timer:** Configurable focus cycles (Focus, Short Break, Long Break) with sound indicators.
- **Ecosystem Growth:** Blossoming trees and 2.5D island groves rewarded for focused sessions.
- **Flashcards & Notes:** Spaced repetition (SM-2 based) and timestamped notes attached to lessons.

---

## Design Invariants & Accessibility

1. **Host Library is Strictly Read-Only:** The interface never provides actions to move, rename, delete, or write original course files on disk.
2. **WCAG 2.2 AA Target Ratios:** All production tokens are designed to meet or exceed 4.5:1 contrast (> 14:1 for primary ink), and interactive controls maintain minimum 44×44px click targets (formal assistive-technology certification pending external audit).
3. **Reduced Motion:** All transitions, rotations, and animations respect `prefers-reduced-motion: reduce`.
4. **Resilient Offline States:** Missing media or disconnected disks are communicated with clear recovery instructions without discarding progress records.
5. **Zero AI-Slop & Clutter:** No decorative drop-shadows, nested multi-card wrappers, or saturated background gradients.
