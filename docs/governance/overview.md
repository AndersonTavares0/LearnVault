# Repository Governance

**Target audience:** anyone opening an issue, pull request, release, or modifying CI.
If you just want to run the project, start with the [README](../../README.md).

**Summary:** LearnVault follows GitHub Flow. Feature changes start in an issue or Discussion,
all code enters via topic branches and pull requests, and `main` receives squash merges only
after all checks and reviews pass.

## Contribution Workflow

1. Open a Discussion when design questions or alternative solutions need consensus.
2. Open an issue for a feature, bug report, or behavioral change.
3. For typos, routine dependency bumps, or simple CI fixes, an issue is optional.
4. Create a focused topic branch from `main`.
5. Open a pull request using the repository template.
6. Pass all required automated checks and obtain needed code review.
7. Resolve all review discussions and squash-merge (single commit) via GitHub.

```text
Discussion (if needed) → Issue (when needed) → Branch → PR → Review → Squash Merge
```

Direct pushes, force pushes, and branch deletions on `main` are strictly disallowed.
GitHub enforces branch protection via [Repository Rulesets](rulesets.md); this document records the project policy.

Forks are not part of the standard maintainer flow. Contributors with write access work on topic
branches within this repository. `.github/CODEOWNERS` indicates subsystem ownership. Every open
issue must have an assigned owner.

## Agent review policy

The canonical [agent collaboration policy](../../.agents/policies/agent-collaboration.md)
defines delegation ownership, review according to risk, and revision-specific evidence.
It complements the required CI and human/agent review workflow; it does not update GitHub
protection settings.

## Issue Templates

| Scenario | Location |
| --- | --- |
| Bug | `.github/ISSUE_TEMPLATE/bug.yml` |
| Feature | `.github/ISSUE_TEMPLATE/feature.yml` |
| Documentation or typo | `.github/ISSUE_TEMPLATE/documentation.yml` |
| Dependency or CI | `.github/ISSUE_TEMPLATE/maintenance.yml` |
| Planning | `.github/ISSUE_TEMPLATE/planning.yml`, with label `planning` |
| Security vulnerability | Private security advisory report, never a public issue |
| Open question or design debate | GitHub Discussions |
| Pull request | `.github/pull_request_template.md` |

Blank issues are disabled.

## Code Organization

Keep the root directory limited to project configurations, tool settings, and deployment entry points.
Application code is organized in these directories:

| Path | Responsibility |
| --- | --- |
| `crates/` | Rust workspace. `crates/core` contains the shared domain (SQLite, catalog, scanner, media identity, and study state); `crates/server` contains the Axum HTTP server; `crates/desktop` contains Tauri 2 desktop app |
| `apps/web-next/` | React 19 + TypeScript + Vite SPA (`src/`, `vite.config.ts`, `tsconfig.json`, `package.json`) |
| `testdata/` | Small synthetic test fixtures |
| `deploy/container/` | Containerfile / OCI image definition |
| `deploy/compose/` | Docker Compose stack and environment examples |
| `deploy/quadlet/` | Systemd Quadlet unit for rootless Podman |
| `scripts/` | Local verification and CI automation scripts |
| `docs/` | Canonical documentation: architecture, governance, operations, product, and releases |
| `.github/` | Workflows, Release Please configuration, and community health files |
| `.agents/` | Shared policies, playbooks, checklists, and templates for autonomous and human agents |
| `.opencode/` | OpenCode-specific agent review profiles |

Community health files (`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`) live in `.github/`.
The changelog lives in `docs/releases/CHANGELOG.md`. Release Please configuration lives in
`.github/release-please/`.

