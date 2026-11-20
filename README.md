<div align="center">

  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/design/branding/learnvault-banner.svg">
    <img src="docs/design/branding/learnvault-banner-light.svg" alt="LearnVault: your courses, your academy, untouched." width="100%">
  </picture>

  <br>

  **A private study academy for the course folders you already own.**

  Self-hosted or fully offline. LearnVault organizes your videos, audio and materials into a study space without moving, renaming or modifying a single file.

  [![CI Status](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg)](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml)
  [![Latest Release](https://img.shields.io/github/v/release/AndersonTavares0/LearnVault?color=1C1815&label=release)](https://github.com/AndersonTavares0/LearnVault/releases)
  [![License](https://img.shields.io/badge/license-MIT-A8761F.svg)](LICENSE)
  [![Rust](https://img.shields.io/badge/rust-1.99.0-black?logo=rust)](https://www.rust-lang.org)
  [![React 19](https://img.shields.io/badge/react-19-149eca?logo=react)](https://react.dev)
  [![Tauri 2](https://img.shields.io/badge/tauri-2-24c8db?logo=tauri)](https://tauri.app)

  [Why](#why-learnvault) • [Features](#features) • [Get it](#get-it) • [Quickstart](#quickstart) • [Name and mark](#the-name-and-the-mark) • [Development](#development) • [Documentation](docs/README.md)

</div>

---

## Why LearnVault

You bought or downloaded a course, and it sits in a folder. A bare media player forgets where you stopped, a cloud platform wants your files and your data. LearnVault is the third option: it reads the folder where it already is and builds the study space around it.

| | |
| :--- | :--- |
| **Yours** | A private library on your own machine or home server. The app ships its fonts and runs without a cloud account. Server update checks contact GitHub and can be disabled with `LEARNVAULT_UPDATE_CHECK=off`. Study records are stored locally in SQLite. Course files stay in their existing folders. |
| **Untouched** | The course library is strictly read-only. LearnVault never modifies, renames, copies or moves your media files. |
| **Moving forward** | Resume exactly where you stopped. Set a weekly study goal and see your streak and badges in Progress. |
| **Self-directed** | Nobody assigns the curriculum. You choose what to learn and when, with notes, search and spaced repetition to make it stick. |

### One engine, two ways to run it

- **Self-hosted server:** an Axum HTTP backend with a SQLite database, multi-user accounts, secure cookie sessions and on-demand transcoding, for a home server or homelab.
- **Offline desktop app:** a Tauri 2 native application with zero open network ports and complete offline capability.

Both ship the same React interface, in English (`en`, default) and Brazilian Portuguese (`pt-BR`), with instant language switching.

---

## Features

### Player

- **Playback controls:** persistent speed, keyboard shortcuts (`J`/`K`/`L`, `0-9`, `M`, `N`, `F`, `P`, `[`/`]`), Picture-in-Picture and automatic lesson advance.
- **Timeline markers:** pins for your notes directly on the scrubber.
- **Formats:** play browser-supported media directly. With FFmpeg installed, the server can convert other files on demand, keeping compatible video streams and encoding audio as AAC, with an H.264 fallback. The desktop app plays browser-supported formats directly and does not transcode. The web player converts `.srt` captions to WebVTT.
- **PDF reader:** PDF lessons open in the app with search, highlights, underlines and notes. The file itself is never written.

### Notes and review

- **Timestamped notes:** capture Markdown notes anchored to an exact moment. Clicking a note seeks the player to that instant.
- **Search:** query lesson titles, module folders and notes across the whole catalog.
- **Flashcards:** create flashcards for a lesson. Again schedules the card in 10 minutes. Good schedules it in one day when its interval is zero, then doubles the interval on later Good reviews, up to 365 days.

### Focus and habit

- **Pomodoro timer:** the timer tracks focus and breaks. Focus sessions keep the same ID when saving is retried, so each one counts once.
- **Study analytics:** Progress shows focus minutes by day, your study-day streak and active days toward your weekly goal.

### Accounts and access

- **Separate profiles:** each account keeps its own progress, notes, flashcard decks and focus stats.
- **Access control:** administrators can grant or restrict access to specific courses, or to the whole catalog, per user.

---

## Get it

Release assets are built automatically by GitHub Actions.

| Platform | Target | Format | Status | Notes |
| :--- | :--- | :--- | :--- | :--- |
| **Windows** | Desktop application | Installer (`.exe`) | **Available** | Per-user NSIS package with a signed auto-updater |
| **Linux** | Self-hosted server | Standalone tarball (`.tar.gz`) | **Available** | `amd64` and `arm64` binaries with the web app included |
| **Linux containers** (amd64 and arm64) | Containerized server | OCI image | **Available** | Published to GHCR (`ghcr.io/andersontavares0/learnvault`) and signed with cosign |
| **Linux** | Desktop application | Flatpak bundle (`.flatpak`) | **Available** | Available as amd64 and arm64 bundles on GitHub Releases. Install a newer bundle to update. See [`docs/operations/flatpak.md`](docs/operations/flatpak.md) |

Releases that include `SHA256SUMS.sigstore.json` (v0.9.6 does; v0.9.4 and v0.8.0 do not) carry a keyless cosign signature for `SHA256SUMS`. Download both files and your asset from the same tag, then verify the signed manifest before checking the asset. This needs cosign v3 or newer:

```sh
cosign verify-blob SHA256SUMS \
  --bundle SHA256SUMS.sigstore.json \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github[.]com/AndersonTavares0/LearnVault/[.]github/workflows/release[.]yml@refs/(tags/v[0-9]+[.][0-9]+[.][0-9]+|heads/main)$' \
  && sha256sum -c SHA256SUMS --ignore-missing
```

Build provenance, the SBOM and the container image check are in [`docs/releases/process.md`](docs/releases/process.md).

---

## Quickstart

### Option A: Docker Compose (self-hosted server)

Create a `compose.yaml`:

```yaml
services:
  learnvault:
    image: ghcr.io/andersontavares0/learnvault:latest
    container_name: learnvault
    restart: unless-stopped
    ports:
      - "${BIND_ADDR:-127.0.0.1}:8090:8090"
    environment:
      - DATA_DIR=/data
      - LISTEN_ADDR=0.0.0.0
      - PORT=8090
      - LIBRARY_DIR=/library
      - ADMIN_USERNAME=admin
      - ADMIN_PASSWORD=replace-with-your-own-password
    volumes:
      - ./data:/data
      - /path/to/your/courses:/library:ro
```

```sh
docker compose up -d
```

Choose your own administrator password before the first start. You can change it later in Settings > Account. The container runs as UID 10001, so make `./data` writable and the course folder readable by that user. The library is mounted `:ro`, so the container cannot write to your courses.

Open `http://localhost:8090` on the server. The port binds to `127.0.0.1` unless you set `BIND_ADDR`. `BIND_ADDR=0.0.0.0` is for LAN access and should sit behind the HTTPS reverse proxy in `deploy/`. Session cookies are `Secure` by default. If your browser rejects the Secure cookie on localhost, set `COOKIE_SECURE=false` for a loopback-only test. For login rate limiting behind a reverse proxy, list the proxy addresses in `TRUSTED_PROXY_IPS` and have each proxy append the address it saw to `X-Forwarded-For`; see [Distribution and operations](docs/operations/distribution.md#reverse-proxy-and-login-rate-limiting).

Hardened Compose files, a reverse proxy with HTTPS and Podman Quadlet units are in [`deploy/`](deploy/README.md).

### Option B: Linux standalone server

1. Download the latest `learnvault-linux-amd64.tar.gz` (or `learnvault-linux-arm64.tar.gz`) from the [Releases page](https://github.com/AndersonTavares0/LearnVault/releases).
2. Extract it and start the server:

```sh
tar -xzf learnvault-linux-amd64.tar.gz
./learnvault-linux-amd64 --web-root web --library-root /path/to/your/courses
```

The server listens on `127.0.0.1:8090`. On a fresh database, a loopback-only start with no bootstrap credentials creates a temporary `admin` / `admin` account. Sign in and change the password in Settings > Account to unlock the library. Pass `--web-root web` to serve the bundled interface; without it the server serves only the JSON API. For arm64, use `arm64` in both commands.

For local HTTP testing, `--insecure-http` drops the Secure attribute from the session cookie. Keep Secure cookies enabled behind an HTTPS proxy.

### Option C: Windows desktop application

1. Download `learnvault-windows-vX.Y.Z.exe` from [Releases](https://github.com/AndersonTavares0/LearnVault/releases).
2. Run the installer. It installs per user into `%LOCALAPPDATA%\LearnVault` and needs no administrator privileges.
3. Pick your course folder in the native dialog and study offline.

### Option D: Linux desktop application

Download `learnvault-linux-desktop-amd64.flatpak` or `learnvault-linux-desktop-arm64.flatpak` from the [Releases page](https://github.com/AndersonTavares0/LearnVault/releases), then run `flatpak install --user <file>`. The GNOME runtime comes from Flathub.

---

## The name and the mark

The mark is the **Academy Portico**: a Greek school front with a pediment, four columns and steps, and an arched door that stands for the vault. The door is gold. The motto is *Sapere aude*, "dare to know" (Horace, *Epistles*). A few other lines from antiquity appear in the interface, each beside its translation:

| Where | Line | Meaning |
| :--- | :--- | :--- |
| Sign-in | *Sapere aude* | Dare to know |
| Progress | *Nulla dies sine linea* | Not a day without a line |
| Importing a course | *Ad fontes* | Back to the sources |
| An empty library | *ἀρχὴ ἥμισυ παντός* | The beginning is half of the whole |

Logos, the seal, the banner and the icon sources are vector files in [`docs/design/branding/`](docs/design/branding/), with the usage rules in its [README](docs/design/branding/README.md). The design record is [`DESIGN.md`](DESIGN.md) and the product record is [`PRODUCT.md`](PRODUCT.md).

---

## Development

LearnVault is a Rust workspace with a React frontend:

- `crates/core`: domain engine, catalog scanner, media identification and SQLite migrations.
- `crates/server`: Axum HTTP server, session cookies, media streaming and delivery of the web app.
- `crates/desktop`: Tauri 2 desktop shell with native IPC commands.
- `apps/web-next`: React 19 and TypeScript single-page app built with Vite.

### Prerequisites

- **Rust:** 1.99.0 (pinned by `rust-toolchain.toml`)
- **Node.js:** 24 (used by CI)
- **pnpm:** 11.28.5 (set by `packageManager` in `package.json`)

### Setup

```sh
git clone https://github.com/AndersonTavares0/LearnVault.git
cd LearnVault

export PATH="$HOME/.cargo/bin:$PATH"
pnpm install --frozen-lockfile
```

### Common commands

```sh
pnpm dev                     # Rust API (8090) and Vite app (5174) with live reload
pnpm dev:tauri               # Tauri desktop application in development mode
pnpm verify --fast           # Type check, lint, Vitest, script tests, rustfmt, Clippy and Cargo tests
pnpm exec playwright test    # Playwright end-to-end browser tests
pnpm run build               # Production web assets
pnpm run build:desktop       # Embedded desktop bundle
cargo clippy --all-targets -- -D warnings
cargo test                   # Core and server (desktop: cargo test -p learnvault-desktop)
```

---

## Documentation

- [Documentation index](docs/README.md)
- [Architecture overview](docs/architecture/overview.md)
- [API reference](docs/reference/api.md)
- [Product requirements](docs/product/requirements.md) and [roadmap](docs/product/roadmap.md)
- [Distribution and operations](docs/operations/distribution.md)
- [Architecture decision records](docs/architecture/adr/README.md)
- [Security policy](.github/SECURITY.md)

---

## Contributing

Read [`.github/CONTRIBUTING.md`](.github/CONTRIBUTING.md) and [`AGENTS.md`](AGENTS.md) before opening a pull request.

1. Open an issue for a proposed feature and discuss the design first.
2. Write Conventional Commits in English, for example `feat(i18n): add language switcher`.
3. Keep the **read-only course library** invariant on every code path.
4. Run `pnpm verify --fast` before pushing.

---

## License

LearnVault is licensed under the [MIT License](LICENSE).
