# Documentation Guide

The repository is the canonical source for all reviewed project documentation. Start here when looking for architecture decisions, operational guides, or governance rules.

| Section | Purpose |
| --- | --- |
| [`architecture/`](architecture/) | System boundaries, data flow, IPC contracts, and ADRs |
| [`design/`](design/) | Design system, visual tokens, and brand identity |
| [`governance/`](governance/) | Repository policies, ownership, code conventions, and CI gates |
| [`operations/`](operations/) | Deployment, container adapters, and release distribution targets |
| [`product/`](product/) | Product roadmap, requirements, and design philosophies |
| [`releases/`](releases/) | Versioning schemes, changelog, and release procedures |
| [`reference/`](reference/) | Stable API specifications and schemas |

## Contribution & Language Standards

- Read [`CONTRIBUTING.md`](../.github/CONTRIBUTING.md) and the [Governance Guide](governance/overview.md) for contribution workflows.
- `README.md`, `docs/releases/CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, code comments, PRs, and editorial documentation are maintained in English (`en`).
- The user-facing application features built-in internationalization (i18n) supporting both English (`en`, default) and Brazilian Portuguese (`pt-BR`).
