# LearnVault — visual directions

> **Superseded in part (2026-11-11).** The palette, themes, navigation and gamification described here were replaced by the direction in [`DESIGN.md`](../../DESIGN.md) and [`PRODUCT.md`](../../PRODUCT.md): light and dark themes, brown and amber, Playfair Display and DM Sans, top navigation, no forest. Where this file names a palette, a theme or a layout, those two files win. It is kept as history and for the research and the accessibility and security rules, which still apply.

## Scope and evidence

Prototypes A/B/C were started as independent empty documents and compared on Library, Course and Player, desktop and mobile. The Open Design MCP was used to create/retrieve HTML artifacts; the observed integration does not expose a native vector canvas. The captures below are local renders of those artifacts in Chromium, with synthetic data. They prove the composition, not playback, persistence or product behavior.

The verifiable research and the functional inventory are in [research.md](research.md). The weighted evaluation and the decision are in [decision.md](decision.md). The three initial concepts did not pass the threshold; K was chosen after later exploration and a documented independent critique.

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

K brings the navigation closer to familiar learning-platform patterns without incorporating a marketplace or cloud: compact topbar, short resume strip, catalog in shelves, a full course page with goals and curriculum, and a Player centred on a 16:9 stage. The identity uses a functional cobalt blue, saffron reserved for state, navy surfaces and a monochrome wordmark. The direction does not reuse the layout of the earlier concepts.

- **Selection:** the independent critique of candidate K v5 scored 4.05/5, above the 4.00 minimum and with no dimension below 3/5. The points and limits are in [decision.md](decision.md); they are not a user-testing result.
- **Real implementation:** final screens, themes, viewports and states are formalized in [DESIGN.md](DESIGN.md) and [final-review.md](final-review.md).

## Completion criterion

The direction is selected and implemented. The final refinement ended when the review of the captures found no visual P0/P1; limits that screenshots do not prove — testing with people, certified accessibility, persistence with real data and real playback — remain explicitly outside the claims.
