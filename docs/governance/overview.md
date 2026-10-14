# Repository governance

## Required workflow

1. Open an issue using the Bug Report or Feature Request form before non-trivial work.
2. Create a focused branch from `main`.
3. Open a pull request using the repository template.
4. Pass the required CI checks and obtain the required review.
5. Resolve all conversations and squash-merge through GitHub.

Direct pushes, force-pushes, and branch deletion on `main` are not allowed. Branch
protection is configured on GitHub and this document records the project rule.

The collaboration path is `Issue → branch → pull request → review → squash merge`.
Private forks are unavailable for this user-owned private repository, so collaborators
with Write access work on branches in the repository. The owner remains CODEOWNER and
controls approval into `main`.

## Template policy

- Blank issues are disabled.
- Bugs use `.github/ISSUE_TEMPLATE/bug.yml`.
- Features use `.github/ISSUE_TEMPLATE/feature.yml`.
- Security reports use the private security policy, never a public issue.
- Pull requests use `.github/pull_request_template.md`.

## Source organization

Keep the root limited to tool configuration, project policy, and deployment entrypoints.
Application code belongs in these directories:

| Path | Responsibility |
| --- | --- |
| `cmd/learnvault/` | Go executable, tests, and embedded SPA build output |
| `internal/` | Private backend packages and migrations |
| `web/src/` | SvelteKit source |
| `testdata/` | Small synthetic fixtures only |
| `deploy/container/` | OCI image definition |
| `deploy/compose/` | Local Docker Compose stack and environment example |
| `deploy/quadlet/` | Rootless Podman systemd unit |
| `scripts/` | Local verification scripts |
| `docs/` | Versioned architecture, governance, operations, product, release, and Wiki documentation |

Generated output (`cmd/learnvault/web/build`, `.svelte-kit`, `node_modules`, local
data/cache) must stay ignored and must not be committed.

Documentation is grouped by responsibility: `architecture/`, `governance/`,
`operations/`, `product/`, `releases/`, and `wiki/`. The repository is canonical;
only the pages declared in `wiki/mapping.md` are mirrored to GitHub Wiki.

## Repository boundaries

- Never commit course media, credentials, private homelab details, databases, or generated output.
- `/mnt/cursos` is external read-only data and must not be accessed by automated tests.
- Changes affecting security, storage, deployment, migrations, or UX architecture require explicit review.
- Clean-room research may inform product behavior, but not copied code, assets, branding, text, or layouts.
- Docker and Podman are optional deployment adapters; application behavior must remain
  testable and buildable without a container runtime.

## Ownership and releases

`.github/CODEOWNERS` defines review ownership. Releases are tag-driven through the release
workflow, with `CHANGELOG.md` and `docs/releases/process.md` updated as part of the change.

## Wiki synchronization

`Wiki sync` runs only after mapped documentation reaches `main`, or by an explicit
manual dispatch. It has independent path filters and is not a required CI check. The
workflow writes only managed pages in the Wiki Git repository, commits only a real diff,
and does not react to Wiki (`gollum`) events, preventing sync loops. Manual edits to a
managed page are intentionally replaced by the next canonical documentation change.

## CI efficiency

The three required CI job names remain stable: `SPA checks`, `Go tests and vet`, and
`Container build`. CI always runs for pull requests so a required check is never skipped.
It cancels obsolete runs for the same branch, builds the SPA once for the Go embed step,
uses dependency/build caches, applies short timeouts, and retains the generated SPA
artifact for one day. Container build remains an independent deployment smoke test with
its own BuildKit cache. The non-required Wiki workflow uses path filters because it only
updates derived documentation.
