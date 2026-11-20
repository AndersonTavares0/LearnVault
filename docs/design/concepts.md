# LearnVault — visual directions

> **Superseded in part (2026-11-11).** [`DESIGN.md`](../../DESIGN.md) and [`PRODUCT.md`](../../PRODUCT.md) replaced the palette, themes, navigation and gamification described here (light and dark themes, Playfair Display and DM Sans, top navigation, no forest). Where they disagree with this file, they win. The file stays as history; its accessibility and security rules still apply.

## Scope and evidence

Prototypes A, B and C were started as independent empty documents and compared on Library, Course and Player, on desktop and mobile. They were HTML artifacts rendered in Chromium with synthetic data. They show composition, not playback, persistence or product behaviour.

Research is in [research.md](research.md) and the weighted evaluation in [decision.md](decision.md). None of the three concepts passed the threshold. K was chosen after further exploration and an independent critique.

## A — Editorial library

- **Navigation:** persistent topbar; Library, Course and Player.
- **Composition:** editorial header, proportional covers, a course treated as a publication, and low/medium density.
- **Identity:** serif display, light paper and earthy tones.
- **Limit:** card navigation and Player controls were static; other screens and essential states were not covered.
- **Result:** Rejected in the comparative evaluation.

## B — Study workbench

- **Navigation:** vertical rail plus a compact topbar with search and profile.
- **Composition:** simultaneous columns for catalog, curriculum, session and media; high density, oriented to resuming.
- **Identity:** functional sans, cool palette and a green accent.
- **Limit:** Player actions were simulations; on mobile, saving and studying became ambiguous.
- **Result:** Rejected in the comparative evaluation.

## C — Learning atlas

- **Navigation:** bottom dock; uses neither a sidebar nor a destinations topbar.
- **Composition:** spatial map of courses and a visual path of lessons; low density.
- **Identity:** condensed display, cartographic grid, deep green and ivory paper.
- **Limit:** the screen called Player was a lesson sheet without a timeline, resume or controls; pointer navigation on mobile was not proven.
- **Result:** Rejected in the comparative evaluation.

## K — linear course-platform flow

K follows familiar learning-platform patterns without a marketplace or cloud: compact topbar, short resume strip, catalog in shelves, a full course page with goals and curriculum, and a player on a 16:9 stage. The identity used cobalt blue, saffron reserved for state, navy surfaces, and a monochrome wordmark. It reuses none of the earlier concepts' layouts.

- **Selection:** the independent critique of candidate K v5 scored 4.05/5, above the 4.00 minimum and with no dimension below 3/5. The points and limits are in [decision.md](decision.md); they are not a user-testing result.
- **Implementation:** [final-review.md](final-review.md) records the K screens. K's palette is gone; [`DESIGN.md`](../../DESIGN.md) describes the current design.

## Completion criterion

K was selected and implemented, and refinement ended when the capture review found no visual P0 or P1. Screenshots do not prove testing with people, certified accessibility, persistence with real data, or real playback, and none of those is claimed.
