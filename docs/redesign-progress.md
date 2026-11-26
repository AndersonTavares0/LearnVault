# Premium interface refinement

Working tree: `redesign/premium-ui`, based on `8a28dcf4`. The before state is the existing premium working tree, not `main`. This is a presentation-only pass. Preserve course discovery, ordering, progress, authentication, theme preferences, API and routes.

## Stack and references

React 19, Vite, Tailwind CSS 4 and shadcn source components on Base UI; Lucide icons. `apps/web-next/src/styles/app.css` defines semantic colors and states, `src/lib/appearance.ts` applies theme/preferences, and `src/lib/i18n.ts` provides `tr(en, ptBr)`. The shipped default language remains English; the audit is in Brazilian Portuguese. See [reference board](references.md).

## Audit and acceptance

| Surface / finding | Severity | Status / acceptance |
| --- | --- | --- |
| Branding: redundant pictorial mark beside the product name | Cosmetic | Resolved: text-only LearnVault in the shell, sign-in, About and desktop titlebar; removed the mark from the empty library. Favicon and native icon remain. |
| Library and course: separate action and cover accents, gold dark accent, saturated blue square from the local fixture after indexing | Important | Resolved for this pass: the graphite and muted-brass dark theme remains; near-uniform cover images leave the keyed monogram visible in either theme. Light Ink remains available and unchanged. |
| Course: pluralized UI already uses i18n; `library` is the fixture's source folder name, not English UI copy | Important | Confirmed distinction; preserve actual names and user-defined categories. Use `aula/aulas` consistently. |
| Course: alphabetical claim contradicted by numbered parent paths and explicit order; vague warning | Important | Resolved: removed the numbering-only warning, kept actionable file-placement notices, and left sorting unchanged. |
| Course: `You are here` can appear on an untouched module because the next lesson drives the active path | Important | Resolved: the overview says Next module. Only the player marks the actual current lesson. |
| Course: equivalent module/lesson titles repeat | Cosmetic | Resolved: the lesson row uses Open lesson with a contextual accessible name when its displayed title duplicates its sole module header. |
| Library: zero-valued habit cards and empty saved list compete with coursework; rare-course whitespace | Important | Resolved: first-time activity has a short prompt, populated stats retain the 2×2 grid, and empty saved lessons use a quiet hint. |
| Library: wrapped status filters and tiny counts; card titles compete with controls | Important | Resolved: horizontal status filters with count pills and roomier card columns until 1536 px. |
| Player: persisted hidden contents cannot be restored on mobile | Important | Resolved: the existing contents toggle is reachable on mobile with the same state and persistence. |
| Notes, course and progress: some touch targets below 44 px; notes at tablet widths squeeze prose | Important | Resolved for touched controls: 44 px minimum; move note metadata under prose at narrow and intermediate widths. |
| Progress: switch changes total focus, not the weekly chart | Important | Resolved: period is next to focus total; chart remains explicitly weekly. Query and aggregation unchanged. |
| Review, Notes, History, Login and Settings: typography, surface and primary-action drift | Cosmetic | In progress: shared tokens and selected local fixes; preserve existing features and states. |
| Search: mobile shortcut and keyboard focus paths may not open/focus the visible search | Important | Pending behavior approval; baseline audit used the existing mobile Search button. |
| History: a failed history request can be presented as genuinely empty | Important | Pending behavior approval; do not add a recovery flow in a visual-only pass. |
| Keyboard, hover and themes: some standalone links darken the dark accent under dark text | Important | In progress: reuse the primary button variants; measure both themes. |

## Sequence and evidence

1. **Research and baseline — partial.** Primary-source reference board and independent source critique complete. The transient baseline is `/tmp/lv-redesign-baseline/`. Verify the theme against `<html data-theme>` when comparing: the baseline run changed browser preference but the app default stayed light, so only its `*-light-*` files are a valid before set.
2. **Wordmark — implemented.** `app/shell.tsx`, `pages/login.tsx`, `pages/home.tsx`, `pages/settings/about.tsx` and `components/desktop-titlebar.tsx` no longer show the logo or seal. `components/brand.tsx` retains the localized mottos. Updated `DESIGN.md` and the About regression test.
3. **Shared primitives and screen refinements — pending.** Theme defaults, linked buttons, cover fallback, scale, count pills and other findings in the audit table have not been implemented in the wordmark pass. Verify populated review/note/history states, loading and injected errors in a later pass; the main matrix used fresh synthetic accounts.
4. **Wordmark validation — passed locally.** ESLint, TypeScript, Vitest (711 tests), Vite build, and Playwright About (desktop 1440 and mobile 320 px, both themes, with English and Portuguese assertions). `/tmp/lv-redesign-branding-verified/` contains 56 captures plus empty and 65-course mobile library captures; the report confirms 36 axe checks with no violations, no uncaught errors, and no document/main horizontal overflow at 1300/390 px in actual light/dark themes. This does not certify hover contrast, every preference, or screen-reader usability. Native titlebar was type-checked, not visually exercised in Tauri. The independent branding review found no implementation mismatch. Its documentation finding was corrected, and its capture-theme finding was addressed by the explicitly verified run above. No commit, push, deploy or image artifacts in Git.

