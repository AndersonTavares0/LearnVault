# Branch Protection & Repository Rulesets

LearnVault enforces branch protection on its default branch (`main`) using **GitHub Repository Rulesets**, superseding legacy branch protection.

---

## Architecture & Policy

All changes to the codebase enter `main` exclusively through pull requests that pass all required status checks. Direct pushes, force pushes, and branch deletions are permanently blocked.

### Key Rules Enforced on `main`

| Rule | Enforcement | Rationale |
| :--- | :--- | :--- |
| **No Direct Pushes** | Active (`pull_request`) | Prevents accidental commits to production without PR review and CI logs. |
| **No Deletions** | Active (`deletion`) | Prevents accidental deletion of the `main` branch. |
| **No Force Pushes** | Active (`non_fast_forward`) | Preserves immutable commit history and avoids overwriting collaborator work. |
| **Conversation Resolution** | Active (`required_review_thread_resolution`) | Guarantees all discussion threads, questions, and code review remarks are resolved before merge. |
| **Dismiss Stale Approvals** | Active (`dismiss_stale_reviews_on_push`) | Resets approvals when new commits are added to an open PR. |
| **Strict Up-To-Date Base** | Active (`strict_required_status_checks_policy`) | Enforces that PR branches must be rebased or updated against current `main` before merge. |
| **Squash-Only Merges** | Active (`allowed_merge_methods: ["squash"]`) | Keeps the effective merge-method restriction explicit in the ruleset as well as repository settings. |
| **Single-Maintainer Compatibility** | Active (`required_approving_review_count: 0`) | Avoids blocking the solo maintainer while retaining all automated CI quality gates and PR verification. |
| **Zero Admin Bypass** | Active (`bypass_actors: []`) | Administrators are subject to the same protections, eliminating backdoors or unverified merges. |

---

## Squash merge policy

The production ruleset export and active GitHub ruleset allow only squash merges, matching the contribution policy. Changes to this JSON file do not update GitHub settings; maintainers must apply and verify remote ruleset changes separately.

## Mandatory Status Checks

The ruleset enforces that the following 5 CI checks must pass before any pull request can be merged:

1. **`SPA checks`:** SvelteKit type checks (`svelte-check`), local scripts tests (`test:scripts`), Vitest unit tests, and dependency audits.
2. **`Rust tests and lints`:** Cargo format checks, Clippy lints (`-D warnings`), and the workspace test suite via `nextest`.
3. **`Desktop crate compile and guards`:** Tauri 2 desktop compilation, Clippy, and desktop integration tests with native libraries.
4. **`Container build (Rust)`:** Rootless Containerfile build verification and Trivy vulnerability scanner.
5. **`CI gate`:** Final gate evaluation ensuring all required checks executed and were not cancelled or unexpectedly skipped.

---

## Templates & Global Reusability

Configuration templates and instructions for importing this model into other projects are maintained in [`.github/rulesets/`](../../.github/rulesets/):

- **LearnVault Production Ruleset:** [`.github/rulesets/learnvault-main.json`](../../.github/rulesets/learnvault-main.json)
- **Generic Project Template:** [`.github/rulesets/default-branch.template.json`](../../.github/rulesets/default-branch.template.json)
- **Import and Customization Guide:** [`.github/rulesets/README.md`](../../.github/rulesets/README.md)
