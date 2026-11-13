# Release Process

LearnVault adheres to Semantic Versioning: `MAJOR.MINOR.PATCH`.

Before `1.0.0`, Release Please increments the patch version for `feat:`, `fix:`, and `perf:` commits, and increments the minor version for breaking changes. From `1.0.0`, breaking changes increment major. Tags and GitHub release names are generated as `vMAJOR.MINOR.PATCH`; release notes and the changelog are generated from English Conventional Commits.

1. Merge approved changes using Conventional Commits into `main`; these changes accumulate for the next monthly release.
2. On the first day of each month (UTC), Release Please opens or updates one release PR with version files and generated `docs/releases/CHANGELOG.md` notes.
3. Review and merge the release PR to publish the accumulated changes. Ordinary code PRs do not publish releases; merging the release PR creates the tag and GitHub Release. For an urgent security release outside the monthly schedule, manually run the `Release Please` workflow on `main` to prepare the candidate early.
4. The published release workflow builds Linux server tarballs and the Windows desktop NSIS installer, verifies checksums with `SHA256SUMS`, and generates the contributors list.
5. Verify assets, checksums, and the Contributors section before general distribution.

The release workflow receives the published tag via environment variables and validates that it strictly matches `vMAJOR.MINOR.PATCH` with non-negative numeric components and no leading zeros.

## How candidates are created

Release Please runs in pre-release mode (`.github/release-please/release-please-config.json`: `versioning: prerelease`, `prerelease: true`, `prerelease-type: rc`). Each release pull request writes the English changelog section and the `vX.Y.Z-rc.N` tag when merged. Merging it publishes the pre-release, which then waits for approval in the `release` environment.

## Promoting a candidate to stable

While `prerelease` is `true`, every release is a candidate. To publish the stable release, open one pull request that sets `"prerelease": false` and `"versioning": "default"` in the config. Its release pull request then produces `vX.Y.Z`, which reuses the tested image digest.

## Release candidates (rc)

Before a release candidate or a stable release is pushed or published, its container and publish jobs wait for approval in the `release` GitHub environment (required reviewer). No tag moves without that approval.

Soak policy: a release candidate runs for 3 to 7 days before its stable release. The stable tag reuses the tested image digest, and any fix found during the soak gets a new candidate (`rc.N+1`) instead of an edit to the candidate.

A release candidate is a manually created GitHub pre-release tagged `vMAJOR.MINOR.PATCH-rc.N` (for example `v0.9.0-rc.1`) on a commit of `main`. The workflow requires its GitHub release to be published and marked as a pre-release, and a stable tag to be a published non-pre-release.

A release candidate publishes the same assets and the container tag `:MAJOR.MINOR.PATCH-rc.N` only. It never moves `:stable`, `:latest` or `:MAJOR.MINOR`, and GitHub does not offer it as the latest release, so the desktop updater (`releases/latest/download/latest.json`) and the server update notice ignore it.

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

## Troubleshooting

- **Desktop update is not offered after a release.** The release must be published (not a draft) and not a pre-release, and `latest.json` must carry a version greater than the installed app. Check with `gh release view <tag> --json isDraft,isPrerelease`.
- **The release workflow refuses the tag.** The tag must match `vX.Y.Z` or `vX.Y.Z-rc.N` exactly. A candidate must be a published pre-release, and a stable tag must be a published non-pre-release.
- **Signature verification fails in the updater or in `update.sh`.** The image or the update file was not signed by this repository's release workflow on a release tag or `main`. Re-run the release workflow for the tag instead of editing the assets by hand.
- **The container image has no version tag.** The image is pushed by digest first and tagged only after the vulnerability scan and the signature check pass. If the scan fails, no tag is applied, which is intended.
- **Windows installer build fails on a candidate.** Check whether the NSIS build accepts the pre-release version string in the build log. A failure stops before the publish job, so nothing is published.