The repository root contains only `AGENTS.md`, `README.md`, `LICENSE`, `.dockerignore`, `.gitignore`,
`Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `package.json`, `pnpm-lock.yaml`, and `pnpm-workspace.yaml`.

Generated output (`crates/server/web/build`, `crates/desktop/frontend-dist`, `target`, `node_modules`,
and local data/cache directories) is strictly gitignored and must never be committed.

## Repository Boundaries

- Never commit course media, secrets, private addresses, SQLite database files, or build artifacts.
- Changes to security, storage persistence, deployment, database migrations, or UX architecture require explicit review.
- Research into other products may guide behavior as a structural reference only; their code, assets, trademarks, copy, or UI layouts must never be copied.
- Docker and Podman are optional deployment adapters. All functionality must remain fully testable without container runtimes.

## Documentation Standards

The repository is the single source of truth. The index in [`docs/README.md`](../README.md) describes the structure.

`README.md`, `AGENTS.md`, canonical docs (`docs/`), `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`,
and `SECURITY.md` are maintained in English. The application provides full runtime internationalization (i18n)
with English as the default and Brazilian Portuguese (`pt-BR`) fully supported.
Conventional Commits and PR titles/descriptions are written in English to keep Release Please changelog generation consistent.

## Releases & CI

Release Please automates semantic versioning, changelog entries, tags, and GitHub Releases. Artifact publication
runs in a dedicated release workflow.

LearnVault code and documentation are distributed under the MIT license ([`LICENSE`](../../LICENSE)).
Cargo workspace metadata, npm package metadata, and desktop metainfo must remain aligned with this license.

### Dependabot Safe Auto-Merge

The `Dependabot safe auto-merge` workflow enables Dependabot to update low-risk dependencies without
requiring manual merges on every routine PR. It executes in the base repository context via `pull_request_target`,
without checking out or running untrusted PR code.

It processes PRs authored by `dependabot[bot]` targeting `main`, within the same repository, for single
patch/minor updates where security alerts are explicitly `null` and the head SHA is verified.

The alert gate fails closed. If alert status is missing, empty, or unknown, the PR is disqualified.
Major updates, security updates, bundled PRs, and conflicting branches require human review.

Eligible PRs are submitted for squash auto-merge via the GitHub API without bypassing branch protection or checks.

### Automated Checks

Mandatory check names are stable: `SPA checks`, `Rust tests and lints`, `Desktop crate compile and guards`,
`Container build (Rust)`, and `CI gate`.

CI runs on pull requests, cancels superseded runs, builds the SPA once for the Rust container and desktop app,
uses caching, and enforces short timeouts. For documentation-only or release metadata changes, heavy jobs
(E2E, Rust, desktop, container, dependency audit) skip gracefully. The `CI gate` fails on unexpected skips,
cancellations, or test failures.

- `Rust tests and lints`: checks formatting, clippy lints, unit tests, and coverage in Rust crates.
- `Desktop crate compile and guards`: compiles `crates/desktop` and runs desktop-specific tests.
- `Container build (Rust)`: builds and validates the OCI container image.
- `Rust dependency audit`: audits `Cargo.lock` with `cargo audit --deny unsound`. There is one narrow,
  documented exception in `.cargo/audit.toml` (`RUSTSEC-2024-0429`, `glib`).

Promoting any check to mandatory requires an explicit maintainer decision.

## Automatic Labeling

`.github/labeler.yml` and the `Labeler` workflow classify PRs by touched paths:
`area:web`, `area:rust`, `area:desktop`, `area:docs`, `area:ci`, `area:deploy`, as well as `tests` and `dependencies`.

Labels are informational and never block merges. Work type (`bug`, `enhancement`, `documentation`) remains a human decision.

## Related Documentation

- [Architecture](../architecture/overview.md) for core, server, and desktop boundaries.
- [Requirements](../product/requirements.md) for functional and non-functional requirements.
- [Distribution](../operations/distribution.md) for build artifacts and deployment.

## Release identity and recovery

Stable builds resolve the requested `refs/tags/vX.Y.Z` to a commit SHA. The container image is built locally, then the workflow refreshes and checks the tag before pushing the image. Asset publication repeats the tag check immediately before upload. Manual rebuilds therefore use the release's tagged source rather than the dispatch branch, and manual repair does not update the mutable `latest` tag. Keep release tags unchanged during publication.

Database backup and restore stage SQLite files and sidecars in a private directory on the destination filesystem. Exports preserve private Unix file permissions, rollback copies fail on an existing filename, and destination replacement does not delete a valid database first. Choose a destination with trusted ancestors or a protected shared directory; Windows confidentiality depends on the directory's inherited ACLs.

Container restore commands override the image entrypoint so `restore` is parsed as the CLI subcommand. Restore and rollback still require the service to be stopped.
