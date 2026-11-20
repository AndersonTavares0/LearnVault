# Release Process

LearnVault uses Semantic Versioning (`MAJOR.MINOR.PATCH`). The [release policy](../../.agents/policies/releases.md) holds the rules for version numbers, pacing, pre-release retention and release notes, and [ADR 0006](../architecture/adr/0006-release-versioning-and-prerelease-retention.md) gives the reasons.

Before `1.0.0`, Release Please bumps the patch version for `feat:`, `fix:` and `perf:` commits and the minor version for breaking changes. From `1.0.0`, breaking changes bump the major version. Tags and release names are `vMAJOR.MINOR.PATCH`, and the changelog and release notes are generated from English Conventional Commits.

1. Merge approved changes using Conventional Commits into `main`; these changes accumulate for the next monthly release.
2. At 12:00 UTC on the first day of each month, Release Please opens or updates one release PR with the version files and the generated `docs/releases/CHANGELOG.md` section.
3. Merging the release PR creates the tag and the GitHub Release; ordinary code PRs never publish. For an urgent security release outside the schedule, run the `Release Please` workflow manually on `main` to prepare the release PR early.
4. The release workflow builds the Linux server tarballs, the Flatpak bundles ([`flatpak.md`](../operations/flatpak.md)), the Windows NSIS installer and the container image. It then generates the SBOM and `SHA256SUMS`, signs the checksums, and attests the assets. On a release event, the workflow appends the Contributors section after uploading assets.
5. Check the assets, the checksums and the Contributors section before general distribution.

The workflow validates the published tag: it must match `vMAJOR.MINOR.PATCH` with no leading zeros, and a pre-release suffix such as `-rc.1` is refused. A GitHub pre-release of a plain `vX.Y.Z` tag is accepted but is built and published under its exact version only; it never moves `:stable`, `:latest` or the `<major>.<minor>` image tags.

## How releases are prepared

`v0.9.1` through `v0.9.4` were GitHub pre-releases. Stable publication resumed with `v0.9.5`. The earlier `release-as` pin to `0.9.0` dropped the `-rc.5` suffix. Editing a pre-release into a stable release does not trigger the publication workflow; promote by publishing a later version. Keep the generated release PR unmerged until the maintainer authorizes publication. Routine code and configuration PRs are not that authorization.

## Release notes

Release Please generates the changelog section. In the release PR, edit it into the house style: a summary, **Highlights**, then the generated lists. The GitHub release body comes from [`release-notes.template.md`](release-notes.template.md):

```sh
pnpm run release:notes -- v0.9.6                  # print the notes for a tag (or HEAD while the tag does not exist yet)
pnpm run release:notes -- v0.9.6 --from v0.9.4    # override the previous tag
gh release edit v0.9.6 --notes-file <(pnpm --silent run release:notes -- v0.9.6)
```

Write the user-facing summary and highlights in `docs/releases/highlights/<version>.md` (a paragraph, then a list). Without that file the notes list only the grouped commits. The generator always states whether `crates/core/src/migrations.rs` changed.

## Publication approval

Two jobs of the `Release` workflow run in the `release` GitHub environment (required reviewer): `container-approval`, which gates the image push and tags, and `publish`, which gates the asset upload. The Git tag and GitHub Release that Release Please creates when the release PR merges are not gated.

LearnVault no longer publishes release candidates. Changes are verified on `main` before the release PR is merged. The `0.9.0-rc.N` candidates were superseded by `0.9.1` and are pruned under the retention rule in the [release policy](../../.agents/policies/releases.md). The `:rc` container tag is no longer updated; the updater's `rc` channel remains only so existing candidate installations can move to a stable release.

## Canonical SPA Bundling

The `linux` (x86_64) and `linux-arm64` (aarch64) jobs package the `crates/server` binary together with the SPA build (`crates/server/web/build`, from `apps/web-next`) as `web/` inside each tarball. The binary does not embed the SPA; start it with `--web-root web`.

## Rollback

To return to a known-good release, follow the [rollback runbook](../operations/rollback.md). Before every release, record the GitHub latest release and the GHCR alias digests.

## Asset Repair & Manual Triggers

On a GitHub `release` event, Actions runs the workflow definition from the **tagged commit**, not `main`. If a release was tagged before a workflow fix reached `main`, a maintainer can run the workflow by hand:

1. Run the `Release` workflow from `main` (Actions → `Release` → `Run workflow`).
2. Enter the exact release tag (for example `v0.9.6`) in the `tag` input. It must identify an existing, published, non-draft release and match the version metadata in the tagged source.
3. The workflow validates the tag, rebuilds every platform asset from it, checks that the tag has not moved, and uploads the assets with `--clobber`.

The workflow never moves or recreates tags. The contributors step runs only on `release` events, so a manual run does not touch the Contributors section, which `<!-- learnvault:contributors -->` otherwise keeps from being appended twice. Image tags: `stable` and `latest` move only on `release` events, while a manual run of a stable release can still update the `<major>.<minor>` tag.

## Published Release Assets

The current workflow publishes the following eleven assets to GitHub Releases and the OCI image to GHCR (`ghcr.io/andersontavares0/learnvault`):

