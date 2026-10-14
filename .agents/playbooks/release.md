# Release playbook

1. Confirm the release target, changelog entry, migration impact, and rollback notes.
2. Run all relevant native, SPA, and deployment validations before creating a tag.
3. Create only a semantic `v*.*.*` tag from reviewed `main`; the release workflow is tag-only.
4. Verify published assets, checksums, release notes, and platform labels after completion.
5. Do not make containers a prerequisite for native executable artifacts.
