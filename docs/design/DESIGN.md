# LearnVault — Production Design System

The canonical specification for LearnVault's visual identity, layout architecture, and component tokens.

---

## Visual Direction

**A private course shelf.** LearnVault anchors user experience around rapid course discovery and immediate resume affordances:
- A compact, non-intrusive topbar with clear page state and responsive wrapping.
- Global instant search accessible via `/` hotkey.
- A prominent "Continue studying" row with timestamped progress (up to 4 most recently studied lessons).
- A structured course catalog supporting distinct Progress and Category filter groups, with numeric folder prefixes cleaned for readability.
- Four persistent views: Covers, List, Compact, and Shelf.
- A media player centered around an uncompromised 16:9 stage with an attached contextual curriculum column.

---

## Production Color Tokens

The design system is implemented in `apps/web/src/redesign.css` and `apps/web/src/ui-components.css` (using daisyUI primitives scoped under the `lvui-` prefix to prevent style collisions):

| Token | Light | Dark | Semantic Purpose |
| :--- | :--- | :--- | :--- |
| **Canvas** | `#f8fafc` | `#090d13` | Full-page background canvas |
| **Surface** | `#ffffff` | `#111722` | Cards, panels, inputs, shelves |
| **Raised** | `#f1f5f9` | `#182230` | Selected state, active tab, hover highlights |
| **Ink** | `#0f172a` | `#f1f5f9` | Primary headings, titles, high-contrast text |
| **Muted** | `#475569` | `#8e9ea9` | Secondary metadata, timestamps, subtle guidance |
| **Accent** | `#d97706` | `#d97706` | Primary action triggers, active streaks, and highlights |
| **Blue** | `#0284c7` | `#0284c7` | Secondary actions, links, active selections |
| **Line** | `#e2e8f0` | `#2a3a52` | Structural borders and container outlines |

---

## Typography & Composition

- **Headings & Display:** `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `Roboto`, `Inter`, sans-serif, weight 400–700 for headings and section entries.
- **Interface & Controls:** `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `Roboto`, `Inter`, system-ui, sans-serif, weight 400–700 for buttons, navigation, and descriptions.
- **Technical Metas:** `Cascadia Code`, `Cascadia Mono`, `JetBrains Mono`, `Consolas`, monospace for timestamps, shortcuts, and counters.
- **Desktop Viewport:** Centered layout with responsive sticky topbar.
- **Mobile Viewport (`≤820px`):** Single-column layout, full-width CTA buttons, touch targets meeting 44×44px minimums.

---

## Core Brand Mark

The official visual brand mark is the architectural geometric **LV Monogram**:
- Monochromatic, clean-room design rendered in `currentColor`.
- Quiet presence that does not compete with user course covers.
- Located in `Brand.svelte` and vector SVGs at `docs/design/branding/learnvault-mark.svg`, `docs/design/branding/learnvault-icon-light.svg`, and `docs/design/branding/learnvault-icon-dark.svg`.


---

## Accessibility & Interaction Invariants

- **Contrast:** High-contrast design tokens targeted for WCAG 2.2 AA (minimum 4.5:1 body text contrast, > 14:1 primary headings; formal certification pending external audit).
- **Focus Indicators:** High-visibility 2px focus ring (`:focus-visible`).
- **Motion:** Full respect for `prefers-reduced-motion: reduce`.
- **Read-Only Invariant:** No UI affords moving, renaming, writing, or deleting original files on disk.
