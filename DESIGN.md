---
name: LearnVault
description: Biblioteca local de cursos com descoberta clara e retomada sem atrito.
colors:
  canvas: "#0b1220"
  surface: "#111b2b"
  surface-raised: "#182438"
  ink: "#f5f7fb"
  muted: "#a8b5c7"
  line: "#2a3a52"
  cobalt: "#1d5fd1"
  cobalt-soft: "#17376f"
  saffron: "#e8b43c"
  danger: "#e88478"
  success: "#65bd91"
typography:
  display:
    fontFamily: "Georgia, Cambria, serif"
    fontSize: "clamp(2rem, 4vw, 3.4rem)"
    fontWeight: 400
    lineHeight: 1.05
    letterSpacing: "-0.035em"
  headline:
    fontFamily: "Georgia, Cambria, serif"
    fontSize: "clamp(1.45rem, 2.4vw, 2.25rem)"
    fontWeight: 400
    lineHeight: 1.12
  title:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "1.05rem"
    fontWeight: 700
    lineHeight: 1.3
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "0.95rem"
    fontWeight: 400
    lineHeight: 1.55
  label:
    fontFamily: "Cascadia Mono, Consolas, monospace"
    fontSize: "0.68rem"
    fontWeight: 700
    lineHeight: 1.35
    letterSpacing: "0.06em"
rounded:
  sm: "0.375rem"
  md: "0.625rem"
  lg: "0.875rem"
spacing:
  xs: "0.375rem"
  sm: "0.625rem"
  md: "1rem"
  lg: "1.5rem"
  xl: "2.5rem"
components:
  button-primary:
    backgroundColor: "{colors.cobalt}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "0.7rem 1rem"
    height: "2.75rem"
  button-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "0.7rem 1rem"
    height: "2.75rem"
  search-field:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "0.75rem 1rem"
  course-shelf:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: "1rem"
---

# Design System: LearnVault

## Overview

**Creative North Star: "A private course shelf"**

LearnVault feels like a focused course platform with the calm authority of a personal archive. The interface makes course discovery, the next lesson, and saved progress visible without importing marketplace mechanics. The memorable move is the shelf: courses are grouped into purposeful horizontal study rows rather than a dashboard of identical cards.

The shell is deep navy and quiet. Cobalt is reserved for the primary next action; saffron marks saved playback position and selected learning state. The LV mark is a small outlined monogram beside a restrained wordmark. Covers can carry authored color and typography, but the shell never competes with them.

**Key Characteristics:**
- Course-first hierarchy: resume, shelves, course overview, then lesson.
- Deep navy surfaces with warm editorial headings and restrained functional accents.
- Content-original labels make local course language intentional, not accidental.
- Progressive disclosure on mobile; no persistent rail inside the player.

## Colors

The palette is dark, layered, and low-saturation except for two functional accents.

### Primary
- **Cobalt action** (`#1d5fd1`): the single strongest action, used for Continue, Open course, and selected navigation.
- **Saffron bookmark** (`#e8b43c`): saved playback position, never a primary CTA.

### Neutral
- **Night canvas** (`#0b1220`): page background and player surround.
- **Course surface** (`#111b2b`): search, shelves, panels, and forms.
- **Raised surface** (`#182438`): selected rows, metadata groups, and active curriculum items.
- **Ink** (`#f5f7fb`): headings and primary text.
- **Muted ink** (`#a8b5c7`): supporting copy and metadata.
- **Quiet line** (`#2a3a52`): structural dividers; never a decorative border on every element.

**The Accent-Rarity Rule.** Cobalt starts the next action; saffron explains saved state. Do not use either as decoration.

## Typography

**Display Font:** Georgia, Cambria, serif
**Body Font:** Segoe UI Variable Text, Segoe UI, system-ui, sans-serif
**Label/Mono Font:** Cascadia Mono, Consolas, monospace

**Character:** Serif headings make course titles feel authored; the sans body keeps filters, metadata, and controls fast to scan. Mono is reserved for paths, saved positions, and system states.

### Hierarchy
- **Display** (400, `clamp(2rem, 4vw, 3.4rem)`, 1.05): library and course entry points only.
- **Headline** (400, `clamp(1.45rem, 2.4vw, 2.25rem)`, 1.12): course and lesson titles.
- **Title** (700, 1.05rem, 1.3): shelf/course names and panel headings.
- **Body** (400, 0.95rem, 1.55): descriptions, outcomes, and instructions; keep measures under 76ch.
- **Label** (700, 0.68rem, 1.35, tracked): paths, status, and saved-position metadata only.

