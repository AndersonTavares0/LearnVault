# Distribution & Operations

**Audience:** operators who publish, install, or deploy LearnVault.
Developers should start with the [README](../../README.md) and the [Architecture Overview](../architecture/overview.md).

The current release workflow attaches the Windows NSIS installer, Linux server tarballs and Linux desktop Flatpak bundles ([`flatpak.md`](flatpak.md)) to GitHub Releases and pushes an OCI image to GHCR. The container runs the same server binary as a native install and serves the SPA from `/app/web`. Docker and Podman are optional.

## Release Targets

| Target | Artifact | Purpose | Status |
| --- | --- | --- | --- |
| Linux Server | `learnvault-linux-<arch>.tar.gz` (amd64, arm64) | Primary Linux server deployment | Available |
| Self-hosted | OCI image via Compose or Quadlet (Docker/Podman) | Containerized server | Local or published image |
| Windows Desktop | NSIS installer (`.exe`) | Per-user install in `%LOCALAPPDATA%\LearnVault`, no admin rights | Available, listed in `SHA256SUMS` |
| Linux Desktop | `learnvault-linux-desktop-<arch>.flatpak` (amd64, arm64) | Desktop app | First published in the v0.9.1 pre-release; first regular release v0.9.5. Built and sandbox-checked in CI |

