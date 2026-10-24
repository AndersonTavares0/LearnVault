# Agent working agreement

Every agent must follow `AGENTS.md` and this index before working in the repository. This
rule applies to implementation, review, planning, documentation, and automation agents.
Agent-specific profiles may specialize the workflow but may not override the repository
policies, playbooks, or checklists.

This file is the short, mandatory index for agents. Read the relevant document under
`.agents/policies/`, `.agents/playbooks/`, or `.agents/checklists/` before changing
that area.

## Product and architecture

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

### GitHub Discussions

- Consulte Discussions relevantes quando a tarefa envolver decisões arquiteturais, mudanças
  de produto ou funcionalidades previamente debatidas; priorize as diretamente relacionadas
  ao escopo atual.
- Trate Discussions como contexto, não como requisitos aprovados. Registre decisões
  consolidadas no ADR, roadmap ou Issue correspondente.
- Evite duplicar Issues ou implementar propostas sem confirmar que foram aprovadas.
- Não encerre nem modifique Discussions sem autorização do mantenedor.

## Canonical documentation

- Read `docs/README.md` for navigation and `docs/governance/overview.md` for repository rules.
- Use GitHub Discussions for unresolved questions; use Issues for features and behavior changes.
- `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, the Wiki, and editorial documentation are in pt-BR. Only `LICENSE` stays in English.
- Conventional Commit prefixes stay in English while descriptions are written in pt-BR for Release Please.
