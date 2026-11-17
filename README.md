<div align="center">

  <picture>
    <source media="(prefers-color-scheme: light)" srcset="docs/design/branding/learnvault-banner-light.svg">
    <img src="docs/design/branding/learnvault-banner.svg" alt="LearnVault: your courses, your academy, untouched." width="100%">
  </picture>

  <br>

  **A private study academy for the course folders you already own.**

  Self-hosted or fully offline. LearnVault organizes your videos, audio and materials into a study space without moving, renaming or modifying a single file.

  [![CI Status](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg)](https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml)
  [![Latest Release](https://img.shields.io/github/v/release/AndersonTavares0/LearnVault?include_prereleases&color=1C1815&label=release)](https://github.com/AndersonTavares0/LearnVault/releases)
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
| **Yours** | A private library on your own machine or home server. No telemetry, no third-party CDNs, no cloud dependency. Everything lives in one local SQLite file. |
| **Untouched** | The course library is strictly read-only. LearnVault never modifies, renames, copies or moves your media files. |
| **Moving forward** | Resume exactly where you stopped. A streak, a weekly goal and quiet badges make progress visible without turning study into a game. |
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
- **Formats:** native playback, automatic WebVTT conversion for SubRip (`.srt`) subtitles, and on-demand FFmpeg remuxing for extended containers (`.mkv`, `.avi`, `.wmv`) and audio formats.
- **PDF reader:** PDF lessons open in the app with search, highlights, underlines and notes. The file itself is never written.

### Notes and review

- **Timestamped notes:** capture Markdown notes anchored to an exact moment. Clicking a note seeks the player to that instant.
- **Search:** query lesson titles, module folders and notes across the whole catalog.
- **Flashcards (SM-2):** generate and drill cards per lesson, scheduled by the SuperMemo-2 spaced repetition algorithm.

### Focus and habit

- **Offline-safe Pomodoro:** a focus timer with local queuing and conflict-free synchronization.
- **Study analytics:** daily focus minutes, completion streaks and weekly consistency.
- **Habit first:** a forgiving streak, a weekly goal and quiet badges instead of a virtual garden.

### Accounts and access

- **Separate profiles:** each account keeps its own progress, notes, flashcard decks and focus stats.
- **Access control:** administrators can grant or restrict access to specific courses, or to the whole catalog, per user.

---

## Get it

Release assets are built automatically by GitHub Actions.

| Platform | Target | Format | Status | Notes |
| :--- | :--- | :--- | :--- | :--- |
| **Windows** | Desktop application | Installer (`.exe`) | **Available** | Per-user NSIS package with a signed auto-updater |
| **Linux** | Self-hosted server | Standalone tarball (`.tar.gz`) | **Available** | `x86_64` and `aarch64` binaries with the web app embedded |
| **Any OS** | Containerized server | OCI image | **Available** | Published on GHCR (`ghcr.io/andersontavares0/learnvault`) |
| **Linux** | Desktop application | Flatpak bundle (`.flatpak`, amd64 and arm64) | **Available** from the release that ships it | Attached to each GitHub Release, no store. Built and sandbox-checked in CI. See [`docs/operations/flatpak.md`](docs/operations/flatpak.md) |

Checksums are in the `SHA256SUMS` file attached to each [GitHub Release](https://github.com/AndersonTavares0/LearnVault/releases).

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
      - "8090:8090"
    environment:
      - DATA_DIR=/data
      - LISTEN_ADDR=0.0.0.0
      - PORT=8090
      - ADMIN_USERNAME=admin
      - ADMIN_PASSWORD=admin-change-me-promptly
    volumes:
      - ./data:/data
      - /path/to/your/courses:/library:ro
```

```sh
docker compose up -d
```

Open `http://localhost:8090`, sign in with the credentials above and change the administrator password in Account Settings. The library is mounted `:ro`, so the container cannot write to your courses even by mistake.

Hardened Compose files, a reverse proxy with HTTPS and Podman Quadlet units live in [`deploy/`](deploy/README.md).

### Option B: Linux standalone server

1. Download the latest `learnvault-linux-amd64.tar.gz` from the [Releases page](https://github.com/AndersonTavares0/LearnVault/releases).
2. Extract it and start the server:

```sh
tar -xzf learnvault-linux-amd64.tar.gz
./learnvault-linux-amd64 --web-root web --library-root /path/to/your/courses
```

### Option C: Windows desktop application

1. Download `learnvault-windows-vX.Y.Z.exe` from [Releases](https://github.com/AndersonTavares0/LearnVault/releases).
2. Run the installer. It installs per user into `%LOCALAPPDATA%\LearnVault` and needs no administrator privileges.
3. Pick your course folder in the native dialog and study offline.

---

## The name and the mark

A **vault** keeps something safe: here, your own library, read-only and private. An **academy** is where people study by their own initiative. Plato's Academy stood on a sacred olive grove outside Athens, and the Pythagoreans before him held a triangle of ten dots, the *tetractys*, as their sacred symbol.

The mark is the **Academy Portico**: a Greek school front with a gold arched door. The motto is *Sapere aude*, "dare to know" (Horace, *Epistles*; Kant made it the motto of the Enlightenment). A few other lines from antiquity appear in the interface, each beside its translation:

| Where | Line | Meaning |
| :--- | :--- | :--- |
| Sign-in | *Sapere aude* | Dare to know |
| Progress | *Nulla dies sine linea* | Not a day without a line |
| Importing a course | *Ad fontes* | Back to the sources |
| An empty library | *ἀρχὴ ἥμισυ παντός* | The beginning is half of the whole |

Logos, the banner and the icon sources are vector files in [`docs/design/branding/`](docs/design/branding/). The design record is [`DESIGN.md`](DESIGN.md) and the product record is [`PRODUCT.md`](PRODUCT.md).

---

## Development

LearnVault is a Rust workspace with a React frontend:

- `crates/core`: domain engine, catalog scanner, media identification and SQLite migrations.
- `crates/server`: Axum HTTP server, session cookies, media streaming and delivery of the web app.
- `crates/desktop`: Tauri 2 desktop shell with native IPC commands.
- `apps/web-next`: React 19 and TypeScript single-page app built with Vite.

### Prerequisites

- **Rust:** 1.99.0 (pinned by `rust-toolchain.toml`)
- **Node.js:** 24+
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
pnpm dev                     # Rust API (8090) and Vite app (5173) with live reload
pnpm dev:tauri               # Tauri desktop application in development mode
pnpm verify --fast           # Fast verification: app checks, Vitest, Cargo check and tests
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
