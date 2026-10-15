# Agent working agreement

This file is the short, mandatory index for agents. Read the relevant document under
`.agents/policies/`, `.agents/playbooks/`, or `.agents/checklists/` before changing
that area.

## Product and architecture

- LearnVault is a self-hosted study library: Go service, SQLite, and SvelteKit SPA.
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

## Canonical documentation

- Read `docs/README.md` for navigation and `docs/governance/overview.md` for repository rules.
- Use GitHub Discussions for unresolved questions; use Issues for features and behavior changes.
- `README.md` and `CHANGELOG.md` remain in English. Wiki and editorial documentation are in pt-BR.
- Conventional Commit prefixes and descriptions remain in English for Release Please.
