# Release playbook

1. Use Conventional Commit SemVer bumps in `.github/release-please/release-please-config.json`: before `1.0.0`, `feat:` increments minor, `fix:` and `perf:` increment patch, and breaking changes increment minor. From `1.0.0`, breaking changes increment major.
2. Confirm the release target, database migration impact, and rollback notes before merging.
3. Use English Conventional Commits (`feat:`, `fix:`, `perf:`, `docs:`, `chore:`). Release Please generates the `vMAJOR.MINOR.PATCH` tag/release name and changelog automatically.
4. Run all relevant native, SPA, E2E, and deployment validations before merging the release PR.
5. Verify published assets, `SHA256SUMS`, release notes, and platform labels after the release workflow publishes.
6. Do not make containers a prerequisite for native executable artifacts.
7. Treat the Release Please PR as the publication gate: merge it when the accumulated changes are ready to ship; merging ordinary code PRs does not publish a release.
