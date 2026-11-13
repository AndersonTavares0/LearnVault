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

### Desktop Automatic Update Checks

The desktop application checks for a new version on launch and at most once every 24 hours, recording the last check time locally. A failed check stays quiet: the app keeps working offline and offers no retry loop. The result appears in the same update banner used everywhere else.

From the banner (or Settings → Updates), **Download update** fetches the signed manifest artifact in the background without pausing playback or reloading the page. Once the bytes are held, the banner offers **Restart to update**, with the release notes shown in place. Nothing restarts on its own: the click is the only trigger, so an open lesson is never interrupted. At install time the app checks the release again and applies the held bytes only while the same version is still offered; every failure is reported as a generic message with no path or URL.

### Desktop Database Before Migration

Before the desktop opens the database on a version whose schema is newer than the file on disk, it copies the file next to itself as `<database>.backup-<old-version>-<timestamp>` (for example `learnvault.sqlite.backup-12-1763000000`). The three newest backups are kept and older ones are pruned. The copy is best effort: a failed backup never blocks startup, but it is reported on stderr so it is not mistaken for success.

### Verify the container image

From the first release after this was added, every image pushed to GHCR is signed by the release workflow with cosign (keyless: no key to keep, the signature is tied to the workflow run). To check an image yourself:

```sh
cosign verify ghcr.io/andersontavares0/learnvault:0.9.0 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github\.com/AndersonTavares0/LearnVault/\.github/workflows/release\.yml@refs/(tags/v[0-9]+\.[0-9]+\.[0-9]+|heads/main)$'
```

The command succeeds only for an image signed by `release.yml` in this repository. The Compose updater (`deploy/scripts/update.sh`) runs the same check on the target version before it backs up or changes anything, and refuses to update to an image that is unsigned or signed by anything else. Set `LEARNVAULT_VERIFY_SIGNATURE=false` only for a private mirror that re-publishes the image. Images published before this change (up to v0.8.0) are not signed.

What this does and does not promise:

- The updater verifies the tag, then pulls and starts the verified digest (`repo@sha256:…`), so retagging the registry between the check and the pull has no effect.
- A signature proves the image came from `release.yml` in this repository, run for a version tag or for `main`. A manual run of the workflow on `main` can re-sign a rebuilt tag; the signature does not distinguish that from the original.
- The image is pushed before it is signed. If signing fails, the release is not published, but the tags already point at an unsigned image; the updater refuses it, but `docker compose pull` of `:latest` would not. Re-run the workflow to fix it.
- The updater installs `cosign` from the Alpine mirror when it starts, unpinned. Bake a pinned `cosign` into the updater image if that matters for your threat model.
- `LEARNVAULT_GITHUB_REPO` must look like `owner/name`, and `LEARNVAULT_VERIFY_SIGNATURE` must be exactly `true` or `false`; anything else stops the update.

### Automatic updates with Podman Quadlet

`deploy/quadlet/learnvault.container` pulls `ghcr.io/andersontavares0/learnvault:stable` and is marked `AutoUpdate=registry`. Enable the native Podman timer to apply new releases:

```sh
systemctl --user enable --now podman-auto-update.timer
podman auto-update --dry-run   # shows what would change
```

`Notify=healthy` delays the "started" signal until the container healthcheck passes. If a new image never becomes healthy, the unit restart fails and `podman auto-update` rolls back to the previous image. Back up the data directory first (`deploy/scripts/backup.sh`): a rollback restores the image, not the database.

To follow a slower line, change the tag to a minor line such as `:0.9`, or pin an exact version to turn automatic updates off. Previously the unit ran a locally built `localhost/learnvault:latest`; build and tag your own image if you prefer that.

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
- A network listener requires explicit initial admin credentials. Loopback-only first startup may use the temporary `admin/admin` account, which must change its password before accessing protected endpoints.
- `TRUSTED_PROXY_IPS`: Optional comma-separated IP addresses of reverse proxies allowed to supply `X-Real-IP` or `X-Forwarded-For` for login rate limiting. Forwarded headers from other peers are ignored; configure the proxy address as stable and ensure it overwrites both headers. When `X-Real-IP` is absent, the last `X-Forwarded-For` entry is used.

Course folders remain external and read-only across all platforms. Installers never copy course folders.

## Related Documentation

- [Architecture Overview](../architecture/overview.md) for core, server, and desktop boundaries.
- [Governance](../governance/overview.md) for contribution guidelines and CI checks.
- [Requirements](../product/requirements.md) for functional and non-functional requirements.
