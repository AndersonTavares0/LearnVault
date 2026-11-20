# Release Process

LearnVault uses Semantic Versioning (`MAJOR.MINOR.PATCH`). The [release policy](../../.agents/policies/releases.md) holds the rules for version numbers, pacing, pre-release retention and release notes, and [ADR 0006](../architecture/adr/0006-release-versioning-and-prerelease-retention.md) gives the reasons.

Before `1.0.0`, Release Please bumps the patch version for `feat:`, `fix:` and `perf:` commits and the minor version for breaking changes. From `1.0.0`, breaking changes bump the major version. Tags and release names are `vMAJOR.MINOR.PATCH`, and the changelog and release notes are generated from English Conventional Commits.

1. Merge approved changes using Conventional Commits into `main`; these changes accumulate for the next monthly release.
2. At 12:00 UTC on the first day of each month, Release Please opens or updates one release PR with the version files and the generated `docs/releases/CHANGELOG.md` section.
3. Merging the release PR creates the tag and the GitHub Release; ordinary code PRs never publish. For an urgent security release outside the schedule, run the `Release Please` workflow manually on `main` to prepare the release PR early.
4. The release workflow builds the Linux server tarballs, the Flatpak bundles ([`flatpak.md`](../operations/flatpak.md)), the Windows NSIS installer and the container image. It then generates the SBOM and `SHA256SUMS`, signs the checksums, attests the assets, and appends the contributors list.
5. Check the assets, the checksums and the Contributors section before general distribution.

The workflow validates the published tag: it must match `vMAJOR.MINOR.PATCH` with no leading zeros, and a pre-release suffix such as `-rc.1` is refused. A GitHub pre-release of a plain `vX.Y.Z` tag is accepted. It is built and published under its exact version only and never moves `:stable`, `:latest` or the `<major>.<minor>` image tags.

## How releases are prepared

Release Please runs with `versioning: default` and `prerelease: false`, so every release is a normal GitHub release. It becomes `releases/latest`, the desktop updater serves it, and it moves the `latest`, `stable` and minor-line container tags. Releases up to and including `0.9.4` were published as pre-releases; the policy records the change. Preparing the release PR updates the version files and the English changelog and publishes nothing. Merging it creates the `vX.Y.Z` tag and the GitHub Release. The asset and container publication jobs then wait for approval in the `release` environment, which does not gate the tag or the GitHub Release.

`0.9.0` was pinned once with `release-as` to leave `0.9.0-rc.5` behind without carrying its suffix; the override was removed in the release PR that published it. It is a GitHub pre-release, so the desktop updater (which reads `releases/latest`) does not offer it and the image has only its `0.9.0` tag. Editing a pre-release into a stable release does not trigger the publication workflow; publish a later non-pre-release instead. Keep a generated release PR unmerged until the final review and the maintainer's explicit authorization to publish. Routine code and configuration PRs are not that authorization.

## Release notes

Release Please generates the changelog section. In the release PR, edit it into the house style: a summary, **Highlights**, then the generated lists. The GitHub release body comes from [`release-notes.template.md`](release-notes.template.md):

```sh
pnpm run release:notes -- v0.9.6                  # print the notes for a tag (or HEAD while the tag does not exist yet)
pnpm run release:notes -- v0.9.6 --from v0.9.4    # override the previous tag
gh release edit v0.9.6 --notes-file <(pnpm --silent run release:notes -- v0.9.6)
```

Write the user-facing summary and highlights in `docs/releases/highlights/<version>.md` (a paragraph, then a list). Without that file the notes list only the grouped commits. The generator always states whether `crates/core/src/migrations.rs` changed.

## Publication approval

The container and asset publication jobs wait for approval in the `release` GitHub environment (required reviewer). The gate covers container tags and asset uploads, not the Git tag or GitHub Release that Release Please creates when the release PR merges.

LearnVault no longer publishes release candidates. Changes are verified on `main` before the release PR is merged. The `0.9.0-rc.N` candidates were superseded by `0.9.1` and are pruned under the retention rule in the [release policy](../../.agents/policies/releases.md). The `:rc` container tag is no longer updated; the updater's `rc` channel remains only so existing candidate installations can move to a stable release.

## Canonical SPA Bundling

