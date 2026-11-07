# LearnVault agent instructions

LearnVault organizes course folders that the user already owns into a self-hosted study space,
without moving or altering the original files. The product is small, sustainable, and self-contained:
Rust server (`crates/server`, Axum, SQLite), Svelte SPA (`apps/web`), shared core domain
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
pnpm install --frozen-lockfile   # once per checkout
pnpm dev                         # Rust API at http://127.0.0.1:8090 + live reload SPA
pnpm dev:tauri                   # desktop application (Tauri) during development
pnpm verify                      # PR checks (SPA + Rust + desktop); --fast skips builds and audit
pnpm run check                   # SPA linting and type checking
pnpm run test:scripts            # node --test scripts/*.test.mjs (runs in CI on every PR)
pnpm run build / build:desktop   # SPA and desktop bundle builds
pnpm audit --audit-level=low     # dependency audit
cargo fmt --all -- --check       # Rust code formatting
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Rust version is pinned by `rust-toolchain.toml`; Node.js 24 and pnpm 11.28.5 come from
`packageManager` in `package.json`. Docker is optional.

## Conventions

- Documentation, code comments, pull requests, and commit messages are in English (`en`).
  Conventional Commits format is required (e.g., `feat(i18n): set English as project default`).
- The application supports internationalization (i18n) with English (`en`) as the default language
  and Brazilian Portuguese (`pt-BR`) fully supported.
- **Mandatory use of Ponytail:** on all LearnVault development tasks, always use Ponytail to write,
  modify, and refactor code. Keep code clean, consistent with existing architecture, and focused on
  surgical changes without unnecessary refactorings or bloat.
- The course library is strictly read-only: no change may write, move, or copy original course files.
- Never include secrets, personal course data, database files, or generated media in commits,
  logs, or prompts. Tests use temporary directories and synthetic fixtures.

## Multi-agent collaboration

LearnVault is developed with help from multiple agents and harnesses (OpenCode,
OMP/Pi, Antigravity CLI / Agy, Codex, ClaudeCode), often concurrently. These rules apply to every
agent, without requiring any shared orchestration infrastructure:

- **Verify before acting** — inspect `git status`, branches, worktrees, recent
  history, and in-flight PRs/issues before starting. Never assume you are the only
  active agent.
- **Preserve others' work** — never overwrite, discard, revert, or commit changes
  authored by another agent without their authorization or an explicit maintainer
  instruction.
- **Avoid conflicts** — do not duplicate tasks already in flight and do not edit the
  same files simultaneously. When in doubt, work in an isolated branch or worktree.
- **Coordinate through repository state** — use Git, issues, PRs, Discussions, and
  the documents under `.agents/` as the source of truth for what is happening. Do
  not assume direct communication between agents.
- **Integrate safely** — review external changes, respect ADRs and contracts, and
  run the relevant validations (tests, lints, build) before merging. Do not merge
  with unresolved conflicts, pending checks, or hidden failures: when a test fails,
  fix the real cause and keep it visible.
- **Act responsibly with autonomy** — continue independent tasks without asking for
  confirmation, and only request guidance on genuine conflicts, ambiguous
  ownership, or risk of losing work.

## Repository map

- `crates/` — workspace Rust: `core` (domínio, catálogo, estudo, SQLite), `server`
  (Axum + API HTTP), `desktop` (Tauri 2).
- `apps/web/` — SPA SvelteKit, membro do workspace pnpm.
- `docs/` — documentação canônica; navegue por [`docs/README.md`](docs/README.md).
- `.agents/` — políticas, playbooks, checklists e templates compartilhados.
- `.opencode/agents/` — perfis de revisão por tipo de agente.
- `deploy/`, `scripts/`, `testdata/` — adaptadores de deploy, automações e fixtures
  sintéticas.
