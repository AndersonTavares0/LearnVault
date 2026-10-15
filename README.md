# LearnVault

[![CI](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg)](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/AndersonTavares0/LearnVault?display_name=tag)](https://github.com/AndersonTavares0/LearnVault/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Go](https://img.shields.io/badge/Go-1.25-00ADD8?logo=go&logoColor=white)](https://go.dev/)
[![Node.js](https://img.shields.io/badge/Node.js-24.x-339933?logo=node.js&logoColor=white)](https://nodejs.org/)
[![pnpm](https://img.shields.io/badge/pnpm-11.28.5-F69220?logo=pnpm&logoColor=white)](https://pnpm.io/)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-2-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/docs/kit)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/)
[![Vite](https://img.shields.io/badge/Vite-7-646CFF?logo=vite&logoColor=white)](https://vite.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![SQLite](https://img.shields.io/badge/SQLite-modernc.org-003B57?logo=sqlite&logoColor=white)](https://sqlite.org/)

LearnVault transforms an existing course folder into a self-hosted study space. It is
designed around a read-only physical library, individual accounts, and a calm local
study workflow. Course-platform products are clean-room product references only; this
project does not copy their code, assets, branding, text, or pixel-level layouts.

The repository is private during the initial collaboration phase. It may become public later after the homelab and documentation have been reviewed.

The current foundation provides a portable Go API, an embedded SvelteKit SPA, local admin
authentication, SQLite migrations, optional container deployment, and an explicit
read-only catalog indexer.

## Status

LearnVault is under active development. The current release line covers the Go service,
local authentication, SQLite migrations, catalog indexing, and the embedded SPA.
Player navigation, progress persistence, thumbnails, search, notes, favorites, and
FFmpeg remuxing are planned follow-up milestones.

The physical course library is never part of this repository. Do not commit course files, personal data, homelab credentials, or generated media.

## Principles

- Keep the source library read-only.
- Prefer one maintainable service over unnecessary infrastructure.
- Preserve user data across rescans whenever identity can be matched.
- Ship small, reviewable changes through pull requests.
- Keep product research clean-room: learn concepts, never copy proprietary implementation or identity.

## Repository map

```text
cmd/learnvault/       Go executable and embedded SPA output
internal/             Private backend packages and SQLite migrations
web/src/              SvelteKit source
deploy/container/     OCI image definition
deploy/compose/       Local Docker Compose stack and env example
deploy/quadlet/       Rootless Podman systemd unit
docs/                 Versioned architecture, product, governance, operations, and release docs
scripts/              Local verification helpers
testdata/             Synthetic fixtures; never real course data
.github/              CI, releases, ownership, and contribution forms
.agents/              Agent policies, playbooks, checklists, and templates
```

Root files are intentionally reserved for tool manifests (`go.mod`, `package.json`),
project policy, and files required by common tooling. Generated output is ignored.

## Stack

Use the same toolchain across development, CI, container builds, and releases:

| Area | Standard |
| --- | --- |
| Backend | Go 1.25; CI and release builds pin Go 1.25.14 |
| Frontend runtime | Node.js 24.x with pnpm 11.28.5 |
| Frontend | SvelteKit 2, Svelte 5, Vite 7, TypeScript 5 |
| Dependency installation | `pnpm-lock.yaml` is authoritative; use `pnpm install --frozen-lockfile` in CI |
| Persistence | SQLite through `modernc.org/sqlite` (pure Go) |
| Application delivery | One Go executable with the static SPA embedded |
| Optional deployment | Docker Compose or rootless Podman/Quadlet; neither is required to develop or run the native executable |

Dependency versions are locked in `pnpm-lock.yaml` and `go.sum`. Keep the major
toolchain versions aligned with this table and propose upgrades through a reviewed PR.
`pnpm` is the only JavaScript package manager; do not add a competing lockfile.

## Development

Requires Go 1.25, Node.js 24.x, and pnpm 11.28.5 for local development. Docker is optional.

```sh
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
go test ./...
go run ./cmd/learnvault
```

The SvelteKit source lives in `web/src`. Its generated output is written to
`cmd/learnvault/web/build` and embedded into the Go executable with `go:embed`.

The native executable is the foundation for future `.exe`, MSI, `.deb`, `.rpm`, and
Flatpak artifacts. See [distribution](docs/operations/distribution.md) for the release matrix.

## Deployment and API

Docker Compose and rootless Podman/Quadlet are optional adapters. Their complete
configuration, mounts, bootstrap rules, and read-only verification are documented in
[`deploy/compose/`](deploy/compose/) and [`docs/operations/distribution.md`](docs/operations/distribution.md).

The endpoint summary is in [`docs/reference/api.md`](docs/reference/api.md). The API
requires local admin bootstrap, keeps the external library read-only, and performs
catalog indexing only through an explicit admin action.

## Roadmap

The maintained product roadmap is in [docs/product/roadmap.md](docs/product/roadmap.md). GitHub Issues
track individual tasks and Milestones may group releases. The Markdown roadmap remains the durable,
reviewable summary; any project board is an optional planning surface, not the source of truth.
The [GitHub Wiki](https://github.com/AndersonTavares0/LearnVault/wiki) is generated from
the versioned documentation after changes merge into `main`; do not edit managed pages in the
Wiki. The repository remains the canonical source for code and reviewed policy.

## Not in this phase

Media delivery/player, FFmpeg, thumbnails, progress, search, notes, favorites, Caddy production change, and Whisper integration remain out of this phase. The existing course library is not altered.

## Contributing and governance

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. Changes to `main`
must go through a pull request and pass the required GitHub Actions checks. See
[architecture](docs/architecture/overview.md) for design boundaries,
[governance](docs/governance/overview.md) for repository rules,
[roadmap](docs/product/roadmap.md) for planned work, and [release process](docs/releases/process.md) for
the release process.

Security issues should be reported privately as described in [SECURITY.md](SECURITY.md), not opened as public issues.

## License

LearnVault is released under the [Apache License 2.0](LICENSE).
