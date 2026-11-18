# LearnVault — design review

## Result

**Direction K implemented; visual refinement closed in this review.** There is no confirmed P0/P1 in the final captures. No new visual directions will be opened without a reproduced functional failure or accessibility blocker.

The preference for a familiar course-platform experience led to a course-first library with no marketplace: compact global navigation, a short resume strip, course shelves, a full course page and a content-centred Player. The exploration compared three independent proposals — editorial, workbench and atlas — before the choice. The matrix and the reasoning are in [`docs/design/decision.md`](design/decision.md); the audience, the research and the observed patterns are in [`docs/design/research.md`](design/research.md).

## Exploration and decision

- **A — editorial:** topbar, publication-style composition and covers; it failed in navigation flows and the Player.
- **B — workbench:** rail + topbar and high density; it organized the session well, but lost clarity on mobile and simulated actions.
- **C — atlas:** dock and learning map; it differentiated the identity, but did not sustain the Player's context and controls.
- A/B/C received averages of **2.90**, **3.65** and **2.90/5**, below the 4.00 gate. K reached **4.05/5** in the independent critique, with no dimension below 3/5. The score is not user testing.
- Direction K v7 was implemented on the six surfaces: Library, Course, Player, Statistics, Settings and Sign in. The decisions are in [`docs/design/concepts.md`](design/concepts.md); tokens and components in [`docs/design/DESIGN.md`](design/DESIGN.md) and [`DESIGN.md`](../DESIGN.md).

## Visual critique and final fixes

The final serial review of the real captures found competition between title and action on mobile Course, a tall Player bar, a completion state separated from the controls, a weekly goal far from its context and a narrow mobile search. Fixes applied:

- Mobile Course reorganized: title/context separated from the main action, full-width CTA.
- Player controls grouped in a compact strip; completion and its related action stay close together.
- Weekly goal moved before the streak history; mobile metrics reorganized to reduce wrapping and height.
- Search widened; when searching, the resume strip unrelated to the query disappears.
- Empty state uses a stable title and a separate explanatory message; an empty search keeps "Clear search" and leaves no contradictory cards/recents.
- A lesson without a file gets a "Media missing" state; the Player explains recovery, does not offer unavailable playback and preserves progress.

The final visual gate found no P0/P1. Parts below the fold on the Course/Statistics screens were not assessed by the captures; the next card in the resume strip appears partly cut off by horizontal scrolling, but the gesture was not tested. This is a limit of the evidence, not a claim of validated behavior.

## Evidence and validation

The visual checks of direction K were carried out in a staging environment with isolated synthetic library and media, covering:
- Library, Course, Player, Statistics and Settings at 390px, 1280px, 1440px and 1920px in the light and dark themes.
- Sign-in, missing-media and no-results-search states (no orphan cards, with the clear-search action visible).
- No horizontal overflow and compliance with the minimum 44×44px targets.
- Evidence and comparative captures recorded in the PR history; the official production vector identity is fixed in `docs/design/branding/learnvault-banner.svg`.

## Checks

- `pnpm --filter learnvault-web check`: passed, 0 errors and 0 warnings.
- `pnpm --filter learnvault-web test`: passed, 22 files / 313 tests.
- `pnpm exec playwright test tests/e2e/server.spec.ts --retries=0`: passed, 4/4 scenarios.
- `pnpm --filter learnvault-web build` and `pnpm --filter learnvault-web build:desktop`: passed.
- `git diff --check`: passed; Git only warned about LF→CRLF conversion in files of the Windows checkout.
- The final visual detector returned `[]` for the UI targets.
- The PR's remote check failed only on `pnpm audit --audit-level=low` because of a low transitive advisory in `deepmerge` (`GHSA-gcqx-w48m-w2h4`), via `@sveltejs/vite-plugin-svelte`; the audit reports no fixed version. It was not suppressed or changed as part of the visual change.

## Limits

