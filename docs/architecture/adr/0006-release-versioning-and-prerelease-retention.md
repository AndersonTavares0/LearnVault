# ADR: Release versioning, pacing and pre-release retention

## Status

Accepted (2026-11-17).

## Context

LearnVault published 18 releases between 2026-10-14 (`v0.1.0`) and 2026-11-16 (`v0.9.3`), 33 days. Six of them are
`v0.9.0-rc.N` candidates and a draft, and five numbered releases (`v0.3.0` to `v0.6.1`) share one day. Early on, the release
configuration let feature commits increment the minor, which is how the line reached `0.8` in three weeks; a later fix
(`keep pre-major feature bumps on patch versions`) changed that, so that before `1.0.0` `feat:`, `fix:` and `perf:` increment
the patch. The recent speed comes from off-cycle releases (`0.9.1`, `0.9.2` and `0.9.3` within two days), not from the bump rule.

Problems found while reviewing the history:

- `docs/releases/CHANGELOG.md` carried the whole Go-to-Rust history again inside `0.9.1` (234 items), and linked to a
  `v0.9.0` tag that does not exist.
- The notes of the five `0.9.0-rc.N` releases say "Superseded by v0.9.0", which is also a dead link.
- The release process said the `0.9.0-rc.N` tags are "immutable history", with no rule for when a pre-release may be removed.
- There was no written rule for pacing, for lowering or reusing a version, or for the notes format.

The project has few users (each release has between 2 and 14 downloads, one star, no forks), which made a full reset tempting:
delete every release and restart at `0.1.0`.

## Decision

1. The version line only moves forward. The project continues from `0.9.3` and does not restart at `0.1.0`, and it does not
   adopt four-part versions.
2. Numbered releases are permanent. Release candidates and drafts are temporary and may be removed once a numbered release
   supersedes them and the changelog records what they carried.
3. Releases are paced by the monthly Release Please cycle. Off-cycle releases are limited to security fixes, data-integrity
   fixes and defects in the release or install path.
4. The changelog and release notes follow one house style, generated from a template, with hand-written highlights.
5. These rules are recorded in [`.agents/policies/releases.md`](../../../.agents/policies/releases.md).

### Alternatives considered

| Alternative | Why it was rejected |
| --- | --- |
| Delete every release and restart at `0.1.0` | SemVer item 3 forbids changing a released version, and a lower number sorts below every installed `0.9.x`, so those installs would never be offered the update. The tags that were removed would also break links. Reaching `0.9` fast is not a SemVer problem: the specification calls `0.y.z` the phase for rapid development. |
| Delete all numbered releases but keep numbering | Same broken links and lost binaries as above, with no gain: a clear changelog solves the confusion without deleting anything. |
| Move `v0.9.3` forward to include later commits | A published tag is not moved. A consumer that already fetched it would keep a different commit. GitHub's immutable releases lock exactly this. |
| Declare `1.0.0` now | The stability criteria in the policy are not met yet. |

Research consulted: the SemVer 2.0.0 specification (items 2, 3 and 4 and the FAQ), GitHub's documentation on immutable
releases, the Stack Overflow and DevOps Stack Exchange discussions on deleting tags (pre-release tags can be removed under a
retention rule; tags that reached users should stay), and the Release Please guidance on bootstrapping a version.

## Consequences

- The next release is `0.9.4`, computed by Release Please. No renumbering is needed.
- The five `v0.9.0-rc.N` releases, their tags and the unpublished draft `rust-rc-v0.4.0-rc.1` are removed after this change
  merges. Their changes are described in the `0.9.1` section of the changelog, whose compare link starts at `v0.8.0`.
- The release process document no longer calls the candidate tags immutable and points to the policy.
- All `0.y.z` releases remain GitHub pre-releases, so `releases/latest` still resolves to `v0.8.0`. Fixing that is a separate
  decision (see the policy); it is not changed here.
- Unpublished container and asset work for `v0.9.3` is not affected by this decision.

## Validation and rollback

- `pnpm run test:scripts` covers the release notes generator.
- Before any release or tag is removed, the release bodies, tag commits and asset names are saved outside the repository.
- Removed tags can be recreated from the saved commits. Removed binaries cannot be rebuilt from current workflows with
  certainty, which is why only candidates and drafts are in scope.
- The decision can be revisited by superseding this ADR.
