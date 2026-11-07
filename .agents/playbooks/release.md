# Release playbook

1. Pre-major versions (< 1.0.0) follow paced patch increments via `.github/release-please/release-please-config.json` (`"bump-minor-pre-major": true`, `"bump-patch-for-minor-pre-major": true`). Regular `feat:` commits increment patch releases (`0.8.1`, `0.8.2`) to prevent premature minor version skips.
2. Confirm the release target, database migration impact, and rollback notes before merging.
3. Use Conventional Commits (`feat:`, `fix:`, `docs:`, `chore:`) in English, allowing Release Please to assemble the changelog and release PR automatically.
4. Run all relevant native, SPA, E2E, and deployment validations before merging the release PR.
5. Verify published assets, `SHA256SUMS`, release notes, and platform labels after the release workflow publishes.
6. Do not make containers a prerequisite for native executable artifacts.
