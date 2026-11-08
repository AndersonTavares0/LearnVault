# GitHub Repository Rulesets Guide & Templates

This directory provides version-controlled configuration templates for GitHub Repository Rulesets.

Rulesets supersede classic Branch Protection by providing:
- **Dynamic Targeting:** Automatically targets `~DEFAULT_BRANCH` without breaking if the default branch changes.
- **Multiple Layers:** Rulesets can layer without conflicting.
- **Enforcement Modes:** Test changes safely with `evaluate` mode before setting them to `active`.
- **Export/Import Portability:** Easily share and sync protection rules across repositories via JSON.

---

## Included Files

| File | Purpose |
| :--- | :--- |
| [`learnvault-main.json`](learnvault-main.json) | The exact, production-ready ruleset for LearnVault enforcing the 5 core CI status checks. |
| [`default-branch.template.json`](default-branch.template.json) | A clean, generic baseline ruleset ready to be imported into any GitHub repository. |

---

## Rule Definitions & Rationale

Both templates enforce the following invariants:

1. **Deletion (`deletion`):**
   Prevents anyone from deleting the default branch.
2. **Non-Fast-Forward (`non_fast_forward`):**
   Blocks force pushes (`git push --force` or `--force-with-lease`).
3. **Pull Request Requirement (`pull_request`):**
   - **Require Pull Request before merging:** Direct pushes to the default branch are blocked.
   - **`required_approving_review_count: 0`:** Configured for single-maintainer repositories. On GitHub, a maintainer cannot approve their own pull request; setting this to 0 prevents solo developers from being locked out while still enforcing the PR workflow, branch-guard plugins, and CI checks.
   - **`dismiss_stale_reviews_on_push: true`:** Invalidates prior approvals when new commits are pushed.
   - **`required_review_thread_resolution: true`:** Requires all conversations and review threads to be explicitly resolved before merging.
4. **Strict Status Checks (`required_status_checks`):**
   - **`strict_required_status_checks_policy: true`:** Requires branches to be strictly up-to-date with the default branch before merging ("Require branches to be up to date before merging").
   - In `learnvault-main.json`, enforces the 5 mandatory jobs: `SPA checks`, `Rust tests and lints`, `Desktop crate compile and guards`, `Container build (Rust)`, and `CI gate`.
   - In `default-branch.template.json`, the `required_status_checks` list is empty (`[]`) so you can populate it with your repository's workflow jobs.
5. **No Unaudited Admin Bypasses (`bypass_actors: []`):**
   Admins and maintainers are subject to the same protections as contributors, preventing accidental force-pushes or direct commits without a PR.

---

## How to Import in Other Repositories

### Method A: Via GitHub Web UI (Recommended)

1. Open your repository on GitHub.
2. Navigate to **Settings** → **Rules** → **Rulesets**.
3. Click the **New ruleset** dropdown button (top right) and select **Import a ruleset**.
4. Select `default-branch.template.json` from your local machine.
5. Review the imported rules:
   - If your project has a team of reviewers, adjust **Required approvals** from `0` to `1` or more.
   - Under **Require status checks to pass**, click **Add checks** and search for the specific CI jobs of your repository (e.g., `test`, `lint`, `build`).
6. Set **Enforcement status** to **Active** (or **Evaluate** to test for a few days).
7. Click **Create** (or **Save changes**).

### Method B: Via GitHub CLI (`gh api`)

You can create or update a ruleset directly from your terminal:

```sh
# 1. Inspect current branch protection before migrating
gh api repos/<OWNER>/<REPO>/branches/main/protection

# 2. Create the ruleset using the template
gh api --method POST repos/<OWNER>/<REPO>/rulesets \
  --input .github/rulesets/default-branch.template.json

# 3. Verify effective branch rules
gh api repos/<OWNER>/<REPO>/rules/branches/main
```

---

## Adapting for Team Repositories

For multi-person teams or enterprise organizations, adjust `pull_request.parameters` in the JSON:

```json
{
  "type": "pull_request",
  "parameters": {
    "required_approving_review_count": 1,
    "dismiss_stale_reviews_on_push": true,
    "require_code_owner_review": true,
    "require_last_push_approval": true,
    "required_review_thread_resolution": true
  }
}
```

---

## GitHub Plan Limitations

Keep in mind GitHub's feature availability when applying rulesets to other repositories:

- **Public Repositories:** Repository Rulesets are **100% free** on all public repositories (GitHub Free, Team, and Enterprise).
- **Private Repositories:**
  - **GitHub Free:** Repository Rulesets are **not** available for private repositories under Free user/organization plans. Use classic branch protection or upgrade.
  - **GitHub Team:** Repository Rulesets are fully supported for private repositories.
  - **GitHub Enterprise Cloud / Server:** Full support, including organization-wide and enterprise-wide rulesets that cascade across multiple repositories.

---

## Safe Migration Checklist (Zero Downtime)

When replacing classic Branch Protection with Rulesets:

1. **Verify Existing Checks:** Read existing branch protection rules using `gh api repos/<OWNER>/<REPO>/branches/<BRANCH>/protection`.
2. **Apply Ruleset:** Create the new ruleset in **Active** mode using `learnvault-main.json`.
3. **Verify Combined Enforcement:** Confirm the ruleset appears under `gh api repos/<OWNER>/<REPO>/rules/branches/<BRANCH>`.
4. **Decommission Classic Protection:** Once the ruleset is active and verified, remove the legacy branch protection using `gh api --method DELETE repos/<OWNER>/<REPO>/branches/<BRANCH>/protection` to eliminate duplicate rule evaluation.
5. **Final Audit:** Test PR creation and verify that checks, conversation resolution, and force push blocks are actively enforced.
