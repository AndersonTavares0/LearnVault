# LearnVault Design System

## Vision & Creative Direction

**North Star: "A private course shelf — calm, durable, and content-first."**

LearnVault is a self-hosted study space and offline workstation designed for developers, researchers, and self-directed learners who study with course directories they already own. The experience combines familiar course discovery and clear resume affordances with workstation density and ergonomic navigation.

The interface prioritizes content clarity, swift curriculum navigation, and the strict integrity of physical course files—which remain 100% read-only on disk.

---

## Brand Identity

The official LearnVault brand mark is an architectural geometric **Shield** with centered 3D depth facets, representing protection, preservation, and organized study of personal knowledge.

- **Monochromatic & Quiet:** Rendered in monochromatic tones without flashy gradients or saturated novelty colors, ensuring it integrates harmoniously into desktop titlebars, web headers, and dark mode surfaces.
- **Orientation Anchor:** Anchored at the top-left of global navigation (`apps/web/src/Brand.svelte`) with proportional optical spacing.
- **Official Vector Asset:** Maintained in clean SVG vector format at `docs/design/branding/learnvault-banner.svg`.

---

## Production Color Tokens

Color tokens are defined in `apps/web/src/redesign.css` and use semantic roles:

### Dark Theme (Default)
- **Canvas (Background):** `#0b1220` (Deep night canvas, minimizing eye fatigue during long sessions)
- **Surface (Panels, Cards):** `#111b2b` (Stratified graphite surface)
- **Surface Raised (Active & Hover):** `#182230` (Elevated surface for selection and active tabs)
- **Borders & Dividers:** `#2a3a52` (Calm structural boundary lines)
- **Primary Ink (Headings & Content):** `#f5f7fb` (High-contrast readable text, > 14:1 contrast ratio)
- **Muted Ink (Secondary Metadata):** `#a8b5c7` (Clean technical typography, > 4.5:1 ratio)
- **Cobalt (Primary Action):** `#1d5fd1` (Initiates the next action; never used as ambient decoration)
- **Saffron (Saved State & Bookmark):** `#e8b43c` (Active progress, resume badges, and study streaks)
- **Success:** `#10b981` (Completed lessons and active streak markers)
- **Danger:** `#f43f5e` (Missing media or unreadable paths)
- **Focus Ring:** `#38bdf8` (2px accessible focus outline)

### Light Theme
- **Canvas:** `#f4f6f9` (Soft neutral canvas, non-glaring)
- **Surface:** `#ffffff` (Pure white structured by 1px neutral lines)
- **Surface Raised:** `#edf1f6` (Subtle selection highlight)
- **Borders & Dividers:** `#d7dee8`
- **Primary Ink:** `#152238` (Deep navy slate text)
- **Muted Ink:** `#56657a`
- **Cobalt:** `#1d5fd1`
- **Saffron:** `#8a5708`
- **Success:** `#059669`
- **Danger:** `#e11d48`
- **Focus Ring:** `#0284c7`

---

## Typography

| Role | Family | Weight | Purpose |
|---|---|---|---|
| **Editorial Headings** | `Georgia`, `Cambria`, serif | 400 | Library headings, course titles, prominent section entrypoints |
| **Interface & Body** | `Segoe UI Variable`, `Segoe UI`, `-apple-system`, `Inter`, system-ui | 400 / 600 / 700 | Navigation, metadata, buttons, labels, readable descriptions |
| **Technical & Mono** | `Cascadia Mono`, `Consolas`, `JetBrains Mono`, monospace | 500 / 600 | Video timestamps, seek indicators, durations, keyboard shortcuts |

---

## Key Workspaces & Components

### 1. Library & Browsing Modes
- **Continue Studying:** Direct resume row with 16:9 thumbnails, current lesson title, and instant playback trigger.
- **Four Layout Views:** User-selectable and persisted in `localStorage`:
  - *Covers (Grid):* 16:9 cards with generative course art or video thumbnails.
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