| Asset | File Name |
| --- | --- |
| Linux Server, x86-64 | `learnvault-linux-amd64.tar.gz` |
| Linux Server, ARM64 | `learnvault-linux-arm64.tar.gz` |
| Linux Desktop, x86-64 (Flatpak bundle) | `learnvault-linux-desktop-amd64.flatpak` |
| Linux Desktop, ARM64 (Flatpak bundle) | `learnvault-linux-desktop-arm64.flatpak` |
| Windows Installer (NSIS) | `learnvault-windows-vMAJOR.MINOR.PATCH.exe` |
| Windows Updater Bundle | `learnvault-windows-vMAJOR.MINOR.PATCH.nsis.zip` |
| Windows Updater Ed25519 Signature | `learnvault-windows-vMAJOR.MINOR.PATCH.nsis.zip.sig` |
| Tauri 2 Updater Manifest | `latest.json` |
| CycloneDX source and lockfile SBOM | `learnvault-vMAJOR.MINOR.PATCH.sbom.cdx.json` |
| SHA-256 Checksums | `SHA256SUMS` |
| Keyless Cosign Bundle for `SHA256SUMS` | `SHA256SUMS.sigstore.json` |

Naming rules:

- Platform packages and the SBOM start with `learnvault-`; `latest.json`, `SHA256SUMS` and `SHA256SUMS.sigstore.json` have fixed names.
- Architectures are `amd64` and `arm64`, never `x86_64`, `x64` or `win64`.
- Windows files carry the version because users download them standalone.
- `SHA256SUMS` is generated with `sha256sum` over every other asset except its own signature bundle.

Release Please reads `.github/release-please/release-please-config.json` and `.github/release-please/release-please-manifest.json`.

Release notes must document database schema migrations. Releases must never contain course files, local databases, secrets, or cache artifacts.

## Container Image Tags and Verification

The image is built for `linux/amd64` and `linux/arm64`, each on its own native runner (no QEMU), and combined into one multi-architecture index. The tags depend on the release:

| Tag | When it is applied |
| --- | --- |
| `<version>` (e.g. `0.9.0`) | Always. Points at the exact release. |
| `<major>.<minor>` (e.g. `0.9`) | Only for stable (not pre-release) releases, including manual runs; moves within the minor line. |
| `stable`, `latest` | Only on stable release events. |

Trivy scans each platform digest for HIGH and CRITICAL findings. The workflow signs and verifies the combined index with keyless Cosign before applying release tags. To fix a build, pin the exact version (see `deploy/README.md`).

Verify an image before deploying it:

```sh
cosign verify ghcr.io/andersontavares0/learnvault:<tag> \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github[.]com/AndersonTavares0/LearnVault/[.]github/workflows/release[.]yml@refs/(tags/v[0-9]+[.][0-9]+[.][0-9]+|heads/main)$'
```

Buildx attaches provenance and the SBOM to each platform digest. Inspect the index with `docker buildx imagetools inspect ghcr.io/andersontavares0/learnvault:<tag>`.

## Verifying Release Binaries

The current workflow publishes a keyless Sigstore bundle for `SHA256SUMS`, a CycloneDX SBOM generated from the tagged source and lockfiles with `trivy fs .`, and GitHub Artifact Attestations. The SBOM lists source and dependency metadata; it is not a scan of the compiled binaries. Build provenance covers every file in the release asset directory. The SBOM attestation covers each Linux server archive, Windows installer and updater bundle, and Flatpak bundle.

Download the assets with `gh release download <tag> --repo AndersonTavares0/LearnVault`, then verify the checksum signature. The release workflow installs Cosign with `sigstore/cosign-installer` v4.1.2 and does not pin a Cosign version. Verifying `SHA256SUMS.sigstore.json` needs Cosign v3 or newer:

```sh
cosign verify-blob SHA256SUMS \
  --bundle SHA256SUMS.sigstore.json \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github[.]com/AndersonTavares0/LearnVault/[.]github/workflows/release[.]yml@refs/(tags/v[0-9]+[.][0-9]+[.][0-9]+|heads/main)$'
```

Verify downloaded assets against the signed manifest:

```sh
sha256sum -c SHA256SUMS --ignore-missing
```

Verify provenance and the CycloneDX SBOM attestation for an asset:

```sh
gh attestation verify learnvault-linux-amd64.tar.gz \
  --repo AndersonTavares0/LearnVault \
  --signer-workflow AndersonTavares0/LearnVault/.github/workflows/release.yml
gh attestation verify learnvault-linux-amd64.tar.gz \
  --repo AndersonTavares0/LearnVault \
  --signer-workflow AndersonTavares0/LearnVault/.github/workflows/release.yml \
  --predicate-type https://cyclonedx.org/bom
```

## Troubleshooting

- **Desktop update is not offered after a release.** The release must be published, not a draft and not a pre-release, and `latest.json` must carry a version above the installed app. Check with `gh release view <tag> --json isDraft,isPrerelease`. The Flatpak never offers updates.
- **The release workflow refuses the tag.** The tag must match `vX.Y.Z` exactly and belong to a published, non-draft release.
- **An OCI verification fails.** Check the digest and the keyless signature from `release.yml`. Re-run the release workflow for the tag instead of editing assets by hand.
- **A desktop verification fails.** Check that `latest.json` and the updater payload carry the matching Tauri signature.
- **The container image has no version tag.** The workflow pushes each architecture by digest, scans and smoke-tests it, then signs and verifies the staged index (`staging-<run id>`, used by no deployment) before applying release tags. If the scan or the signature check fails, no release tag is applied. Staging tags stay in GHCR and can be removed by hand.
- **A platform build fails.** A Windows or native Linux build failure blocks the container approval and the asset upload. A Flatpak build failure blocks the asset upload, while the independent container jobs may still publish to GHCR.
