# LearnVault - Deployment and Self-Hosting Guide with Docker

This directory contains the official deployment artifacts for **LearnVault** in production and self-hosting environments with Docker and Podman.

---

## Architecture Principles

1. **Read-only library (`/library:ro`)**: LearnVault never writes, moves, renames or deletes your original course files.
2. **Secure by default (zero privileges)**:
   - Runs as a non-root user (`UID 10001:10001`).
   - `read_only: true` on the container filesystem (only `/tmp` is mounted, as a strict tmpfs).
   - `no-new-privileges: true` and `cap_drop: ALL`.
   - Isolation on an internal network (`learnvault_net`).
   - The Docker socket is never mounted inside the application container.
3. **Reliability and persistence**:
   - SQLite in WAL mode with transactional integrity.
   - A native backup and restore tool with an integrity check (`PRAGMA integrity_check`).
4. **Trustworthy updates**:
   - New versions are detected through the GitHub Releases API without leaking local data.
   - The update script takes an **automatic pre-update backup**, runs a healthcheck and **rolls back immediately** on failure.

---

## File Layout

- `compose/compose.yaml`: Main Docker Compose file.
- `compose/compose.proxy.yaml`: Caddy reverse proxy with automatic HTTPS and OWASP headers.
- `compose/compose.secrets.yaml`: Optional configuration through Docker Secrets.
- `compose/compose.updater.yaml`: Optional (opt-in) companion service for automatic updates.
- `compose/Caddyfile`: Reverse proxy configuration tuned for HTTP Range streaming.
- `compose/.env.example`: Environment variable template.
- `container/Containerfile`: Multi-stage OCI definition (Node.js + Rust on Alpine).
- `scripts/backup.sh`: Transactional backup utility.
- `scripts/restore.sh`: Safe restore utility with validation.
- `scripts/update.sh`: Update script with a pre-update backup and rollback.
- `scripts/updater-daemon.sh`: Scheduled automatic update daemon.

---

## Getting Started

### 1. Initial setup

```sh
cd deploy/compose
cp .env.example .env
```

Edit the `.env` file and set your administrator credentials and paths:
- `ADMIN_USERNAME`: The initial administrator user.
- `ADMIN_PASSWORD`: A strong password (at least 12 characters).
- `DATA_DIR`: Where the database and metadata are stored.
- `LIBRARY_DIR`: Folder on the host, mounted read-only at `/library`. It sets the root the server is allowed to read and, on the first start, registers each course folder directly under it as a course — no manual import step. A folder that keeps the media files directly in the root is registered as a single course instead. Importing a subfolder also works (for example when your course folders are nested one level deeper); folders added after the first start are not registered automatically.

### 2. Run with Docker Compose

By default Compose uses the official image published on the GitHub Container Registry (GHCR), so a new release is one `docker compose pull && docker compose up -d` away (or automatic, with the opt-in updater below). You can also pin a version or build the image yourself:

```sh
# Option A: the official image (the default; recommended for production)
docker compose up -d

# Option B: pin a version
LEARNVAULT_IMAGE=ghcr.io/andersontavares0/learnvault:0.9.2 docker compose up -d

# Option C: build the image from the source code (contributors)
docker compose -f compose.yaml -f compose.build.yaml up -d --build
```

> **Tag channels:** the first release that is not a GitHub pre-release moves `latest`, `stable`, and
> the minor line. `0.9.0` is a pre-release and keeps only its exact tag, so until a later release
> exists, Option A fails to pull; pin `LEARNVAULT_IMAGE`
> to an exact published version as in Option B. Release candidates are no longer published, and the
> `:rc` tag is not updated any more; the existing `0.9.0-rc.N` tags never move `latest`, `stable`, or
> the minor line.

> **Upgrading from a setup that built the image locally:** if your `.env` still has `LEARNVAULT_IMAGE=learnvault-rust:local`, remove that line (the default is now the published image), or keep building by adding `-f compose.build.yaml` to every `docker compose` command.

Open `http://localhost:8090` in your browser.

