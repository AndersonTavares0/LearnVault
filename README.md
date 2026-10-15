# LearnVault

[![CI](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg)](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/AndersonTavares0/LearnVault?display_name=tag&sort=semver)](https://github.com/AndersonTavares0/LearnVault/releases)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Go](https://img.shields.io/badge/Go-1.26-00ADD8?logo=go&logoColor=white)](https://go.dev/)
[![Node.js](https://img.shields.io/badge/Node.js-24.x-339933?logo=node.js&logoColor=white)](https://nodejs.org/)
[![pnpm](https://img.shields.io/badge/pnpm-11.28.5-F69220?logo=pnpm&logoColor=white)](https://pnpm.io/)
[![SvelteKit](https://img.shields.io/badge/SvelteKit-2-FF3E00?logo=svelte&logoColor=white)](https://svelte.dev/docs/kit)

LearnVault turns an existing course folder into a self-hosted study space. It combines
a Go service, SQLite, and a SvelteKit SPA around a read-only library, local accounts,
catalog indexing, and a focused study workflow.

## Status

The foundation is in active development. Authentication, SQLite migrations, catalog
indexing, the embedded SPA, and optional container adapters are available. Player
navigation, progress, search, notes, favorites, thumbnails, and media processing are
planned next.

## Start here

Requirements: Go 1.26, Node.js 24.x, and pnpm 11.28.5. Docker is optional.

```sh
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
go test ./...
go run ./cmd/learnvault
```

The SPA is built from `web/src`, written to `cmd/learnvault/web/build`, and embedded
into the Go executable. The first run requires `ADMIN_USERNAME` and
`ADMIN_PASSWORD` with at least 12 characters. See `.env.example` for local settings.

## Repository map

```text
cmd/learnvault/   Go executable and embedded SPA output
internal/         Backend packages, catalog, library, HTTP API, and SQLite
web/src/           SvelteKit source
deploy/            Container, Compose, and rootless Quadlet adapters
docs/              Architecture, API, governance, operations, product, and releases
.agents/           Policies, playbooks, checklists, and templates
.github/           Actions, Release Please, Dependabot, and contribution forms
scripts/           Local verification and release helpers
testdata/          Synthetic fixtures used by tests
```

Root files are limited to manifests, tool configuration, project policy, and common
entrypoints required by Go, pnpm, SvelteKit, GitHub, or the documentation toolchain.
Generated output and runtime data are ignored.

## Architecture

- One Go executable with the static SPA embedded.
- SQLite through `modernc.org/sqlite`.
- The physical library is read-only and remains outside the application data store.
- Docker Compose and Podman/Quadlet are optional deployment adapters.
- Native executable builds remain the source of truth for future installers.

See the [architecture overview](docs/architecture/overview.md) and the
[API reference](docs/reference/api.md).

## Deployment

For local containers, use the Compose adapter:

```sh
docker compose --env-file deploy/compose/.env.example \
  -f deploy/compose/compose.yaml up --build
```

Container and Quadlet details are documented under [`deploy/`](deploy/) and in the
[distribution guide](docs/operations/distribution.md).

## Releases

Release Please owns version calculation, release PRs, `CHANGELOG.md`, tags, and GitHub
Release notes. Use Conventional Commits such as `feat:`, `fix:`, `docs:`, and `chore:`.
Published releases receive versioned Linux binaries and `SHA256SUMS` automatically.

Read the [release process](docs/releases/process.md) before changing release files.

## Documentation and contribution

The [documentation guide](docs/README.md) is the index for canonical project docs.
The repository is the source of truth; the managed Wiki is a derived reading surface.

Before contributing, read [CONTRIBUTING.md](CONTRIBUTING.md) and the
[governance overview](docs/governance/overview.md). Changes to `main` go through a
pull request and the required checks.

Security issues should be reported privately as described in [SECURITY.md](SECURITY.md).

## License

LearnVault is released under the [Apache License 2.0](LICENSE).
