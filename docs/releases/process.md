# Release process

LearnVault uses Semantic Versioning: `MAJOR.MINOR.PATCH`.

1. Merge approved changes into `main`.
2. Update `CHANGELOG.md` and move the relevant entries from `Unreleased` into the version section.
3. Create and push an annotated tag such as `v0.2.0`.
4. GitHub Actions runs the release workflow.
5. The workflow builds application artifacts and creates the GitHub Release with generated notes.
6. Verify the artifact in a clean environment before homelab deployment.

Database migrations must be documented in release notes. Releases must never contain the physical course library, local databases, credentials, or cache files.