No user testing, no WCAG 2.2 AA certification, no real-media validation, no persistence check on real data and no interactive native run of the desktop binary. The specialized review of screenshots does not replace these tests.

## Later refinement — 2026-10-29

### Critique and decision

An independent visual review of the 390 px capture pointed out the catalog below the first useful fold and the horizontal resume strip partly cut off. The decision was to preserve the resume → catalog order and the compact covers already present on mobile; making Compact the default would hide the covers' identity without bringing the start of the catalog closer. The refinement reduced only the vertical spacing of the introduction, search and sections up to 620 px.

The implementation review found a confirmed functional regression: the `hover: none` rule that showed the cover actions was overridden further down by the shared zero-opacity rule. The problem was reproduced in Playwright before the fix (`opacity: 0`). The final touch rule now keeps the control visible and with a 44×44 px target; mouse and keyboard focus keep their existing states.

### Visual and interaction evidence

- Isolated-flow validations on a 390×844 mobile viewport: touch cover control visible and cover menu operable.
- This smoke's fixture contains one course and no resumes; the test shows the reduced spacing and the visible, operable touch control.
- The touch test confirms `hover: none`, opacity 1, a 44 px target and the menu opening by touch.

### Verification

- Failure reproduced before: Playwright expected the touch control to be visible and received opacity 0.
- After the fix: `pnpm exec playwright test tests/e2e/server.spec.ts --retries=0` passed, 5/5 scenarios.
- `pnpm --filter learnvault-web test`: 22 files / 313 tests; `check`: 0 errors / 0 warnings; web build and desktop build passed.
- Impeccable detector run on `src/redesign.css`. The report contains legacy token/radii/typography warnings in other parts of the sheet; no warning points to the new rules at the end of the file. The earlier CSS was not reformatted or changed outside the scope.

### Integration limitation

The Open Design MCP is not exposed as a usable tool in this runtime; there was no call to it and no evaluation attributed to it in this iteration. The independent critique came from two separate agents and the visual/interactive validation came from Playwright. No Open Design evaluation was invented.

## Revised diagnosis of the Open Design MCP — 2026-10-29

The previous note correctly described that this conversation's runtime did not provide a top-level MCP tool, but wrongly concluded that the integration was not usable. The user's configuration is active and the real STDIO bridge responded; no configuration or credential was changed.

- `~/.omp/agent/mcp.json` contains `open-design` enabled, STDIO transport and a 120 s timeout. The Open Design process was active and its local daemon was listening on the configured port.
- The `2025-11-25` MCP handshake passed through the configured command. `tools/list` returned 22 tools; `list_projects` and `get_project` answered for `learnvault-concept-k-course-library-fresh-20261029`.
- `get_active_context` reported that there was no active project at that moment. The project can be selected explicitly in the `project` argument, without activating or changing the canvas.
- `mcp://opendesign` queries MCP resources, not the tool list; the absence of resources is normal on this server and does not indicate a connection failure.
- This investigation did not create or edit projects or artifacts.

## Mobile Library refinement — 2026-10-29

The independent review of the real captures (390×844 light/dark and 1440×900 light; Open Design run `0a06d008-94e5-4c67-9e39-871a311c7798`) found no P0/P1. It recorded three observable P2s: mobile cover metadata visually indented, 8 px between the selector and the grid, and CTAs taking 310 px of a 366 px useful column.

Fixes applied:

- Mobile covers now stretch the content across the card width; the title and metadata align with the cover and gain room for long names.
- The gap between the selector and the grid rises to 14 px.
- Import and Refresh form two equal columns across the available width; below 340 px, padding and font shrink, with both labels checked on a 320 px viewport.
- Import keeps a secondary hierarchy when there is a resume and becomes primary again with no resume or during a search.

The E2E checks the title alignment (≤16 px), the gap (≥12 px), column equality and text without overflow on 320/390 px viewports; it also waits for CSS transitions to finish before measuring contrast so as not to read intermediate values. Verified on synthetic fixtures with captures recorded in the PR history.

