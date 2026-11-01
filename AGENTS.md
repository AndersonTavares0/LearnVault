# LearnVault agent instructions

LearnVault organiza pastas de cursos que o usuário já possui em um espaço de estudo
auto-hospedado, sem mover nem alterar os arquivos originais. O produto é pequeno,
sustentável e autocontido: servidor Rust (`crates/server`, Axum, SQLite), SPA Svelte
(`apps/web`), núcleo de domínio compartilhado (`crates/core`) e aplicativo desktop
offline Tauri 2 (`crates/desktop`). Docker/Podman são adaptadores opcionais de deploy,
não requisitos de runtime.

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
pnpm dev                         # API Rust em http://127.0.0.1:8090 + SPA com recarga
pnpm dev:tauri                   # aplicativo desktop (Tauri) durante o desenvolvimento
pnpm verify                      # checks de PR (SPA + Rust + desktop); --fast pula builds e audit
pnpm run check                   # lints/types da SPA
pnpm run test:scripts            # node --test scripts/*.test.mjs (roda na CI em todo PR)
pnpm run build / build:desktop   # builds da SPA e do bundle desktop
pnpm audit --audit-level=low     # auditoria de dependências
cargo fmt --all -- --check       # formatação Rust
cargo clippy --all-targets -- -D warnings
cargo test --all
```

A versão do Rust é garantida por `rust-toolchain.toml`; Node.js 24 e pnpm 11.28.5 vêm de
`packageManager` em `package.json`. Docker é opcional.

## Conventions

- Documentação, Wiki e textos editoriais em pt-BR; Conventional Commits com prefixo em
  inglês e descrição em pt-BR (ex.: `docs: ajustar README`).
- **Uso obrigatório do Ponytail:** em todas as tarefas de desenvolvimento do LearnVault,
  utilize sempre o Ponytail para escrever, modificar e refatorar código. Não utilize outros
  métodos de edição quando o Ponytail estiver disponível. Mantenha o código limpo, consistente
  com a arquitetura existente e focado em edições cirúrgicas, sem refatorações desnecessárias.
- A biblioteca de cursos é somente leitura: nenhuma mudança pode escrever, mover ou
  copiar os arquivos de origem.
- Nunca inclua segredos, dados de cursos, bancos de dados ou mídia gerada em commits,
  logs ou prompts. Testes usam diretórios temporários e fixtures sintéticas.

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