### 3. Run with the Caddy reverse proxy (automatic HTTPS)

For deployments exposed on the local network or on the internet with your own domain and automatic TLS certificates:

```sh
DOMAIN=courses.example.com docker compose -f compose.yaml -f compose.proxy.yaml up -d
```

Caddy automatically handles SSL/TLS certificate renewal, zstd/gzip compression and protective headers (HSTS, CSP, X-Frame-Options, X-Content-Type-Options).

---

## Backup and Restore

### Creating a transactional backup

To take a hot backup of the database without interrupting users' playback:

```sh
# With the helper script
../../deploy/scripts/backup.sh

# Or directly through the container CLI
docker compose exec learnvault /app/learnvault-server backup --database /data/learnvault.sqlite --output /data/backups/my-backup.sqlite
```

The command performs the online backup through the SQLite API and runs `PRAGMA integrity_check` before finishing.

### Restoring from a backup

```sh
# With the script (integrity is validated first, and the swap is atomic)
../../deploy/scripts/restore.sh /path/to/my-backup.sqlite
```

---

## Update System

### Manual update with safe rollback

When a new version is published, you can run the update script:

```sh
../../deploy/scripts/update.sh [optional-version]
```

The script:
1. Reads the currently installed version.
2. Takes a transactional pre-update backup.
3. Downloads the versioned image.
4. Restarts the containers.
5. Checks the `healthcheck` for 60 seconds.
6. **Rolls back automatically**: if the new container fails the healthcheck or the migration is incompatible, the pre-update database is restored and the previous image runs again.
7. Writes a log entry to `/data/update-history.json`.

### Opt-in automatic updates

If you want LearnVault to check for and apply updates on its own during the night:
1. In `.env`, set:
   ```env
   AUTO_UPDATE=true
   AUTO_UPDATE_WINDOW=03:00-05:00
   # stable (default) installs only final releases; rc also accepts candidates.
   LEARNVAULT_CHANNEL=stable
   ```
2. Start the companion container:
   ```sh
   docker compose -f compose.yaml -f compose.updater.yaml --profile updater up -d
   ```
The companion updater container runs in isolation, queries the API and applies updates only inside the configured maintenance window. It never mounts the Docker socket: a proxy that only allows the required endpoints talks to the daemon.

`LEARNVAULT_CHANNEL` limits what can be installed: `stable` refuses candidates and `rc` also accepts them. In both cases the image needs a valid signature from the release workflow, and the updater never downgrades an installation: a target that is equal or older is ignored.

**Docker socket security.** The updater never mounts the host socket: it talks to `docker-proxy` (`tecnativa/docker-socket-proxy`, pinned by digest), which only enables the API sections used by `update.sh` (containers, start/stop, images, networks, volumes and exec) and refuses the rest (secrets, swarm, build, commit and plugins). This reduces the surface; it does not eliminate the risk: anyone able to create containers through the API could, in theory, mount the host. Therefore the updater is opt-in and should run only on machines you control. For a stronger guarantee, prefer the Quadlet with `podman-auto-update.timer`, which does not use a socket.

## Updater failure boundaries

Automatic updates remain opt-in. Maintenance windows use the updater container's local clock, include their start minute, and exclude their end minute. Overnight windows such as `23:30-01:15` are supported. Equal endpoints and malformed clock values are rejected. `CHECK_INTERVAL_SECONDS` must contain at most ten decimal digits and represent a value from 1 to 2147483647; leading zeros are accepted.

An update requires a successful backup and target image download before restarting the service. A healthy container is accepted only when its reported version matches the requested stable version. Startup failure or a version mismatch enters rollback using the previous immutable image ID for database restoration and service restart. A failed restore or restart stops recovery and leaves a nonzero exit status; it is never reported as success.

An atomic lock directory in `DATA_DIR/.update.lock` prevents simultaneous attempts against the same data directory. A process crash can leave a stale lock: after confirming that no updater is running, the operator must remove the empty lock directory before retrying. Do not remove an active updater's lock.
