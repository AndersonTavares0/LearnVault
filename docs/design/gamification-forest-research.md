# Gamification research: the Forest app model (removed feature)

> **Superseded (2026-11-11).** The forest, island, nursery, species, and focus coins were removed in the
> React rewrite ([ADR 0003](../architecture/adr/0003-react-frontend.md)). The shipped model is streak, weekly
> goal, daily missions, XP, and quiet badges ([`DESIGN.md`](../../DESIGN.md#gamification)), and
> [`PRODUCT.md`](../../PRODUCT.md) rules out coins and leaderboards. This file keeps the research and the
> removed proposal as history. On first load the app deletes the orphaned `learnvault-forest-nursery` and
> `learnvault-forest-session-seeds` browser keys.

## Sources

| Source | Use |
| --- | --- |
| Seekrtech, [Forest: Stay Focused](https://www.forestapp.cc/) | Planting, 10 to 120 minute growth, loss aversion (a withered tree), a coin economy, and a species nursery |
| Yu-kai Chou, *Actionable Gamification* (2015) | The eight Octalysis core drives, split into positive motivation and urgency or loss aversion |
| Francesco Cirillo, *The Pomodoro Technique* (2006) | 25 minute focus, 5 minute short break, 15 minute long break |
| W3C, [WCAG 2.2](https://www.w3.org/TR/WCAG22/) | Contrast, keyboard operation, `prefers-reduced-motion`, and `aria-live` announcements |

## What Forest does

A user picks a seed and a duration. Staying focused grows the plant through four stages (25% steps). Leaving
the session kills it, and the withered tree stays in that day's plot as a reminder. A healthy session earns
coins proportional to the minutes, and coins unlock new species. The coins buy cosmetics only.

## What was proposed for LearnVault

A local "Study Forest": a focus-session table and a rewards table in `crates/core`, a species catalog
(Sapling and Oak free, then Pine, Cherry, Willow, Ginkgo, and an Ancient Tree at 40 to 350 "Focus Drops"),
a seed selector before the timer, a real-time growth animation with a withered variant on abort, procedural
rain and sea soundscapes, and `aria-live` announcements for planting, completion, and abandonment.

## Why it was dropped

Direction L put habit and visible progress ahead of points and loss mechanics, and the forest was removed
([decision.md](decision.md)). Focus sessions, XP, missions, and badges stayed on the shared SQLite domain.
