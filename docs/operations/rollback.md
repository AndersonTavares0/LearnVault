# Rollback runbook

The `Rollback` workflow (`.github/workflows/rollback.yml`, logic in `scripts/rollback.mjs`) returns the
published state to a known-good release after a bad one. It runs only manually (`workflow_dispatch`) and only from `main`. The `Verify and plan the rollback` job always runs; the `Apply the rollback` job runs only when `dry_run=false`.

Steps, in order:

0. Refuses to plan or apply while any `Release` workflow run is `in_progress`, `queued` or `waiting`, or when that lookup fails.
1. Verifies the known-good release `target_version`: it is published, not a pre-release, its GHCR index resolves, **every platform actually present in that index** resolves (for example `v0.8.0` is amd64-only; arm64 is not required), and the index digest has a valid keyless cosign signature from `release.yml`.
2. Verifies every digest in the recorded alias state the same way.
3. Snapshots the live aliases (`latest`, `stable`, the minor tag of the target and of the bad release) and prints a plan.
4. Only when `dry_run=false` and the `release` environment is approved: re-checks that the registry did not change since the plan, restores each alias to the recorded state, marks the bad release as a pre-release, marks the known-good release as GitHub latest, and confirms `releases/latest` returns it.

It never deletes, moves or recreates a git tag, never changes a `vX.Y.Z` image tag, and never deletes a GitHub release.
Rollback has its own concurrency group (`rollback`, never cancelled); `Release` keeps per-tag groups. Step 0 keeps them apart with separate `gh run list --workflow release.yml --status` calls for `in_progress`, `queued` and `waiting`. A release that starts after that check is not blocked, so do not publish while a rollback runs, and re-run the dry run if anything changed.

## Absent aliases

GHCR cannot untag. An alias that did not exist before the bad release is restored to *absent* by first moving it to a tombstone index (the same images plus an annotation, so a different digest) and then deleting only the version whose tag list is exactly `[<alias>]`. A version that carries any other tag is never deleted. If the alias cannot be detached, the workflow fails before touching GitHub releases.

## Unsigned legacy images (v0.8.0)

`v0.8.0` and older images were published before image signing, so signature verification cannot pass for them. Rolling back to one needs `allow_unsigned_legacy=true`, which the workflow accepts only when all of these hold:

- `target_version` is `v0.8.0` or older;
- `recorded_aliases` also contains the exact version key (for example `"0.8.0": "sha256:..."`) and it equals the live digest of `:0.8.0`;
- the cosign output contains `no signatures found` (case-insensitive) and does not mention expected identities or a certificate; any other error still fails;
- every platform in the index resolves.

The plan prints an `UNSIGNED LEGACY IMAGE` warning. Record the digest from your own pre-release inspection, not from the dry run.

## Before every release: record the state

Run these read-only commands and keep the output in the release PR or issue.

```sh
IMAGE=ghcr.io/andersontavares0/learnvault
TARGET_TAG=v0.8.0   # the known-good release to roll back to
BAD_TAG=v0.9.5      # the release being published
target=${TARGET_TAG#v}; bad=${BAD_TAG#v}
gh api repos/AndersonTavares0/LearnVault/releases/latest --jq .tag_name
gh release list --repo AndersonTavares0/LearnVault --limit 10
for alias in latest stable "${target%.*}" "${bad%.*}"; do
  printf '%s ' "$alias"
  docker buildx imagetools inspect "$IMAGE:$alias" --format '{{.Manifest.Digest}}' 2>&1 | tail -n1
done
printf '%s ' "$target"
docker buildx imagetools inspect "$IMAGE:$target" --format '{{.Manifest.Digest}}' 2>&1 | tail -n1   # exact-version digest of the target
docker buildx imagetools inspect --raw "$IMAGE:$target" | jq '.manifests[].platform'
```

Record the digest of `$IMAGE:$bad` too, once the new release is published and before any rollback.

Write the result as the `recorded_aliases` JSON: a `sha256:` digest if the alias resolved, `null` if the command reported `not found`. The aliases are exactly `latest`, `stable`, the minor tag of `target_version` and the minor tag of `bad_version` (one entry when both share a minor line); a missing or extra key is rejected. The sample below suits a rollback from `v0.9.5` to `v0.8.0`. Use a verified digest, or `null` only for an alias confirmed absent. Add the exact target version as an extra key (`"0.8.0"` here) only for the unsigned-legacy exception, with the recorded target digest as its value. Replace every angle-bracket placeholder with a real value before running a command or submitting the JSON.

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
2. Dispatch it again from `main` with `dry_run=false` and approve the `release` environment. This run makes a fresh plan, and the apply job refuses to continue if any alias differs from that run's snapshot.
3. Verify GitHub latest, the recorded aliases and both exact-version digests. If the target published `latest.json`, also confirm that the download from GitHub latest matches the target version.

   ```sh
   gh api repos/AndersonTavares0/LearnVault/releases/latest --jq .tag_name   # the known-good tag
   curl -fsSL https://github.com/AndersonTavares0/LearnVault/releases/latest/download/latest.json | jq .version
   for alias in latest stable "${target%.*}" "${bad%.*}"; do   # compare with recorded_aliases; null must report not found
     docker buildx imagetools inspect "$IMAGE:$alias" --format '{{.Manifest.Digest}}'
   done
   docker buildx imagetools inspect "$IMAGE:$bad" --format '{{.Manifest.Digest}}'      # unchanged
   docker buildx imagetools inspect "$IMAGE:$target" --format '{{.Manifest.Digest}}'   # unchanged
   ```
4. Follow up with a fix release with a higher patch version; the bad release stays as a pre-release for traceability under the [release policy](../../.agents/policies/releases.md).

If the workflow is unavailable, these commands are a partial manual recovery. They repoint only `latest` and flip the release flags. They do not restore `stable` or the minor tags, remove aliases that did not exist before, check for a running release, or clean up staging. Verify the target first with `cosign verify "$IMAGE@<digest>" --certificate-oidc-issuer https://token.actions.githubusercontent.com --certificate-identity-regexp '^https://github\.com/AndersonTavares0/LearnVault/\.github/workflows/release\.yml@refs/(tags/v[0-9]+\.[0-9]+\.[0-9]+|heads/main)$'`, then:

```sh
docker buildx imagetools create --tag "$IMAGE:latest" "$IMAGE@<known-good-digest>"
gh release edit <bad-tag> --repo AndersonTavares0/LearnVault --prerelease
gh release edit <good-tag> --repo AndersonTavares0/LearnVault --latest
```

## Staging images

The Release workflow pushes `staging-<run id>` before applying release tags. The rollback removes that package version only when its tag list is exactly `[staging-<run id>]`. If the same digest also carries release tags, the cleanup is skipped and nothing is deleted.

## Limits

- The tombstone step (`imagetools create --annotation` to give an alias its own digest, then delete that one version) is covered by the repository tests against a simulated registry in `scripts/rollback.test.mjs`. The script checks that the tombstone digest differs from the target digest and from every digest in the alias snapshot and record. It removes only a version whose tags are exactly `[alias]`. If an alias remains after apply, inspect the package versions before retrying.
- Images published before cosign signing was introduced cannot pass signature verification. Only a target `v0.8.0` or older can be accepted, through `allow_unsigned_legacy` and the conditions in "Unsigned legacy images"; an unsigned image newer than `v0.8.0`, and any other verification failure, fails closed.
- Users who already installed the bad release are not downgraded; publish a fixed higher version.