### Dark palette follow-up

Implemented in `styles/app.css` and `lib/appearance.ts`; palette and settings tests updated. Light tokens and stored preferences are unchanged. ESLint, TypeScript, Vite, and all 711 Vitest tests passed. The browser run in `/tmp/lv-redesign-graphite/` confirmed both actual themes at 1300/390 px and returned no axe violations across 36 settled-state checks. Token-pair contrast: body/card 13.32:1, secondary/overlay 6.65:1, primary button 9.23:1, accent/panel 7.11:1, input outline/overlay 3.31:1. This is not a certification of every state or custom palette.

### Critical refinement rounds

1. **Palette and hierarchy.** Compared the graphite baseline on desktop and mobile. Removed the blue-cover dominance with a presentation-only image-detail check, plus themed sage, clay, slate, and ochre fallback covers keyed by course ID. Detailed images still display; unreadable pixels fail open; no source or stored image is changed. Removed the course backdrop blur instead of tinting it further.
2. **Layout and copy.** Compared the first revision at 1300/390 px. Reduced the mobile course header to a compact cover/title row and two levels of actions; simplified status and structure copy; improved narrow-note reading measure; put the focus period beside the total it changes. Kept data, ordering, routes and mutations intact.
3. **Reflow and verification.** Captures use synthetic courses and the application's actual theme. Desktop and phone: `/tmp/lv-redesign-critique-final/`; tablet and narrow phone: `/tmp/lv-redesign-critique-widths/`. Independent source and capture review found no major remaining issue in scope. It did not exercise populated notes or cover transport edge cases. The automated coverage for the cover is a real solid-frame sample plus component tests with synthetic pixel data; transparent and unreadable cross-origin covers were not exercised in a browser. The final build, ESLint, TypeScript and 712 Vitest tests passed. Playwright passed all 51 tests before the final accessible-name adjustment; the affected module disclosure passed again afterwards. Final browser evidence: 112 matrix captures, 72 axe checks with no violations, and no overflow in the document or main content at the tested widths. The mobile filters were exercised by keyboard. No publish, commit, or deployment.

## Decision log

- Use the product name alone as in-app branding, following the maintainer’s direction. Keep icons used by the browser tab and operating system.
- Dark-mode follow-up: replace the near-black brown canvas with neutral graphite and reserve muted brass for actions, focus, and rewards. This gives surfaces a clearer hierarchy without repainting the light theme, saved accent choices, or course imagery.
- Preserve the built-in surface/accent system rather than creating a separate premium theme. Use the requested editorial hierarchy and clay/brass default pair; preserve existing saved choices. The references favor a quiet canvas with a single clearly actionable emphasis.
- Keep course content and folder-derived labels in their original language. Prefer `aula` for interface copy, including singular and plural. Avoid rewriting titles or changing filter keys.
- Keep the review and activity models. A blank state should explain what can happen next, not invent data. As in the reference board, distinguish a course not yet started from unknown or unavailable data.
- Prefer a restrained 44 px cover and textual title to a broad banner. Keep original file covers available; presentation fallback does not write course files or the thumbnail cache.

## Skills and limitations

Impeccable and Frontend Design informed hierarchy and the critique; Web Interface Guidelines and the repository checklist supplied accessibility criteria; shadcn and React Best Practices informed component reuse; Research produced the cited reference board; Diagnosing Bugs and TDD informed focused checks; Code Review informs the independent pass. The named `superpowers:*` and `design:*` skills were not installed. No Firecrawl design extraction or third-party assets were used. The Impeccable qualitative source critique and the rendered technical audit are separate evidence; no subjective score is asserted.
