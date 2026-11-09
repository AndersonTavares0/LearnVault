# ADR 0003: Replace the Svelte SPA with a React SPA

## Status

Accepted. This ADR supersedes the frontend part of [ADR 0001](0001-reescrita-em-rust.md), which chose
"a single SPA in Svelte 5 + TypeScript + Vite". Everything else in ADR 0001 still applies: the Rust
workspace, the Axum server, the Tauri desktop shell, and the shared core.

Tracking: epic #347, Phase 0 #334.

## Context

ADR 0001 kept Svelte because the previous SPA already used it. It recorded no evaluation of
alternatives. Two years of growth have exposed these problems:

- **Ecosystem.** Svelte has few mature, accessible component libraries, and few maintained media
  or document viewer components. Each accessible primitive (dialog, menu, tabs, combobox,
  tooltip) is hand-built.
- **CSS debt.** `apps/web/src/redesign.css` is 8.3k lines with three stacked `:root` token blocks,
  on top of a legacy `styles.css` and a daisyUI layer. The documented palette
  (`docs/design/DESIGN.md`) no longer matches the shipped tokens.
- **Structure.** `App.svelte` (about 1.4k lines) still uses legacy Svelte 4 reactivity and holds
  most screens inline. Stores use both `svelte/store` and runes.
- **Long-term maintenance.** React is used by roughly 45% of web developers (Stack Overflow 2025),
  compared with about 7% for Svelte. Contributors, hiring, documentation, and AI coding agents
  are all strongest on React.

Constraints that any replacement must keep:

- A static SPA served by Rust (`--web-root`) and embedded in Tauri. No Node server at runtime.
- Hash routing (`#/course/<id>/<path>?t=`), so links and bookmarks keep working.
- Two builds that select the transport at compile time: HTTP for the server, Tauri IPC for the
  desktop. The desktop bundle must not contain `fetch(`.
- The server CSP (`style-src 'self' 'unsafe-hashes' <hashes>`). No runtime `<style>` injection,
  no inline scripts, and no third-party requests.
- `tr(en, ptBr)` for every user-facing string.
- The course library stays strictly read-only.

## Decision

Rebuild the SPA with:

| Concern | Choice |
| --- | --- |
| UI | React 19, TypeScript strict |
| Build | Vite, with the same `__LEARNVAULT_TRANSPORT__` define and `desktop` mode |
| Routing | React Router, hash router |
| Server state | TanStack Query on top of the existing `Transport` interface |
| Styling | Tailwind CSS 4, with tokens in one file (`@theme`) |
| Components | shadcn/ui (source copied into the repo) on an accessible headless base |
| Lint | ESLint with `jsx-a11y`, `react-hooks`, `react/no-danger` |
| Tests | Vitest + Testing Library; Playwright + axe end to end |

The headless base for shadcn/ui is **Base UI**. A dialog and a menu from each candidate were opened
under the server's `style-src 'self' 'unsafe-hashes'` policy (Chromium, Playwright):

| Base | CSP violations | Injected `<style>` |
| --- | --- | --- |
| Radix (`@radix-ui/react-dialog` 1.3.1) | 1 | no (blocked inline style) |
| React Aria (`react-aria-components` 1.22.1) | 1 | yes |
| Base UI (`@base-ui/react` 1.9.0) | 0 | no |

Allowing `'unsafe-inline'` styles to fit Radix or React Aria would weaken the CSP for every user. Base UI
is also shadcn/ui's default base.

We rejected these alternatives:

- **Next.js.** Its value is SSR, server components, and a Node runtime. None of these applies
  to an app served by Axum and packaged offline in Tauri. A static export would add layers
  for no benefit.
- **Vue 3 and Angular.** Both are viable. Vue has a smaller accessible-component ecosystem and less
  agent support than React. Angular is heavy for an app of this size.
- **Staying on Svelte and only rewriting the CSS.** This fixes the CSS debt but not the ecosystem,
  hiring, or agent-productivity problems.

The framework-free TypeScript modules move over unchanged with their tests: `transport`, `catalog`,
`navigation`, `search`, `player`, `player-preferences`, `study`, `statistics`, `gamification`,
`pomodoro`, `focus-outbox`, `thumbnails`, `cover`, `session`, `update`, `theme`, `library-view`.
`i18n` keeps the `tr(en, ptBr)` signature and the `learnvault-locale` storage key.

## Consequences

- No backend change is needed to reach parity. The existing HTTP API and IPC commands cover every
  screen.
- The new app is built next to the old one in `apps/web-next` and replaces it in a single cutover
  (Phase 6, #340). Until then, `apps/web` remains the shipped UI.
- The server and desktop builds keep their output paths (`crates/server/web/build`,
  `crates/desktop/frontend-dist`), so packaging, the Containerfile stage, and Tauri config change
  only in package names.
- New runtime dependencies (React, React DOM, React Router, TanStack Query, and the headless base)
  follow `.agents/playbooks/dependencies.md`. Each needs a pinned version and a minimum release age.
- The 2.5D forest, nursery, and related components are not ported. Their data lives only in browser
  storage, so no server data is lost.
- The `tr()` location in `AGENTS.md` and `.agents/repository.md` changes at cutover.

## Validation and rollback

- Each phase PR runs lint, type check, Vitest, and both builds for `apps/web-next`, alongside the
  existing checks.
- Before cutover, the new build is dogfooded with `learnvault-server --web-root apps/web-next/dist`
  against real data.
- The cutover is gated by the parity checklist ([`docs/design/parity.md`](../../design/parity.md)),
  the accessibility checklist ([`docs/design/accessibility.md`](../../design/accessibility.md)),
  green Playwright end-to-end tests with axe, zero CSP violations, and `build-output.test.ts`.
- Rollback before cutover: delete `apps/web-next`. Rollback after cutover: revert the cutover PR.
  The backend and data formats are unchanged, so either version of the SPA works against the same
  database.
