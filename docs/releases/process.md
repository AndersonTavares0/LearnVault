# Release Process

LearnVault adheres to Semantic Versioning: `MAJOR.MINOR.PATCH`.

1. Merge approved changes using Conventional Commits into `main`.
2. Release Please automatically opens or updates a release candidate PR with updated version files and `docs/releases/CHANGELOG.md`.
3. Merge the release PR; this tags the commit and generates the GitHub Release.
4. The published release workflow builds Linux server tarballs and the Windows desktop NSIS installer, verifies checksums with `SHA256SUMS`, and generates the contributors list.
5. Verify assets, checksums, and the Contributors section before general distribution.

The release workflow receives the published tag via environment variables and validates that it strictly matches `vMAJOR.MINOR.PATCH` with non-negative numeric components and no leading zeros.

## Canonical SPA Bundling

The Linux server binary (`crates/server`) embeds static web assets directly from the canonical build output directory `crates/server/web/build`, produced by the web SPA build (`apps/web`). The `linux` job in the release workflow compiles `crates/server` for target architectures (`x86_64` and `aarch64`) with embedded SPA assets.

## Asset Repair & Manual Triggers

On a GitHub `release` event, Actions executes the workflow definition from the **tagged commit**, not `main`. If a release was tagged before a workflow fix reached `main`, the workflow can be triggered manually by repository maintainers:

1. Navigate to `Actions` → `Release` → `Run workflow`.
2. Input the exact semantic tag (e.g. `v0.8.1`).
3. The workflow validates the tag, confirms or initializes the release (supporting drafts for atomic publishing), rebuilds all platform assets from that tag, verifies that the tag has not moved, uploads signed artifacts with `--clobber`, publishes the release, and appends the Contributors section.

The repair workflow never moves or recreates tags. The Contributors section is idempotent via the `<!-- learnvault:contributors -->` comment marker.

## Published Release Assets

Each release publishes seven files on GitHub Releases, alongside the OCI container image on GHCR (`ghcr.io/andersontavares0/learnvault`):

| Asset | File Name |
| --- | --- |
| Linux Server, x86-64 | `learnvault-linux-amd64.tar.gz` |
| Linux Server, ARM64 | `learnvault-linux-arm64.tar.gz` |
| Windows Installer (NSIS) | `learnvault-windows-vMAJOR.MINOR.PATCH.exe` |
| Windows Updater Bundle | `learnvault-windows-vMAJOR.MINOR.PATCH.nsis.zip` |
| Windows Updater Ed25519 Signature | `learnvault-windows-vMAJOR.MINOR.PATCH.nsis.zip.sig` |
| Tauri 2 Updater Manifest | `latest.json` |
| SHA-256 Checksums | `SHA256SUMS` |

Asset naming rules:
- Prefix is always `learnvault-`; never use alternative suffixes or architecture aliases (`x64`, `win64`).
- Architecture naming follows Go/Actions standards: `amd64` and `arm64`, never `x86_64` or `x64`.
- The Windows installer includes the version in its filename because users download it standalone.
- `SHA256SUMS` is generated automatically via `sha256sum`.

Release Please is the single source of truth for `docs/releases/CHANGELOG.md`. After uploading assets, the release workflow appends an idempotent Contributors section. Dependabot identities are excluded. Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`) must be written in English to keep the generated changelog clean and consistent.

Configuration is defined in `.github/release-please/release-please-config.json` and manifest in `.github/release-please/release-please-manifest.json`.
Database schema migrations must always be documented in release notes. Releases must never contain course files, local databases, secrets, or cache artifacts.
