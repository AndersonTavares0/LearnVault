# Rollback runbook

The `Rollback` workflow (`.github/workflows/rollback.yml`, logic in `scripts/rollback.mjs`) returns the
published state to a known-good release after a bad one. It runs only manually (`workflow_dispatch`) and only from `main`.

Steps, in order:

0. Refuses to plan or apply while any `Release` workflow run is `in_progress`, `queued` or `waiting`, or when that lookup fails.
1. Verifies the known-good release `target_version`: it is published, not a pre-release, its GHCR index resolves, **every platform actually present in that index** resolves (for example `v0.8.0` is amd64-only; arm64 is not required), and the index digest has a valid keyless cosign signature from `release.yml`.
2. Verifies every digest in the recorded alias state the same way.
3. Snapshots the live aliases (`latest`, `stable`, the minor tag of the target and of the bad release) and prints a plan.
4. Only when `dry_run=false` and the `release` environment is approved: re-checks that the registry did not change since the plan, restores each alias to the recorded state, marks the bad release as a pre-release, marks the known-good release as GitHub latest, and confirms `releases/latest` returns it.

It never deletes, moves or recreates a git tag, never changes a `vX.Y.Z` image tag, and never deletes a GitHub release.
Rollback has its own concurrency group (`rollback`, never cancelled); `Release` keeps per-tag groups. Step 0 keeps them apart with `gh run list --workflow release.yml --status in_progress|queued|waiting`. A release that starts after that check is not blocked, so do not publish while a rollback runs, and re-run the dry run if anything changed.

## Absent aliases

GHCR cannot untag. An alias that did not exist before the bad release is restored to *absent* by first moving it to a tombstone index (the same images plus an annotation, so a different digest) and then deleting only the version whose tag list is exactly `[<alias>]`. A version that carries any other tag is never deleted. If the alias cannot be detached, the workflow fails before touching GitHub releases.

## Unsigned legacy images (v0.8.0)

`v0.8.0` predates image signing. On 2026-11-19 GHCR had no `.sig` tag and no referrer for `ghcr.io/andersontavares0/learnvault:0.8.0` (index `sha256:67a29b09f708c980e1685871dbc71c9a00c345374b6250d4a1ea00fc0156d96b`, amd64 plus an attestation manifest), so signature verification cannot pass. Rolling back to it needs `allow_unsigned_legacy=true`, which the workflow accepts only when all of these hold:

- `target_version` is `v0.8.0` or older;
- `recorded_aliases` also contains the exact version key (for example `"0.8.0": "sha256:..."`) and it equals the live digest of `:0.8.0`;
- cosign reports exactly `no signatures found` (a wrong identity or any other error still fails);
- every platform in the index resolves.

The plan prints an `UNSIGNED LEGACY IMAGE` warning. Record the digest from your own pre-release inspection, not from the dry run.

## Before every release: record the state

Run these read-only commands and keep the output in the release PR or issue.

```sh
IMAGE=ghcr.io/andersontavares0/learnvault
gh api repos/AndersonTavares0/LearnVault/releases/latest --jq .tag_name
gh release list --repo AndersonTavares0/LearnVault --limit 10
for alias in latest stable 0.8 0.9; do   # latest, stable, and the minor tags of the current and next release
  printf '%s ' "$alias"
  docker buildx imagetools inspect "$IMAGE:$alias" --format '{{.Manifest.Digest}}' 2>&1 | tail -n1
done
docker buildx imagetools inspect --raw "$IMAGE:<current-version>" | jq '.manifests[].platform'
```

Write the result as the `recorded_aliases` JSON: the digest if the alias resolved, `null` if the command reported `not found`. Every alias must be listed; a missing key is rejected.

```json
{"latest":"sha256:<digest>","stable":null,"0.8":null,"0.9":null,"0.8.0":"sha256:<digest, only for allow_unsigned_legacy>"}
```

A timeout, an auth error or any other output that is neither a clean digest nor `not found` is uncertain. Repeat the command instead of recording `null`.

## Rolling back

1. Dry run (default). Actions → Rollback → Run workflow with:
   - `target_version`: known-good tag, for example `v0.8.0`
   - `bad_version`: the bad tag, for example `v0.9.5`
   - `recorded_aliases`: the JSON recorded above
   - `staging_run`: optional, the Release run id whose `staging-<run>` image should be removed
   - `allow_unsigned_legacy`: `false`, except for a release `v0.8.0` or older (see "Unsigned legacy images")
   - `dry_run`: `true`

   Read the plan in the job summary: one line per alias (`keep`, `<old> -> <digest>`, or `-> absent (delete)`), the GitHub release changes, and the staging cleanup note. Any failure here (missing signature, unknown platform, uncertain lookup) is a stop: fix or choose another target.
2. Run it again with `dry_run=false` and approve the `release` environment. The apply job refuses to continue if any alias differs from the plan.
3. Verify afterwards (read-only). This step is mandatory because the tombstone mechanism has not been exercised against GHCR (see Limits).

   ```sh
   gh api repos/AndersonTavares0/LearnVault/releases/latest --jq .tag_name   # the known-good tag
   curl -fsSL https://github.com/AndersonTavares0/LearnVault/releases/latest/download/latest.json | jq .version
   docker buildx imagetools inspect "$IMAGE:latest" --format '{{.Manifest.Digest}}'
   ```

   `latest.json` must come from the known-good release. Also confirm that every alias recorded as `null` now reports `not found`, and that every `vX.Y.Z` tag still resolves to its original digest:

   ```sh
   for alias in stable 0.8 0.9; do docker buildx imagetools inspect "$IMAGE:$alias" --format '{{.Manifest.Digest}}'; done
   docker buildx imagetools inspect "$IMAGE:<bad-version>" --format '{{.Manifest.Digest}}'   # unchanged
   ```
4. Follow up with a fix release with a higher patch version; the bad release stays as a pre-release for traceability under the [release policy](../../.agents/policies/releases.md).

The equivalent manual commands, if the workflow is unavailable (verify the target first with `cosign verify "$IMAGE@<digest>" --certificate-oidc-issuer https://token.actions.githubusercontent.com --certificate-identity-regexp '^https://github\.com/AndersonTavares0/LearnVault/\.github/workflows/release\.yml@refs/(tags/v[0-9]+\.[0-9]+\.[0-9]+|heads/main)$'`):

```sh
docker buildx imagetools create --tag "$IMAGE:latest" "$IMAGE@<known-good-digest>"
gh release edit <bad-tag> --repo AndersonTavares0/LearnVault --prerelease
gh release edit <good-tag> --repo AndersonTavares0/LearnVault --latest
```

## Staging images

The Release workflow pushes `staging-<run id>` before applying release tags. The rollback removes that package version only when its tag list is exactly `[staging-<run id>]`. If the same digest also carries release tags, the cleanup is skipped and nothing is deleted.

## Limits

- The tombstone (`imagetools create --annotation` to give an alias its own digest, then delete that one version) was not tested against GHCR, because review allows no mutation. The script refuses to delete unless the new digest differs from every known digest, and it removes only a version whose tags are exactly `[alias]`. The post-apply check above is the real confirmation. If an alias remains, inspect the package versions before retrying.
- Images published before cosign signing was introduced cannot pass signature verification. Only a target `v0.8.0` or older can be accepted, through `allow_unsigned_legacy` and the conditions in "Unsigned legacy images"; an unsigned image newer than `v0.8.0`, and any other verification failure, fails closed.
- Users who already installed the bad release are not downgraded; publish a fixed higher version.