### Second visual critique

Serial re-evaluation after the fixes in Open Design run `3b1428ea-0000-48c3-8fb7-8d715b79987d`: approved, with no P0/P1. The second round confirmed CTAs in two 179 px columns with an 8 px gap (before 148/154 px), 14 px between the selector and the grid (before 8 px) and cover content aligned with an 11 px inner inset. The P2 caveat about long titles was treated as a hypothesis to verify, not as a confirmed defect.

An E2E with a long Portuguese title confirmed the current handling: 15 px serif text, a two-line limit with an ellipsis and no excess width in the card; on opening the course, the full title appears in the header. The abbreviation on the cover is deliberate to keep the grid compact; the full content stays accessible in the Course flow.

### Final verification

- `pnpm --filter learnvault-web check`: 0 errors, 0 warnings.
- `pnpm --filter learnvault-web test`: 22 files, 313 tests.
- `pnpm --filter learnvault-web build` and `pnpm --filter learnvault-web build:desktop`: passed.
- `pnpm exec playwright test tests/e2e/server.spec.ts --retries=0`: 6/6 scenarios; includes CTA without overflow at 320/390 px, touch controls, a long title without overflow and access to the full title on the Course screen.
- The two rounds of independent critique found no P0/P1; the measurable P2 points were fixed and the hypothesis about long names was reproduced, mitigated and verified.
- The validation does not cover real personal data and is not equivalent to user testing.

## Third round of critique and the resume strip — 2026-10-29

A third independent reading of the same 390×844 captures proposed fourteen findings. Seven did not hold up when checked against the code:

- **Contrast of the selector and borders in the dark theme:** the tokens `--blue-soft`, `--blue`, `--ink` and `--line` already deliver about 13:1 for the selected text and 7:1 for the border; replacing them with fixed hex values would have left the palette. No change applied.
- **Flattened type scale:** the page `h1` is 32 px and the section `h2`s are 24 px on mobile; there is a step. No change applied.
- **Title truncated to two lines:** it is a recorded decision, with the full title in the Course header and E2E coverage. Kept.
- **CTA parity:** the primary/secondary alternation is intentional when there is a resume and is verified by E2E. Kept.
- **Status badge:** "In progress" and "Catalog updated." have different semantics (a persistent state and a transient notice); unifying them would be abstraction without demand.
- **Search pinned while scrolling:** the topbar wraps onto two lines at ≤820 px with different minimum heights, so any fixed `top` hides the field behind it. Without JS or a layout reorganization, the alternative would be an incorrect overlay. Discarded.
- **Loading skeletons:** the catalog is loaded once; there is no asynchronous path that justifies a shimmer.

The finding that corresponded to a real gap already documented was the resume strip on mobile: horizontal scrolling had never been exercised. The fix was `scroll-snap-type: x mandatory` (before `proximity`), chosen for deterministic behavior across Chromium and WebKit, not for a gain measured in Chromium.

The new E2E uses a fixture with two lessons and four resumes, makes a real horizontal wheel gesture over the strip, confirms `scrollLeft > 0`, checks that the resting position matches the first card's reference alignment (≤1 px) and that each card, when reached, stays inside the strip's edges. To prove the test is not empty, the strip was temporarily changed to `overflow-x: visible`: the scenario failed with `scrollLeft` at 0, as expected.

Limits of this verification: the gesture is a mouse wheel, not real touch; `proximity` also passes on the same test design, so the choice of `mandatory` has no evidence of superiority in Chromium, only of determinism across engines. Evidence recorded in the E2E test history.

### Verification

- `pnpm --filter learnvault-web check`: 0 errors, 0 warnings.
- `pnpm --filter learnvault-web test`: 22 files, 313 tests.
- `pnpm --filter learnvault-web build` and `build:desktop`: passed.
- `pnpm exec playwright test tests/e2e/server.spec.ts --retries=0`: 7/7 scenarios.
