# Branch Protection & Repository Rulesets

LearnVault protects its default branch (`main`) with **GitHub Repository Rulesets** instead of classic branch protection.

---

## Architecture & Policy

Every change reaches `main` through a pull request that passes the required status checks. Direct pushes, force pushes, and branch deletions are blocked.

### Key Rules Enforced on `main`

| Rule | Enforcement | Rationale |
| :--- | :--- | :--- |
| **No Direct Pushes** | Active (`pull_request`) | Every commit has a PR and CI logs. |
| **No Deletions** | Active (`deletion`) | `main` cannot be deleted by accident. |
| **No Force Pushes** | Active (`non_fast_forward`) | History stays intact and collaborators' work is not overwritten. |
| **Conversation Resolution** | Active (`required_review_thread_resolution`) | Review threads must be resolved before merge. |
| **Dismiss Stale Approvals** | Active (`dismiss_stale_reviews_on_push`) | A new push to an open PR resets its approvals. |
| **Strict Up-To-Date Base** | Active (`strict_required_status_checks_policy`) | A PR branch must be up to date with `main` before merge. |
| **Squash-Only Merges** | Active (`allowed_merge_methods: ["squash"]`) | The ruleset states the merge-method limit as well as the repository settings. |
| **Single-Maintainer Compatibility** | Active (`required_approving_review_count: 0`) | A solo maintainer is not blocked, and every automated check still applies. |
| **Zero Admin Bypass** | Active (`bypass_actors: []`) | Administrators follow the same rules. |

---

## Squash merge policy

The production ruleset export and the active GitHub ruleset allow only squash merges, matching the contribution policy. Editing the JSON file does not change GitHub settings. Maintainers apply and verify remote ruleset changes separately.

## Mandatory Status Checks

The ruleset requires these 7 checks to pass before a pull request can merge (`.github/rulesets/learnvault-main.json`):

1. **`SPA checks`:** `tsc --noEmit`, `eslint --max-warnings 0`, the script tests (`test:scripts`), Vitest, and `pnpm audit`.
2. **`Rust tests and lints`:** `cargo fmt`, Clippy (`-D warnings`), the workspace tests with `nextest`, and the `crates/core` coverage floor.
3. **`Windows core tests`:** formatting, Clippy, and tests for `crates/core` on Windows.
4. **`Desktop crate compile and guards`:** Tauri 2 desktop build, Clippy, and the desktop integration tests.
5. **`Container build (Rust)`:** rootless Containerfile build and a Trivy scan.
6. **`Dependency review`:** GitHub's dependency review on the pull request.
7. **`CI gate`:** confirms the required jobs ran and none was cancelled or unexpectedly skipped.

---

## Templates & Global Reusability

Templates and import instructions for other projects are in [`.github/rulesets/`](../../.github/rulesets/):

- **LearnVault Production Ruleset:** [`.github/rulesets/learnvault-main.json`](../../.github/rulesets/learnvault-main.json)
- **Generic Project Template:** [`.github/rulesets/default-branch.template.json`](../../.github/rulesets/default-branch.template.json)
- **Import and Customization Guide:** [`.github/rulesets/README.md`](../../.github/rulesets/README.md)
