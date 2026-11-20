# Visual direction decision

> **Superseded in part (2026-11-11).** [`DESIGN.md`](../../DESIGN.md) and [`PRODUCT.md`](../../PRODUCT.md) replaced the palette, themes, navigation and gamification described here (light and dark themes, Playfair Display and DM Sans, top navigation, no forest). Where they disagree with this file, they win. The file stays as history; its accessibility and security rules still apply.

> **Brand superseded (2026-11).** The Academy Portico mark and the *Sapere aude* motto replaced the LV monogram mentioned below. See the Brand section of [`DESIGN.md`](../../DESIGN.md) and [`branding/`](branding/).

## Criterion

Concepts were evaluated on screenshots, not in tests with users. Approval requires a weighted average ≥4.00/5 and no dimension below 3/5:

| Dimension | Weight |
|---|---:|
| Usability and clarity | 30% |
| Hierarchy and visual architecture | 25% |
| Identity and aesthetic quality | 20% |
| Consistency and accessibility | 15% |
| Technical feasibility | 10% |

## A/B/C — three independent proposals

| Concept | Usability | Hierarchy | Identity | Consistency | Feasibility | Average | Decision |
|---|---:|---:|---:|---:|---:|---:|---|
| A — Editorial library | 2 | 3 | 4 | 3 | 3 | **2.90/5** | Rejected |
| B — Study workbench | 4 | 4 | 3 | 3 | 4 | **3.65/5** | Rejected |
| C — Learning atlas | 2.5 | 3 | 4 | 2 | 3 | **2.90/5** | Rejected |

B organized the curriculum, the current lesson and the session better, but the Player simulated actions, did not persist progress and made "Save" ambiguous on mobile. A had an editorial identity, but incomplete navigation/player and no essential flows. C was visually distinctive, but the spatial metaphor weakened resuming, controls and pointer navigation. None met the threshold; the proposals were discarded, not refined as a final solution. See the A/B/C captures in [concepts.md](concepts.md).

## K — the chosen proposal

Direction K uses recognizable course-platform patterns with a local-first architecture: a Library in shelves with a short resume strip, a Course as a full page with goals/progress/curriculum and a media-centred Player with a contextual curriculum. It avoids a marketplace, ratings, enrollment, decorative metrics and dependence on a remote account. A compact topbar instead of a persistent sidebar; the global destinations fit on wide screens and reorganize on mobile.

The independent critique of candidate K v5 scored:

| Dimension | Weight | Score | Share |
|---|---:|---:|---:|
| Usability and clarity | 30% | 4.0 | 1.20 |
| Hierarchy and visual architecture | 25% | 4.2 | 1.05 |
| Identity and aesthetic quality | 20% | 4.1 | 0.82 |
| Consistency and accessibility | 15% | 3.5 | 0.525 |
| Technical feasibility | 10% | 4.5 | 0.45 |
| **Total** | **100%** |  | **4.045 ≈ 4.05/5** |

**Gate passed.** The evaluation was specialized and blind to the justifications/code; it is not equivalent to user testing. The critique of the real implementation is a distinct stage: the final screenshot cycle found no P0/P1, with the limitations of content below the fold and the carousel gesture stated in the [final review](final-review.md).

## Reasons for the selection

- It is familiar from course platforms but has its own identity, and it leaves out marketplace features that do not fit a local library.
- It puts the next step and resuming before metrics, preserves the course → lesson → progress context and keeps the 16:9 stage as the Player's main area.
- Cobalt indicates action; saffron indicates position/state. The LV wordmark is monochrome and discreet.
- The interface was in pt-BR at the time. Titles and descriptions from the files keep their original language.
- The K v7 implementation and the token system are formalized in [DESIGN.md](DESIGN.md); the evidence of the final review is recorded in [final-review.md](final-review.md).

## Accepted risks and limits

- K's score came from candidate v5; later versions adjusted composition and content. The final implementation received a separate visual evaluation, with no apparent new P0/P1.
- Screenshots do not demonstrate full keyboard/screen-reader navigation, real playback, persistence beyond the fixture, or quality across all of the user's content.
- There was no research with participants and no WCAG 2.2 AA certification. The automated coverage and its limitations are in [final-review.md](final-review.md).

**Decision:** keep K as the final direction and do not open another visual exploration without a confirmed functional failure or accessibility blocker.

## Post-K review (reopening, see #330)

Three maintainer-confirmed functional/accessibility failures satisfy the reopening clause above: #317 (layout overflow at ~1270px), #319 (language toggle unreachable at desktop widths), and #324 (27 interactive elements below the WCAG 44×44px target). This authorizes a composition revision within K — library, course page, and Player — using course platforms as a structural reference only, never as a source of code, assets, or pixel-level layout (reference policy, [`.github/CONTRIBUTING.md`](../../.github/CONTRIBUTING.md)). The original A/B/C/K evaluation above remains a historical record and is not redone.

## Direction L: course-platform redesign on React (reopened, see #347)

On 2026-11-09 the maintainer reopened the visual direction as part of the React migration
([ADR 0003](../architecture/adr/0003-react-frontend.md)). The reasons were recurring layout and target-size failures under K (#317, #319, #324, #321), CSS debt that blocks further fixes, and a
documented palette that no longer matches the shipped one. Direction L replaces K. The A/B/C/K
records above stay as history.

L keeps K's product stance (local-first, no marketplace, ratings, or enrollment). It changes:

- **Navigation.** A topbar holds the global destinations (today My learning, Review, Progress, and Notes) and the account menu. A curriculum sidebar appears only inside the course page and the player. Mobile uses a bottom tab bar. Research behind this choice: course platforms keep curriculum and completion next to the lesson, and Open edX is restoring the course sidebar it removed in 2017.
- **Palette (superseded).** L started from shadcn/ui themes (Stone + Emerald by default, Zinc + Teal, Zinc + Indigo). The shipped palette is the warm one in [`DESIGN.md`](../../DESIGN.md), with eight surfaces and nine accent presets.
- **Gamification.** Habit and visible progress come first: a forgiving streak, the weekly goal, per-course progress, and the next lesson one click away. XP, levels, and achievements stay on the Progress screen. The forest, island, and nursery are removed.
- **Accessibility and security** are written requirements. See [accessibility.md](accessibility.md) and [security.md](security.md).

The screen-level spec is in [redesign.md](redesign.md); `DESIGN.md` replaced it at cutover.

**Gate.** L was to be scored with the same rubric and threshold (weighted ≥ 4.00, no dimension below 3), by a blind critique of the implemented React screens before cutover (#340). No L score is recorded in this repository. The cutover is complete and `DESIGN.md` describes what ships.
