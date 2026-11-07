<div align="center">
  <img src="apps/web/public/favicon.svg" alt="LearnVault Logo" width="64" height="64">
  <h1>LearnVault</h1>
  <p>Your courses, organized in a personal study space you control.</p>
  <p>
    <a href="https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml"><img src="https://github.com/AndersonTavares0/LearnVault/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-4f8f68.svg" alt="MIT License"></a>
  </p>
  <p>
    <a href="https://github.com/AndersonTavares0/LearnVault/releases/latest">Download</a> ·
    <a href="#what-you-can-do">Explore Features</a> ·
    <a href="#contributing">Contributing</a>
  </p>
</div>

> **Early Stage.** LearnVault is under active development ahead of version 1.0. Changes between releases may affect features and data schemas; always maintain your own course folder backups.

---

## Study at your own pace

LearnVault indexes course folders that you already own on your disk. It builds a structured study space where you can track progress, watch videos, listen to audios, take timestamps notes, practice flashcards, and cultivate focus—all without moving or modifying your original files.

You can run LearnVault as a self-hosted server accessed from any web browser or open it directly as a 100% offline desktop application.

## What you can do

- **Preserve original files:** The course library is strictly read-only. Your directory structure and filenames remain completely intact.
- **Searchable catalog:** Quickly search across courses, modules, lessons, notes, and materials.
- **Video & audio player:** Playback speed controls, auto-advance, resume position tracking, subtitles (SRT/VTT) with on-the-fly conversion, and on-demand FFmpeg transcoding fallback.
- **Study tools:** Spaced repetition flashcards (SM-2 algorithm) and lesson notes synchronized with playback.
- **Gamification & Focus Forest:** Built-in Pomodoro timer with an evolving botanical study forest and weekly streak rewards.
- **Multi-user access:** Local student accounts and granular per-course access controls for household or lab servers.
- **Internationalization (i18n):** Native support for English (`en`, default) and Brazilian Portuguese (`pt-BR`), selectable directly from the interface.

## Choose how to use

| Feature | Server + Web SPA | Desktop Offline (Tauri 2) |
| --- | --- | --- |
| **Best for** | Hosting on a home server/NAS and accessing from any device | Personal offline study without open network ports or credentials |
| **Availability** | Linux x86-64 / ARM64 tarballs, Docker/Podman | Windows installer (`.exe`), Linux bundle |
| **Auth** | Multi-user login with admin dashboard | Instant single-user local access |

Check out the latest builds on the [Releases page](https://github.com/AndersonTavares0/LearnVault/releases/latest) and read the [Distribution Guide](docs/operations/distribution.md) for details.

## Development setup

Requirements:
- **Rust:** 1.99+ (enforced by `rust-toolchain.toml`)
- **Node.js:** 24+
- **pnpm:** 11.28.5 (specified in `package.json`)

Install workspace dependencies:

```sh
pnpm install --frozen-lockfile
```

### Running locally

| Command | Description |
| --- | --- |
| `pnpm dev` | Starts the Rust API server at `http://127.0.0.1:8090` and Vite dev server at `http://localhost:5173` |
| `pnpm dev:tauri` | Launches the desktop application with hot reload |
| `pnpm verify` | Runs all PR validation checks (Rust format/clippy/tests + Web checks + scripts) |
| `pnpm verify --fast` | Runs fast checks skipping desktop bundles and security audits |

On your first web launch, sign in with default credentials `admin` / `admin`, import your course directories, and update your password.

## Contributing

We welcome contributions! Please review our [Contributing Guide](.github/CONTRIBUTING.md) and [Governance Policy](docs/governance/overview.md) before submitting pull requests.

1. Pick or open an Issue for proposed feature changes; use Discussions for design debates.
2. Follow Conventional Commits in English (e.g., `feat(i18n): add language selector`).
3. Never include proprietary course files, database binaries, or private media in commits or pull requests.
4. Verify your branch with `pnpm verify` before opening a pull request.

## Documentation

- [Documentation Index](docs/README.md)
- [Architecture Overview](docs/architecture/overview.md)
- [Governance & Guidelines](docs/governance/overview.md)
- [Product Roadmap](docs/product/roadmap.md)
- [Security Policy](.github/SECURITY.md)
- [MIT License](LICENSE)
