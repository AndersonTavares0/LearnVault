# LearnVault — Production Design System

The canonical specification for LearnVault's visual identity, layout architecture, and component tokens.

---

## Visual Direction

**A private course shelf.** LearnVault anchors user experience around rapid course discovery and immediate resume affordances:
- A compact, non-intrusive topbar with clear page state.
- Global instant search accessible via `/` hotkey.
- A prominent "Continue studying" row with timestamped progress.
- A structured course catalog supporting four persistent views: Covers, List, Compact, and Shelf.
- A media player centered around an uncompromised 16:9 stage with an attached contextual curriculum column.

---

## Production Color Tokens

The design system is implemented in `apps/web/src/redesign.css` and `apps/web/src/ui-components.css` (using daisyUI primitives scoped under the `lvui-` prefix to prevent style collisions):

| Token | Light | Dark | Semantic Purpose |
| :--- | :--- | :--- | :--- |
| **Canvas** | `#f4f6f9` | `#0b1220` | Full-page background canvas |
| **Surface** | `#ffffff` | `#111b2b` | Cards, panels, inputs, shelves |
| **Raised** | `#edf1f6` | `#182230` | Selected state, active tab, hover highlights |
| **Ink** | `#152238` | `#f5f7fb` | Primary headings, titles, high-contrast text |
| **Muted** | `#56657a` | `#a8b5c7` | Secondary metadata, timestamps, subtle guidance |
| **Cobalt** | `#1d5fd1` | `#1d5fd1` | Primary action triggers and active selections |
| **Saffron** | `#8a5708` | `#e8b43c` | Saved progress, resume position, active streaks |
| **Line** | `#d7dee8` | `#2a3a52` | Structural borders and container outlines |

---

## Typography & Composition

- **Editorial Headings:** `Georgia`, `Cambria`, serif, weight 400, fluid scale for library section entries and course titles.
- **Interface & Controls:** `Segoe UI Variable`, `Segoe UI`, `-apple-system`, `Inter`, sans-serif, weight 400–700 for buttons, navigation, and descriptions.
- **Technical Metas:** `Cascadia Mono`, `Consolas`, monospace for timestamps, shortcuts, and counters.
- **Desktop Viewport:** Centered layout constrained to 1240–1380px with sticky topbar.
- **Mobile Viewport (`≤820px`):** Single-column layout, full-width CTA buttons, touch targets meeting 44×44px minimums.

---

## Core Brand Mark

The official visual brand mark is a geometric Shield with centered 3D depth lines:
- Monochromatic, clean-room design.
- Quiet presence that does not compete with user course covers.
- Located in `Brand.svelte` and vector SVG at `docs/design/branding/learnvault-banner.svg`.

---

## Accessibility & Interaction Invariants

- **Contrast:** High-contrast design tokens targeted for WCAG 2.2 AA (minimum 4.5:1 body text contrast, > 14:1 primary headings; formal certification pending external audit).
- **Focus Indicators:** High-visibility 2px focus ring (`:focus-visible`).
- **Motion:** Full respect for `prefers-reduced-motion: reduce`.
- **Read-Only Invariant:** No UI affords moving, renaming, writing, or deleting original files on disk.
