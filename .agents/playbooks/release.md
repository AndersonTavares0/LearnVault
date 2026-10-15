# Release playbook

1. Confirm the release target, migration impact, and rollback notes.
2. Use Conventional Commits and let Release Please prepare the changelog/release PR.
3. Run all relevant native, SPA, and deployment validations before merging the release PR.
4. Verify the published assets, `SHA256SUMS`, release notes, and platform labels.
5. Do not make containers a prerequisite for native executable artifacts.
