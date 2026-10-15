# Agent working agreement

Every agent must follow `AGENTS.md` and this index before working in the repository. This
rule applies to implementation, review, planning, documentation, and automation agents.
Agent-specific profiles may specialize the workflow but may not override the repository
policies, playbooks, or checklists.

This file is the short, mandatory index for agents. Read the relevant document under
`.agents/policies/`, `.agents/playbooks/`, or `.agents/checklists/` before changing
that area.

## Product and architecture

- LearnVault is a self-hosted study library. The approved target architecture is a Rust
  workspace with a shared `core`, an Axum server, a Tauri 2 desktop app, and SQLite. The
  current Go + SvelteKit implementation is the functional and visual reference and remains
  in place until the Rust MVP covers the approved requirements.
- The repository is a typed monorepo: Rust code in `crates/`, the legacy Go module as a
  self-contained unit in `services/go/` (use `go -C services/go ...`), and the SPA in
  `apps/web/` as a pnpm workspace member.
- Read `docs/architecture/adr/0001-reescrita-em-rust.md` before architectural work.
- Preserve the clean-room product policy: do not copy another product's code,
  assets, brand, proprietary text, or pixel-level layout.
- Keep Docker/Podman as optional deployment adapters; do not make them a runtime
  requirement for native packages.
- Read `docs/architecture/overview.md` and `docs/product/roadmap.md` before
  architectural or roadmap work.

## Data safety

- Follow `.agents/policies/data-safety.md`.

## Engineering workflow

- Follow `.agents/playbooks/testing.md` and the appropriate change playbook.

## GitHub governance

- Follow `.agents/policies/governance.md` and `.agents/checklists/pull-request.md`.
- For pull requests, replace every template prompt with factual content and record only
  validations that actually ran.

## Canonical documentation

- Read `docs/README.md` for navigation and `docs/governance/overview.md` for repository rules.
- Use GitHub Discussions for unresolved questions; use Issues for features and behavior changes.
- `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, the Wiki, and editorial documentation are in pt-BR. Only `LICENSE` stays in English.
- Conventional Commit prefixes stay in English while descriptions are written in pt-BR for Release Please.
