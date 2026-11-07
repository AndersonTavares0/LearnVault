# Distribution & Operations

**Target audience:** operators publishing, installing, or deploying LearnVault.
If you are developing or studying the architecture, start with the [README](../../README.md) and [Architecture Overview](../architecture/overview.md).

**Summary:** LearnVault publishes a Windows desktop installer (NSIS), Linux server tarballs, and an optional OCI container image. Flatpak Linux desktop packaging is currently in development. Docker and Podman are optional deployment adapters, not runtime requirements.

## Current Release Targets

| Target | Artifact | Purpose | Status |
| --- | --- | --- | --- |
| Linux Server | Rust Linux tarball (`learnvault-linux-<arch>.tar.gz`) | Primary supported Linux server deployment | Available |
| Self-hosted | OCI container image via Compose (Docker/Podman) | Containerized server deployment | Supported; local or published image |
| Windows Desktop | NSIS installer (`.exe`) | Local desktop usage; per-user installation without admin rights | Automated, verified with SHA256SUMS |
| Linux Desktop | Flatpak | Desktop application for Linux | `xdg-portal` configured; sandbox smoke testing in progress |

## Choosing a Distribution Target

- **Windows Desktop:** use the NSIS installer. It installs per-user in `%LOCALAPPDATA%\LearnVault` without requiring administrator privileges.
- **Linux Server:** use the `learnvault-linux-<arch>.tar.gz` tarball.
- **Container Deployments:** use Docker Compose or systemd Quadlet.
- **Linux Desktop:** Flatpak packaging is in progress.

### Update Notifications

When enabled in configuration, the server queries the public GitHub Releases API over HTTPS and compares the latest stable tag with the local running version using strict SemVer. The check times out after 5 seconds, consumes at most 64 KiB, and caches results in-process for 1 hour. No binaries or files are ever downloaded or installed automatically. Local builds (`0.0.0`) skip remote checks.

### Linux Server Tarball Usage

The `learnvault-linux-<arch>.tar.gz` tarball contains the `learnvault-linux-<arch>` binary, `LICENSE`, and canonical SPA files in `web/` with `web/200.html`. Extract and point `--web-root` to the web directory:

```sh
tar -xzf learnvault-linux-amd64.tar.gz
./learnvault-linux-amd64 --web-root web
```

Without `--web-root`, the server serves only the JSON API (web route returns 404).

### Windows Desktop Installer (NSIS)

The desktop installer is an NSIS `.exe` built in the release workflow. It installs to `%LOCALAPPDATA%\LearnVault` per-user. The installer supports English and Brazilian Portuguese. Missing WebView2 runtimes are downloaded automatically via `downloadBootstrapper`.

To build the installer locally on Windows:

```sh
pnpm install --frozen-lockfile
pnpm run build:desktop
pnpm exec tauri build --bundles nsis --config crates/desktop/tauri.conf.json
```

The output artifact is generated in `target/release/bundle/nsis/`. Verify against `SHA256SUMS`.

#### Desktop User Data Paths on Windows

Desktop state is kept strictly isolated from the external course library:

| Path | Contents |
| --- | --- |
| `%APPDATA%\app.learnvault.desktop\library.json` | Selected library root (versioned atomic configuration) |
| `%APPDATA%\app.learnvault.desktop\learnvault.sqlite` | Catalog metadata, notes, flashcards, and study progress |
| `%LOCALAPPDATA%\app.learnvault.desktop\` | WebView2 browser cache and state |

Reinstalling or updating preserves this data. The uninstaller includes an opt-in checkbox ("Delete application data") to wipe local application state if desired. Course folders are never touched or modified.

## Build & Release Constraints

- Each Rust binary (`crates/server`, `crates/desktop`) is the source of truth for its target.
- The web SPA is built into `crates/server/web/build` and `crates/desktop/frontend-dist`.
- Container definitions live in `deploy/container/`, `deploy/compose/`, and `deploy/quadlet/`.
- Containers never define behavior absent from the binary.
- Release Please manages changelogs and release candidate PRs.
- `.github/workflows/release.yml` triggers when a release is published on GitHub, validating tag format `vMAJOR.MINOR.PATCH`.

## Read-Only Verification

`scripts/verify-readonly.sh` verifies that the container runtime cannot write to the mounted library:

```sh
COMPOSE="docker compose --env-file deploy/compose/.env.example -f deploy/compose/compose.yaml" \
  ./scripts/verify-readonly.sh
```

## OCI Container Image Validation

`scripts/verify-rust-oci.sh` validates the Rust container image locally:

```sh
./scripts/verify-rust-oci.sh
CONTAINER_ENGINE=podman ./scripts/verify-rust-oci.sh
```

It validates:
- Non-root execution (UID 10001) without additional capabilities (`cap_drop: [ALL]`, `no-new-privileges: true`).
- Read-only root filesystem (`read_only: true`).
- Read-only library mount (`/library:ro`).
- Data directory persistence (`/data`).
- Healthcheck endpoint (`/api/healthz`).

## Configuration Reference

Native installations configure runtime options via environment variables or CLI flags:
- `DATA_DIR`: SQLite database and course registry directory.
- `CACHE_DIR`: Ephemeral cache directory.
- `LISTEN_ADDR`: Listening network address.
- `PORT`: HTTP port.
- `ADMIN_USERNAME` / `ADMIN_PASSWORD` (or `ADMIN_USERNAME_FILE` / `ADMIN_PASSWORD_FILE`): Initial admin credentials.

Course folders remain external and read-only across all platforms. Installers never copy course folders.

## Related Documentation

- [Architecture Overview](../architecture/overview.md) for core, server, and desktop boundaries.
- [Governance](../governance/overview.md) for contribution guidelines and CI checks.
- [Requirements](../product/requirements.md) for functional and non-functional requirements.