The server binary embeds the SPA from `crates/server/web/build`, the output of the web build (`apps/web-next`). The `linux` (x86_64) and `linux-arm64` (aarch64) jobs in the release workflow compile `crates/server` with those assets.

## Rollback

To return to a known-good release, follow the [rollback runbook](../operations/rollback.md). Before every release, record the GitHub latest release and the GHCR alias digests.

## Asset Repair & Manual Triggers

On a GitHub `release` event, Actions runs the workflow definition from the **tagged commit**, not `main`. If a release was tagged before a workflow fix reached `main`, a maintainer can run the workflow by hand:

1. Open Actions → `Release` → `Run workflow`.
2. Enter the exact tag (for example `v0.9.6`).
3. The workflow validates the tag, confirms or creates the release (drafts are supported for atomic publishing), rebuilds every platform asset from that tag, checks that the tag has not moved, uploads the assets with `--clobber`, publishes the release, and appends the Contributors section.

The workflow never moves or recreates tags. The `<!-- learnvault:contributors -->` marker keeps the Contributors section from being appended twice.

## Published Release Assets

Each release publishes eleven files on GitHub Releases, plus the OCI image on GHCR (`ghcr.io/andersontavares0/learnvault`):

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

- The prefix is always `learnvault-`.
- Architectures are `amd64` and `arm64`, never `x86_64`, `x64` or `win64`.
- Windows files carry the version because users download them standalone.
- `SHA256SUMS` is generated with `sha256sum` over every other asset except its own signature bundle.

Release Please is the only source of `docs/releases/CHANGELOG.md`; write Conventional Commits in English. After the assets upload, the release workflow appends the Contributors section (Dependabot excluded). Release Please reads `.github/release-please/release-please-config.json` and `.github/release-please/release-please-manifest.json`.

Release notes must document database schema migrations. Releases must never contain course files, local databases, secrets, or cache artifacts.

## Container Image Tags and Verification

The image is built for `linux/amd64` and `linux/arm64`, each on its own native runner (no QEMU), and combined into one multi-architecture index. The tags applied depend on the release:

| Tag | When it is applied |
| --- | --- |
| `<version>` (e.g. `0.9.0`) | Always. Points at the exact release. |
| `<major>.<minor>` (e.g. `0.9`) | Only for stable (not pre-release) releases; moves within the minor line. |
| `stable`, `latest` | Only for stable (not pre-release) releases. |

Every tag points at a digest that was scanned for HIGH and CRITICAL vulnerabilities, signed with keyless cosign, and verified before tagging. To fix a build, pin the exact version (see `deploy/README.md`).

Verify an image before deploying it:

```sh
cosign verify ghcr.io/andersontavares0/learnvault:<tag> \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  --certificate-identity-regexp '^https://github[.]com/AndersonTavares0/LearnVault/[.]github/workflows/release[.]yml@refs/(tags/v[0-9]+[.][0-9]+[.][0-9]+|heads/main)$'
```

Provenance and the SBOM are attached to the same digest; inspect them with `docker buildx imagetools inspect ghcr.io/andersontavares0/learnvault:<tag>`.

## Verifying Release Binaries

Each release publishes a keyless Sigstore bundle for `SHA256SUMS`, a CycloneDX SBOM generated from the tagged source and lockfiles with `trivy fs .`, and GitHub Artifact Attestations. The SBOM lists source and dependency metadata; it is not a scan of the compiled binaries. Build provenance covers every file in the release asset directory. The SBOM attestation covers each Linux server archive, Windows installer and updater bundle, and Flatpak bundle.

Download the assets with `gh release download <tag> --repo AndersonTavares0/LearnVault`, then verify the checksum signature with Cosign v3 or newer. Cosign 2.x cannot verify the bundle, which stores the certificate in a format that version does not expect:

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
- **Signature verification fails in the updater or in `update.sh`.** The image or update file was not signed by this repository's release workflow on a release tag or `main`. Re-run the release workflow for the tag instead of editing assets by hand.
- **The container image has no version tag.** Each architecture is pushed by digest and scanned before anything names it. The combined index gets a temporary `staging-<run id>` tag, which no deployment uses, and release tags are applied only after the signature check passes. If the scan fails, no release tag is applied. Staging tags stay in GHCR and can be removed by hand.
- **A platform build fails.** The Windows, Linux and Flatpak jobs all run before the publish job, so a failure in any of them publishes nothing. Check that job's log.