The full asset list is in [Published Release Assets](../releases/process.md#published-release-assets).

## Choosing a Target

- **Windows Desktop:** run the NSIS installer.
- **Linux Server:** extract the tarball (see below).
- **Containers:** use Docker Compose or the Podman Quadlet unit in `deploy/`.
- **Linux Desktop:** `flatpak install --user <file>`. The GNOME runtime comes from Flathub. See [`flatpak.md`](flatpak.md).

### Update Notifications

The server checks the latest stable GitHub release over HTTPS and compares it with the running version using strict SemVer. The request times out after 5 seconds, reads at most 64 KiB, and normally reuses a cached result for an hour. Checks are enabled by default. Set `LEARNVAULT_UPDATE_CHECK=off` or `false`, or pass `--no-update-check`, to disable them. Source builds display `dev` and use `0.0.0` for version comparisons. The server reports availability without downloading or installing the release.

### Desktop Update Checks

The desktop app checks for a new version on launch and at most once every 24 hours, and stores the time of the last check locally. A failed check is silent: the app keeps working offline and does not retry. The result appears in the update banner.

In the banner or in Settings → Updates, **Download update** fetches the signed update in the background without pausing playback or reloading the page. When the download finishes, the banner shows **Restart to update** with the release notes. Nothing restarts on its own, so an open lesson is never interrupted. At install time the app checks the release again and applies the downloaded update only if the same version is still offered. Failures are reported as a generic message with no path or URL.

Inside the Flatpak the updater is off; a new version arrives as a new bundle.

### Desktop Database Before Migration

When the schema of the installed version is newer than the database file, the desktop copies the file next to itself as `<database>.backup-<old-version>-<timestamp>` (for example `learnvault.sqlite.backup-12-1763000000`) before opening it. The three newest backups are kept and older ones are deleted. A failed backup does not block startup; it is written to stderr.

### Verify the Container Image

The release workflow signs the combined image index with keyless cosign, so there is no key to manage and the signature is tied to the workflow run. To check an image:

```sh
cosign verify ghcr.io/andersontavares0/learnvault:0.9.6 \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github\.com/AndersonTavares0/LearnVault/\.github/workflows/release\.yml@refs/(tags/v[0-9]+\.[0-9]+\.[0-9]+|heads/main)$'
```

The command succeeds only for an image signed by `release.yml` in this repository. Images up to v0.8.0 are not signed.

The Compose updater (`deploy/scripts/update.sh`) runs the same check on the target version before it backs up or changes anything, and refuses an image that is unsigned or signed by anything else. Set `LEARNVAULT_VERIFY_SIGNATURE=false` only for a private mirror that re-publishes the image.

Limits of the check:

- The updater verifies the tag, then pulls and starts the verified digest (`repo@sha256:…`), so retagging the registry between check and pull has no effect.
- A signature proves the image came from `release.yml` in this repository, run for a version tag or for `main`. A manual run on `main` can re-sign a rebuilt tag, and the signature cannot tell that apart from the original.
- The workflow first pushes the image index under a temporary `staging-<run id>` tag, then signs and verifies it. Version, minor-line, `stable` and `latest` tags are applied only after verification, so a signing failure can leave an unsigned staging image but moves no release tag.
- If verification is enabled and `cosign` is missing, the updater installs it from the Alpine mirror, unpinned. Bake a pinned `cosign` into the updater image if your threat model needs that.
- `LEARNVAULT_CHANNEL` selects which releases may be installed. The `stable` channel (the default) accepts `X.Y.Z`, tries the server update endpoint and falls back to GitHub's latest release. The `rc` channel also accepts `X.Y.Z-rc.N` and reads the first entry in the GitHub release list. Both channels verify signatures by default.
- When the installed version matches the channel's version format, the updater installs only newer versions; otherwise it logs the comparison and exits without touching the containers.
- `LEARNVAULT_GITHUB_REPO` must use `owner/name`. `LEARNVAULT_VERIFY_SIGNATURE` defaults to `true` when unset or empty and otherwise accepts only `true` or `false`. Any other value stops the update.

### Automatic Updates with Podman Quadlet

`deploy/quadlet/learnvault.container` pulls `ghcr.io/andersontavares0/learnvault:stable` and sets `AutoUpdate=registry`. Enable the native Podman timer to apply new releases:

```sh
systemctl --user enable --now podman-auto-update.timer
podman auto-update --dry-run   # shows what would change
```

`Notify=healthy` holds the "started" signal until the container healthcheck passes. If a new image never becomes healthy, the unit restart fails and `podman auto-update` rolls back to the previous image. Back up the data directory first (`deploy/scripts/backup.sh`), because a rollback restores the image, not the database.

To follow a slower line, change the tag to a minor line such as `:0.9`. To turn automatic updates off, pin an exact version.

### Linux Server Tarball

`learnvault-linux-<arch>.tar.gz` contains the `learnvault-linux-<arch>` binary, `LICENSE`, and the SPA files in `web/` (including `web/200.html`). Extract it and point `--web-root` at `web`:

```sh
tar -xzf learnvault-linux-amd64.tar.gz
./learnvault-linux-amd64 --web-root web
```

Without `--web-root` the server serves only the JSON API.

### Windows Desktop Installer (NSIS)

The release workflow builds the NSIS `.exe`. It installs per user to `%LOCALAPPDATA%\LearnVault` and offers English and Brazilian Portuguese. If the WebView2 runtime is missing, the installer downloads it (`downloadBootstrapper`).

To build it locally on Windows:

```sh
pnpm install --frozen-lockfile
pnpm run build:desktop
pnpm exec tauri build --bundles nsis --config crates/desktop/tauri.conf.json
```

The installer lands in `target/release/bundle/nsis/`. Verify downloaded installers against `SHA256SUMS`.

#### Desktop User Data Paths on Windows

Desktop state is kept apart from the course library:

| Path | Contents |
| --- | --- |
| `%APPDATA%\app.learnvault.desktop\library.json` | Selected library root |
| `%APPDATA%\app.learnvault.desktop\learnvault.sqlite` | Catalog metadata, notes, flashcards, study progress |
| `%LOCALAPPDATA%\app.learnvault.desktop\` | WebView2 cache and state |

Reinstalling or updating keeps this data. The uninstaller has an opt-in checkbox to delete the application data. Course folders are never touched.

## Build & Release Constraints

- Containers add no behavior of their own: the image runs `learnvault-server --web-root /app/web` as UID 10001.
- The SPA builds into `crates/server/web/build` and `crates/desktop/frontend-dist`.
- Container definitions live in `deploy/container/`, `deploy/compose/`, and `deploy/quadlet/`.
- Release Please manages the changelog and the release PR.
- `.github/workflows/release.yml` runs when a release is published, or manually to rebuild the assets of an existing release (see [Asset Repair](../releases/process.md#asset-repair--manual-triggers)). It accepts only tags shaped `vMAJOR.MINOR.PATCH`.

## Read-Only Verification

`scripts/verify-readonly.sh` checks that the container cannot write to the mounted library:

```sh
COMPOSE="docker compose --env-file deploy/compose/.env.example -f deploy/compose/compose.yaml" \
  ./scripts/verify-readonly.sh
```

## OCI Image Validation

`scripts/verify-rust-oci.sh` validates the container image locally:

```sh
./scripts/verify-rust-oci.sh
CONTAINER_ENGINE=podman ./scripts/verify-rust-oci.sh
```

It checks:

- Non-root execution (UID 10001), `no-new-privileges`, and all capabilities dropped.
- A read-only root filesystem and a read-only, readable `/library` mount that the container cannot write to.
- A writable `/data` whose database and directory survive a restart.
- `/api/healthz` returns JSON with `status: ok` and the running `version`, with `Cache-Control: no-store`, the image `HEALTHCHECK` becomes healthy, and health returns after a restart. Under Podman the script may skip the check of the image healthcheck command.
- `/api/catalog` and `/api/media` return `401` without a session.
- The packaged binary reports its version, and the script's validation container is removed afterwards.

## Configuration Reference

Environment variables and CLI flags (a flag overrides its variable):

| Variable | Flag | Default | Purpose |
| --- | --- | --- | --- |
| `LISTEN_ADDR`, `PORT` | `--listen-addr <ip:port>` | `127.0.0.1`, `8090` | `LISTEN_ADDR` accepts a literal IP or a full socket address; a full socket address ignores `PORT`. The flag takes a full socket address and overrides both. |
| `DATA_DIR` | `--database <file>` | `data` (database `data/learnvault.sqlite`) | Database and course registry location |
| `LIBRARY_DIR` | `--library-root <dir>` | unset | Initial library seed (see [API reference](../reference/api.md#catalog-organization)) |
| | `--web-root <dir>` | unset | Serve the SPA from this directory |
| `ADMIN_USERNAME` or `ADMIN_USERNAME_FILE` | `--admin-username` | unset | Initial admin username; the `_FILE` variable reads it from a file. Setting both forms is an error. |
| `ADMIN_PASSWORD` or `ADMIN_PASSWORD_FILE` | none | unset | Initial admin password. The `_FILE` form reads the password from a file. Setting both forms is an error. There is no password flag. |
| `COOKIE_SECURE` | `--insecure-http` | `true` | Session cookie `Secure` flag; the flag sets it to `false` for local HTTP only |
| `LEARNVAULT_UPDATE_CHECK` | `--no-update-check` | on | Unset or blank enables checks. After trimming, `on` and `true` enable and `off` and `false` disable, case-insensitively. Other values fail startup unless the CLI flag disables checks. |
| `TRUSTED_PROXY_IPS` | none | empty | Comma-separated literal IP addresses of trusted reverse proxies, used only for login throttling (see below) |

Creating the first administrator on a network listener requires explicit credentials. A loopback-only first start may create temporary `admin/admin` credentials. A session with a pending credential change can read `/api/me` and update credentials at `/api/account/credentials`.

The server ignores `HTTP_PROXY`, `HTTPS_PROXY` and `ALL_PROXY` for WebDAV remote sources and connects directly to the validated address, so the server must be able to reach each source without a proxy (see [Remote Course Sources](../reference/api.md#remote-course-sources-webdav-and-smb)).

Course folders stay external and read-only on every platform. Installers never copy them.

## Reverse Proxy and Login Rate Limiting

Login rate limiting is keyed on the client address. Behind a reverse proxy, set `TRUSTED_PROXY_IPS` to the proxy IP addresses (comma-separated literals; CIDR ranges are not supported and invalid entries are ignored) and make the proxy append the address it observed to `X-Forwarded-For`. When the direct peer is trusted, the server walks the `X-Forwarded-For` chain right to left, skips trusted addresses, and uses the first untrusted one. If none is usable, it uses the socket peer. Client-supplied entries to the left of the first untrusted address are never used. List every trusted hop.

> **Warning:** `X-Real-IP` is ignored. A proxy that sets only `X-Real-IP` puts every client in the proxy's rate-limit bucket.
>
> **Upgrading:** if you relied on `X-Real-IP`, add `X-Forwarded-For` to the proxy configuration and keep the proxy address in `TRUSTED_PROXY_IPS`.

nginx, in the `location` that proxies to LearnVault:

```nginx
proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
```

Caddy's `reverse_proxy` sends `X-Forwarded-For` by default. If another proxy or load balancer sits in front of Caddy, list it in `trusted_proxies` (global `servers` options) so Caddy appends to the incoming chain instead of replacing it.

```caddyfile
example.com {
	reverse_proxy 127.0.0.1:8090
}
```

Traefik appends the client address automatically. Only if Traefik itself sits behind another trusted proxy, list that proxy under the entry point's forwarded headers (static configuration):

```yaml
entryPoints:
  websecure:
    address: ":443"
    forwardedHeaders:
      trustedIPs:
        - "192.0.2.10/32"
```

Replace the example addresses with your own and check your proxy's documentation for the options in your version.

## Related Documentation

- [Architecture Overview](../architecture/overview.md): core, server, and desktop boundaries.
- [Governance](../governance/overview.md): contribution guidelines and CI checks.
- [Requirements](../product/requirements.md): functional and non-functional requirements.
