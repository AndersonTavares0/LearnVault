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
- Always use Ponytail to write, modify, and refactor code across all development tasks when available, keeping changes surgical, clean, and consistent with existing architecture.
- Ensure all user-facing strings (UI, modals, alerts, error fallbacks, and transport errors) use `tr(en, ptBr)` from `apps/web/src/i18n.svelte.ts` (the React app, `apps/web-next`, has the same function in `src/lib/i18n.ts`).

## GitHub governance

- For delegation and review of security, persistence, or release/update changes, follow
  `.agents/policies/agent-collaboration.md`.

- Follow `.agents/policies/governance.md` and `.agents/checklists/pull-request.md`.
- For pull requests, replace every template prompt with factual content and record only
  validations that actually ran.

### GitHub Discussions

- Consult relevant Discussions when tasks involve architectural decisions, product changes,
  or previously debated functionality; prioritize discussions directly related to the active scope.
- Treat Discussions as background context, not approved specifications. Record accepted decisions
  in the corresponding ADR, roadmap, or GitHub Issue.
- Avoid duplicate issues or implementing unapproved proposals.
- Do not close or alter Discussions without explicit maintainer authorization.

## Canonical documentation

- Read `docs/README.md` for navigation and `docs/governance/overview.md` for repository rules.
- Use GitHub Discussions for unresolved questions; use Issues for features and behavior changes.
- `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, and documentation are in English. The application provides built-in internationalization (i18n) supporting English as default and Brazilian Portuguese (`pt-BR`).
- Conventional Commits and descriptions are written in English for Release Please.
- Release Please follows Conventional Commit version bumps before `1.0.0`: `feat:`, `fix:`, and
  `perf:` increment patch, while breaking changes increment minor. From `1.0.0`, breaking changes
  increment major. Release tags and names use `vMAJOR.MINOR.PATCH`.
- Release Please prepares one release candidate on the first day of each month (UTC). Merge its
  release PR to publish; urgently needed security releases can prepare a candidate early by
  manually running the `Release Please` workflow on `main`.
- GitHub Wiki is permanently removed; do not recreate wiki sync scripts or workflows.
