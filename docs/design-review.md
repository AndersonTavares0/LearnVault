# LearnVault design review (direction K, historical)

> **Superseded (2026-11-11).** This review covers direction K on the Svelte app, which no longer exists.
> [`DESIGN.md`](../DESIGN.md) describes the shipped design, and the scores and reasoning behind the
> direction changes are in [`design/decision.md`](design/decision.md). The commands and screens named
> below belong to the removed app.

## Result

Direction K was implemented and its visual refinement closed with no confirmed P0 or P1 issue in the
final captures. The work was judged on screenshots of the real app against a synthetic library, not on
tests with users.

K is a course-first library without marketplace mechanics: compact global navigation, a short resume
strip, course shelves, a full course page, and a media-centred player.

## Exploration

Three independent proposals were compared before K:

| Concept | Average | Why it fell short |
| --- | ---: | --- |
| A, editorial | 2.90 | Incomplete navigation flows and player |
| B, workbench | 3.65 | Lost clarity on mobile and simulated its player actions |
| C, atlas | 2.90 | The map metaphor weakened the player's context and controls |

K scored 4.05/5 in an independent critique, with no dimension below 3. The gate was a weighted average
of at least 4.00. Details: [`design/decision.md`](design/decision.md), [`design/concepts.md`](design/concepts.md).

## Fixes from the final critique

- Mobile course page: title separated from the main action, full-width call to action.
- Player controls grouped in a compact strip, with the completion toggle next to them.
- Weekly goal moved before the streak history, and mobile metrics reorganized to wrap less.
- Search widened; an active search hides the unrelated resume strip.
- A lesson without a file shows a "Media missing" state that explains recovery and keeps progress.

## Later refinements (2026-10-29)

- **Touch cover control.** The `hover: none` rule that shows cover actions was overridden by the shared
  zero-opacity rule. Playwright reproduced `opacity: 0`; the fixed rule keeps the control visible with a
  44 by 44 px target.
- **Mobile library.** Cover metadata aligned with the card edge, 14 px between the selector and the grid,
  and Import and Refresh as two equal columns, checked at 320 and 390 px.
- **Resume strip.** `scroll-snap-type: x mandatory` replaced `proximity` for deterministic behaviour across
  Chromium and WebKit. The test uses a mouse wheel, not real touch, and shows no measured gain in Chromium.
- Seven of fourteen findings in a third critique did not hold up against the code and were not applied.

## Limits

No user testing, no WCAG 2.2 AA certification, no real-media validation, no check against real data, and
no interactive run of the desktop binary. Screenshot review does not replace any of these.
