# LearnVault agent instructions

LearnVault organizes course folders that the user already owns into a self-hosted study space,
without moving or altering the original files. The product is small, sustainable, and self-contained:
Rust server (`crates/server`, Axum, SQLite), Svelte 5 SPA (`apps/web`), shared core domain
(`crates/core`), and offline desktop application Tauri 2 (`crates/desktop`). Docker/Podman are
optional deployment adapters, not runtime requirements.

## Mandatory rule for every agent

Every agent working in this repository must follow this instruction file, read
[`.agents/repository.md`](.agents/repository.md), and then read the relevant policies,
playbooks, architecture documents, and checklists before changing anything. No agent may
skip, replace, or create a parallel project workflow. The repository rules apply equally
to implementation, review, planning, documentation, and automation agents.

OpenCode loads this file automatically. Project-specific reviewer profiles live in
`.opencode/agents/`; shared policies, playbooks, checklists, and templates live in
`.agents/`.

## Commands

```sh
export PATH="$HOME/.cargo/bin:$PATH" # ensure Cargo tools are in PATH
pnpm install --frozen-lockfile       # once per checkout
pnpm dev                             # Rust API at http://127.0.0.1:8090 + live reload SPA
pnpm dev:tauri                       # desktop application (Tauri) during development
pnpm verify                          # PR checks (SPA + Rust + desktop); --fast skips builds and audit
pnpm run check                       # SPA linting and type checking
pnpm --filter learnvault-web test    # Vitest web unit tests
pnpm exec playwright test            # Playwright end-to-end browser tests
pnpm run test:scripts                # node --test scripts/*.test.mjs (runs in CI on every PR)
pnpm run build / build:desktop       # SPA and desktop bundle builds
pnpm audit --audit-level=low         # dependency audit
cargo fmt --all -- --check           # Rust code formatting
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Rust version is pinned by `rust-toolchain.toml`; Node.js 24 and pnpm 11.28.5 come from
`packageManager` in `package.json`. Docker is optional.

## Conventions

- Documentation, code comments, pull requests, and commit messages are in English (`en`).
  Conventional Commits format is required (e.g., `feat(i18n): set English as project default`).
- The application supports internationalization (i18n) with English (`en`) as the default language
  and Brazilian Portuguese (`pt-BR`) fully supported. All user-facing strings (UI copy, modals,
  validation alerts, error fallbacks, and transport messages) must use `tr(en, ptBr)` from
  `apps/web/src/i18n.svelte.ts` (the React app, `apps/web-next`, has the same function in `src/lib/i18n.ts`). Raw untranslated strings in user-facing UI or error toasts are forbidden.
- **Mandatory use of Ponytail:** on all LearnVault development tasks, always use Ponytail to write,
  modify, and refactor code. Keep code clean, consistent with existing architecture, and focused on
  surgical changes without unnecessary refactorings or bloat.
- The course library is strictly read-only: no change may write, move, rename, or copy original course files.
- Release Please follows Conventional Commit version bumps before `1.0.0`: `feat:` increments the
  minor version, `fix:` and `perf:` increment patch, and breaking changes increment minor. From
  `1.0.0`, breaking changes increment major. Release tags and names use `vMAJOR.MINOR.PATCH`.
- Release Please prepares one release candidate on the first day of each month (UTC). Merge its
  release PR to publish; urgently needed security releases can prepare a candidate early by
  manually running the `Release Please` workflow on `main`.
- GitHub Wiki is permanently removed; do not create wiki sync scripts, workflows, or documentation.
- Never include secrets, personal course data, database files, or generated media in commits,
  logs, or prompts. Tests use temporary directories (`tempfile::TempDir`, `Database::open_in_memory()`)
  and synthetic fixtures.
- **Repository hygiene and zero media bloat:** Never commit screenshots, prototypes, binary mockups,
  or AI tool configurations (`.impeccable/`, `.claude/`, etc.) to the repository. Visual review artifacts
  belong in PR comments/descriptions, never in git tracking. Vector branding is maintained strictly in
  SVG format under `docs/design/branding/`. Every PR runs automated hygiene validation in CI (`scripts/repo-hygiene.mjs`).

## Multi-agent collaboration

Before starting work, follow [`.agents/policies/agent-collaboration.md`](.agents/policies/agent-collaboration.md)
for repository-state checks, ownership, delegation, integration, and independent review
of security, persistence, and release/update changes. The same policy applies to every
agent and harness working in this repository.

## Repository map

- `crates/` — Rust workspace: `core` (shared domain, catalog scanner, media identity, study engine, SQLite),
  `server` (Axum HTTP server, authentication cookies/sessions, media streaming, SPA delivery), `desktop` (Tauri 2 offline desktop application).
- `apps/web/` — Svelte 5 + TypeScript + Vite SPA, pnpm workspace member (`crates/server/web/build` and `crates/desktop/frontend-dist`).
- `docs/` — Canonical documentation; navigate via [`docs/README.md`](docs/README.md).
- `.agents/` — Shared policies, playbooks, checklists, and templates for autonomous and human agents.
- `.opencode/agents/` — OpenCode-specific reviewer profiles.
- `deploy/`, `scripts/`, `testdata/` — Deployment adapters (Containerfile, Docker Compose, Podman Quadlet), CI automations, and synthetic fixtures.
