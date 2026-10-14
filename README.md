# LearnVault

Self-hosted course library for turning an existing folder of courses into a calm, searchable study space. LearnVault uses useful course-platform patterns as clean-room product references, without copying code, assets, branding, or pixel-level layouts.

The repository is private during the initial collaboration phase. It may become public later after the homelab and documentation have been reviewed.

The current foundation provides a portable Go API, an embedded SvelteKit SPA, local admin
authentication, SQLite migrations, optional container deployment, and an explicit
read-only catalog indexer.

## Project status

LearnVault is under active development. The current release line is the foundation/catalog phase. Player navigation, progress persistence, thumbnails, search, notes, favorites, and FFmpeg remuxing are planned follow-up milestones.

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
docs/                 Architecture, governance, roadmap, distribution, and releases
scripts/              Local verification helpers
testdata/             Synthetic fixtures; never real course data
.github/              CI, releases, ownership, and contribution forms
```

Root files are intentionally reserved for tool manifests (`go.mod`, `package.json`),
project policy, and files required by common tooling. Generated output is ignored.

## Build and tests

Requires Go 1.25+ and Node.js 24+ for local development. Docker is optional.

```sh
npm ci
npm run check
npm run build
go test ./...
go run ./cmd/learnvault
```

The SvelteKit source lives in `web/src`. Its generated output is written to
`cmd/learnvault/web/build` and embedded into the Go executable with `go:embed`.

The native executable is the foundation for future `.exe`, MSI, `.deb`, `.rpm`, and
Flatpak artifacts. See [docs/distribution.md](docs/distribution.md) for the release matrix.

## Optional Docker Compose adapter

Copy `deploy/compose/.env.example` to `deploy/compose/.env`, set a unique initial admin
password of at least 12 characters, and configure `DATA_DIR`, `CACHE_DIR` and
`LIBRARY_DIR`. The default library is a tiny fake fixture for local foundation testing,
not real courses.

```sh
docker compose --env-file deploy/compose/.env -f deploy/compose/compose.yaml up --build -d
```

`PORT` and `LISTEN_ADDR` are configurable. Port 8090 is only the suggested local deployment default. First startup requires `ADMIN_USERNAME` and `ADMIN_PASSWORD`; the first admin is created once and subsequent starts do not replace it. Use HTTPS when `COOKIE_SECURE=true` (the homelab reverse proxy will provide TLS).

The container runs as UID/GID 10001, with a read-only root filesystem, dropped capabilities, and `no-new-privileges`. Only `/data` and `/cache` are writable. On Linux, ensure the host data/cache directories are writable by UID 10001 (or configure a matching user in the image); `/library` is mounted read-only. To verify the mount protection with Docker Compose, run `scripts/verify-readonly.sh` from a POSIX shell after configuring `.env`; a failed write is the expected passing result. Never point the foundation test fixture at `/mnt/cursos`.

## Podman rootless / Quadlet

Build and tag the image as `localhost/learnvault:latest`, install `deploy/quadlet/learnvault.container` under `/home/media/.config/containers/systemd/`, create data/cache directories owned by `media`, and create `%h/stacks/learnvault/learnvault.env` with the initial admin credentials (restrict it to owner read/write). The unit uses port 8090 as its example deployment value; edit its `PORT`, `PublishPort` and health-check URL together as needed. Its library volume is explicitly `/mnt/cursos:/library:ro`.

The host's initial admin environment values are only used for first-time bootstrap and should be removed from the environment file after the database has been initialized. Back up the SQLite database consistently along with the required configuration and manually uploaded assets; destination, retention, and automation belong to the homelab.

## Phase 1 API

- `GET /api/healthz`: process/database health.
- `POST /api/login`: JSON `{ "username": "...", "password": "..." }`, sets an HttpOnly, SameSite=Strict session cookie.
- `POST /api/logout`: invalidates the current session.
- `GET /api/me`: current account; returns 401 when unauthenticated.

## Catalog API

- `GET /api/catalog`: returns the persisted category/course/module/lesson tree for an authenticated user.
- `POST /api/admin/catalog/rescan`: scans `LIBRARY_DIR` and replaces the persisted catalog. Admin authentication is required; scanning is explicit and is not performed automatically at startup.

The scanner reads directory entries and file metadata only. It ignores downloader temporary directories, treats common video extensions as media, groups subtitle files and other lesson files as materials, and only treats `.ts` files larger than 1 MiB as video to avoid confusing TypeScript source with MPEG-TS. The physical library remains outside the database and is never modified.

The initial admin is provisioned from `ADMIN_USERNAME` and `ADMIN_PASSWORD`; there is no public self-registration endpoint. `LIBRARY_DIR` must point to an existing directory. The application library abstraction only offers reads; container mount flags provide the OS-level read-only guarantee.

## Roadmap

The maintained product roadmap is in [ROADMAP.md](ROADMAP.md). GitHub Issues track the
individual tasks, Milestones group releases, and the private GitHub Project provides the
operational board. The Markdown roadmap remains the durable, reviewable summary.
The [GitHub Wiki](https://github.com/AndersonTavares0/LearnVault/wiki) is reserved for
navigable operational notes after its initial page is created.

## Not in this phase

Media delivery/player, FFmpeg, thumbnails, progress, search, notes, favorites, Caddy production change, and Whisper integration remain out of this phase. The existing course library is not altered.

## Contributing and governance

Read [CONTRIBUTING.md](CONTRIBUTING.md) before opening a pull request. Changes to `main`
must go through a pull request and pass the required GitHub Actions checks. See
[docs/architecture.md](docs/architecture.md) for design boundaries,
[docs/governance.md](docs/governance.md) for repository rules,
[ROADMAP.md](ROADMAP.md) for planned work, and [docs/releases.md](docs/releases.md) for
the release process.

Security issues should be reported privately as described in [SECURITY.md](SECURITY.md), not opened as public issues.

## License

LearnVault is released under the [Apache License 2.0](LICENSE).
