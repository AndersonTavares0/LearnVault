# Repository Governance

**Target audience:** anyone opening an issue, pull request, release, or modifying CI.
If you just want to run the project, start with the [README](../../README.md).

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

Direct pushes, force pushes, and branch deletions on `main` are not allowed. GitHub enforces this
with [Repository Rulesets](rulesets.md); this document records the policy.

Forks are not part of the maintainer flow. Contributors with write access use topic branches in this
repository. `.github/CODEOWNERS` names the owner. Every open issue needs an assigned owner.

## Release governance

The [release and versioning policy](../../.agents/policies/releases.md) covers version numbers (the line only
moves forward), pacing (monthly, with a short list of off-cycle reasons), pre-release retention, the changelog and
release notes format, approvals, and rollback. [ADR 0006](../architecture/adr/0006-release-versioning-and-prerelease-retention.md)
records the reasoning. The mechanics are in [`docs/releases/process.md`](../releases/process.md).

## Agent review policy

The canonical [agent collaboration policy](../../.agents/policies/agent-collaboration.md)
defines delegation ownership, risk-based review, and revision-specific evidence. It adds to the required
CI and review workflow and does not change GitHub protection settings.

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

The repository root holds project configuration, tool settings, and entry points only. Code and content
live in these directories:

| Path | Responsibility |
| --- | --- |
| `crates/` | Rust workspace. `crates/core` contains the shared domain (SQLite, catalog, scanner, media identity, and study state); `crates/server` contains the Axum HTTP server; `crates/desktop` contains Tauri 2 desktop app |
| `apps/web-next/` | React 19 + TypeScript + Vite SPA (`src/`, `vite.config.ts`, `tsconfig.json`, `package.json`) |
| `testdata/` | Small synthetic test fixtures |
| `deploy/container/` | Containerfile / OCI image definition |
| `deploy/compose/` | Docker Compose stack and environment examples |
| `deploy/quadlet/` | Systemd Quadlet unit for rootless Podman |
| `scripts/` | Local verification and CI automation scripts |
| `tests/` | Playwright end-to-end specs and fixtures (`playwright.config.ts` is at the root) |
| `docs/` | Documentation: architecture, design, governance, operations, product, reference, and releases |
| `.github/` | Workflows, Release Please configuration, rulesets, and community health files |
| `.cargo/` | `audit.toml`, the accepted Rust advisories |
| `.agents/` | Shared policies, playbooks, checklists, and templates for autonomous and human agents |
| `.opencode/` | OpenCode-specific agent review profiles |

Community health files (`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`) live in `.github/`; the
root `CONTRIBUTING.md` only points there. The changelog is `docs/releases/CHANGELOG.md`, and Release Please
configuration is in `.github/release-please/`.

Generated output (`crates/server/web/build`, `crates/desktop/frontend-dist`, `target`, `node_modules`, and
local data and cache directories) is gitignored and is never committed.

## Repository Boundaries

- Never commit course media, secrets, private addresses, SQLite database files, or build artifacts.
- Changes to security, persistence, deployment, database migrations, or UX architecture need explicit review.
- Other products are a structural reference only. Never copy their code, assets, trademarks, copy, or UI layouts.
- Docker and Podman are optional deployment adapters, and all functionality must stay testable without them.

## Documentation Standards

See [the documentation index](../README.md).

`README.md`, `AGENTS.md`, everything under `docs/`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, and
`SECURITY.md` are in English. The app itself switches between English (default) and Brazilian Portuguese
(`pt-BR`). Conventional Commits and PR titles and descriptions are in English so Release Please can build the
changelog.

## Releases & CI

Release Please computes versions, changelog entries, tags, and GitHub Releases, and a dedicated release workflow
publishes the artifacts. Before 1.0.0, `feat`, `fix`, and `perf` commits raise the patch version and breaking
changes raise the minor (`bump-minor-pre-major` and `bump-patch-for-minor-pre-major` in
`.github/release-please/release-please-config.json`).

LearnVault code and documentation use the MIT license ([`LICENSE`](../../LICENSE)). Cargo workspace metadata, npm
package metadata, and desktop metainfo must match it.

### Dependabot Safe Auto-Merge

The `Dependabot safe auto-merge` workflow merges low-risk dependency updates without a manual merge on each PR.
It runs in the base repository context through `pull_request_target` and never checks out or runs PR code.

It handles PRs from `dependabot[bot]` to `main` in this repository, for a single patch or minor update, when
the security alert state is explicitly `null` and the head SHA is verified. The alert gate fails closed: a
missing, empty, or unknown alert state disqualifies the PR. Major updates, security updates, bundled PRs, and
conflicting branches need human review.

Eligible PRs are queued for squash auto-merge through the GitHub API, and branch protection and checks still
apply.

### Automated Checks

Required check names are stable: `SPA checks`, `Rust tests and lints`, `Windows core tests`,
`Desktop crate compile and guards`, `Container build (Rust)`, `Dependency review`, and `CI gate`.

CI runs on pull requests, cancels superseded runs, and builds separate server and desktop SPA variants. For
documentation-only or release-metadata changes, browser E2E, Rust, Windows core, desktop, container, and Rust
dependency audit jobs skip. The `CI gate` fails on unexpected skips, cancellations, or test failures.

- `Rust tests and lints`: formatting, clippy, `cargo nextest` for the workspace except the desktop crate, doc tests,
  and an 80% line-coverage floor for `crates/core`.
- `Windows core tests`: formatting, clippy, and tests for `crates/core` on Windows.
- `Desktop crate compile and guards`: compiles `crates/desktop` and runs its tests.
- `Container build (Rust)`: builds and validates the OCI image and scans it with Trivy for HIGH and CRITICAL findings.
- `Rust dependency audit`: `cargo audit --deny unsound` on `Cargo.lock`. `.cargo/audit.toml` accepts two advisories
  with a written rationale and a removal condition: `RUSTSEC-2024-0429` (`glib`) and `RUSTSEC-2023-0071` (`rsa`).
  This job feeds the `CI gate` and is not itself a required check.

Promoting any check to mandatory requires an explicit maintainer decision.

## Automatic Labeling

`.github/labeler.yml` and the `Labeler` workflow classify PRs by touched paths:
`area:web`, `area:rust`, `area:desktop`, `area:docs`, `area:ci`, `area:deploy`, as well as `tests` and `dependencies`.

Labels are informational and never block a merge. Work type (`bug`, `enhancement`, `documentation`) stays a human decision.

## Release identity and recovery

The workflow resolves the release Git tag to a commit and builds from that commit. It pushes platform images by digest for scanning and smoke tests, then rechecks the Git tag before staging the combined image index. It checks the Git tag again immediately before uploading assets. Manual rebuilds use the tagged source and leave the `latest` image alias in place.

Database backup and restore stage SQLite files and sidecars in a private directory on the destination filesystem. Exports keep private Unix file permissions, rollback copies fail if the filename exists, and replacing the destination never deletes a valid database first. Pick a destination with trusted ancestors or a protected shared directory. On Windows, confidentiality depends on the directory's inherited ACLs.

Container restore commands override the image entrypoint so `restore` is parsed as the CLI subcommand. Restore and rollback require the service to be stopped.

## Related Documentation

- [Architecture](../architecture/overview.md)
- [Requirements](../product/requirements.md)
- [Distribution](../operations/distribution.md)