**The Original-Content Rule.** Interface chrome is Portuguese. Course titles and descriptions remain in the language supplied by local files, with a visible `Conteúdo original` marker.

## Layout

The desktop shell uses a compact topbar and a centered content column capped near 1240px. The Library leads with a short resume row, then search/filter controls, then editorial shelves. Course detail is a full-width reading surface with cover, outcomes, metadata, and a module curriculum. The Player gives the stage the largest region; curriculum follows below or opens as a disclosure on narrow screens.

Spacing follows a 6/10/16/24/40px rhythm. Related metadata is tight; a new shelf or content mode earns 32–40px of separation. At 820px and below, utility controls wrap, shelves become vertical rows, and the player breadcrumb becomes `Voltar ao curso` plus the current lesson. No fixed text column may force horizontal scrolling.

## Elevation & Depth

Depth comes from tonal layering and one-pixel structural lines, not omnipresent shadows. Raised surfaces distinguish selection and interaction; the player uses a dark stage inside a slightly raised shell. Shadows are reserved for menus and transient overlays.

### Shadow Vocabulary
- **Menu lift** (`0 14px 30px rgba(0,0,0,.34)`): menus and popovers only.
- **Cover lift** (`0 10px 24px rgba(0,0,0,.22)`): hover/focus on a course cover only.

**The Flat-at-Rest Rule.** A shelf is calm until the user selects, opens, or focuses it.

## Shapes

Use 6px fields/buttons, 10px panels, and 14px feature shells. Borders are quiet and structural. Avoid pill-shaped containers except status chips and compact selected filters. Course covers use an authored 3:4 or 16:9 ratio; do not let image treatment dictate the whole shell.

## Components

### Buttons
- **Shape:** compact rectangular controls with 6px corners.
- **Primary:** cobalt background, ink text, 44px minimum height; one primary action per visible group.
- **Hover / Focus:** darken cobalt; preserve a 2px visible focus ring using the focus token.
- **Ghost:** transparent or surface background with a quiet line; use for navigation and secondary actions.

### Chips / Status
- **Style:** small mono label or compact filter button; status colors carry meaning, not decoration.
- **State:** selected filters use cobalt-soft plus ink; saved position uses saffron; missing media uses danger text and an explicit disabled state.

### Cards / Shelves
- **Corner Style:** 10px shelf shell, 6px course row.
- **Background:** surface on canvas; raised surface for selected course or lesson.
- **Shadow Strategy:** flat at rest; menu/cover lift only on interaction.
- **Internal Padding:** 16px desktop, 12px mobile.

### Inputs / Fields
- **Style:** surface fill, quiet line, 6px corners, 44px minimum height.
- **Focus:** cobalt/focus outline with 2px offset; never rely on placeholder alone.
- **Error / Disabled:** retain text contrast and explain recovery; disabled media must not look playable.

### Navigation
- **Style:** compact topbar with the subdued LV logo, Library/Stats/Settings destinations, focus control, theme control, and user context.
- **Player:** hide secondary shell navigation only in focus mode; show `Voltar ao curso` and current lesson context on mobile.
- **Active:** cobalt underline or raised surface, not a filled dashboard tile.

### Course stage
The player stage is the signature component: 16:9, centered, with a restrained preview frame when paused and a single primary time readout. A missing local file replaces playback controls with an explanation and recovery instruction.

## Do's and Don'ts

### Do:
- **Do** lead with the next lesson or course, not generic metrics.
- **Do** use daisyUI primitives (`lvui-btn`, `lvui-input`, `lvui-join`, `lvui-range`) as accessible mechanics, then apply LearnVault tokens around them.
- **Do** keep the logo quiet and identical across themes, routes, and breakpoints.
- **Do** label source-language course metadata as `Conteúdo original`.
- **Do** test 390px, 1280px, 1440px, and 1920px widths with long titles and missing media.

### Don't:
- **Don't** add marketplace affordances such as ratings, prices, enrollment, certificates, or cloud profiles.
- **Don't** use gradients, flashy logo colors, decorative badges, or a card inside a card without a clear information role.
- **Don't** turn every state into a border or every action into cobalt.
- **Don't** truncate the only breadcrumb that tells the learner where they are.
- **Don't** claim playback, persistence, folder scanning, or accessibility verification without evidence.
