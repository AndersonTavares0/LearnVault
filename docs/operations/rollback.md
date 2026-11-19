# Rollback runbook

The `Rollback` workflow (`.github/workflows/rollback.yml`, logic in `scripts/rollback.mjs`) returns the
published state to a known-good release after a bad release. It is manual (`workflow_dispatch`, from `main` only).

What it does, in order:

1. Verifies the known-good release `target_version`: it is published, not a pre-release, its GHCR index resolves, **every platform actually present in that index** resolves (for example `v0.8.0` is amd64-only; arm64 is not required), and the index digest has a valid keyless cosign signature from `release.yml`.
2. Verifies every digest in the recorded alias state the same way.
3. Snapshots the live aliases (`latest`, `stable`, the minor tag of the target and of the bad release) and prints a plan.
4. Only when `dry_run=false` and the `release` environment is approved: re-checks that the registry did not change since the plan, restores each alias to the recorded state, marks the bad release as a pre-release, marks the known-good release as GitHub latest, and confirms `releases/latest` returns it.

It never deletes, moves or recreates a git tag, never changes a `vX.Y.Z` image tag, and never deletes a GitHub release.
It shares the concurrency group `release-pipeline` with the `Release` workflow, so the two never run together; nothing is cancelled, runs queue (GitHub keeps one pending run per group, so check the Actions tab after queueing).

## Absent aliases

GHCR cannot untag. An alias that did not exist before the bad release is restored to *absent* by first moving it to a tombstone index (the same images plus an annotation, so a different digest) and then deleting only the version whose tag list is exactly `[<alias>]`. A version that carries any other tag is never deleted. If the alias cannot be detached, the workflow fails before touching GitHub releases.

## Before every release: record the state

Run these and keep the output in the release PR or issue. They are read-only.

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
{"latest":"sha256:<digest>","stable":null,"0.8":null,"0.9":null}
```

Anything other than a clean digest or a clear `not found` (timeouts, auth errors) is uncertain: repeat the command instead of recording `null`.

## Rolling back

1. Dry run (default). Actions → Rollback → Run workflow with:
   - `target_version`: known-good tag, for example `v0.8.0`
   - `bad_version`: the bad tag, for example `v0.9.5`
   - `recorded_aliases`: the JSON recorded above
   - `staging_run`: optional, the Release run id whose `staging-<run>` image should be removed
   - `dry_run`: `true`

   Read the plan in the job summary: one line per alias (`keep`, `<old> -> <digest>`, or `-> absent (delete)`), the GitHub release changes, and the staging cleanup note. Any failure here (missing signature, unknown platform, uncertain lookup) is a stop: fix or choose another target.
2. Run it again with `dry_run=false` and approve the `release` environment. The apply job refuses to continue if any alias differs from the plan.
3. Verify afterwards (read-only):

   ```sh
   gh api repos/AndersonTavares0/LearnVault/releases/latest --jq .tag_name   # the known-good tag
   curl -fsSL https://github.com/AndersonTavares0/LearnVault/releases/latest/download/latest.json | jq .version
   docker buildx imagetools inspect "$IMAGE:latest" --format '{{.Manifest.Digest}}'
   ```

   `latest.json` must come from the known-good release.
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

- Images published before cosign signing was introduced cannot be verified; the workflow then fails closed and the alias change must be a maintainer decision made outside the workflow.
- Users who already installed the bad release are not downgraded; publish a fixed higher version.
