# Release process

LearnVault uses Semantic Versioning: `MAJOR.MINOR.PATCH`.

1. Merge approved Conventional Commit changes into `main`.
2. Release Please opens or updates a release PR with the version and `CHANGELOG.md` entries.
3. Merge the Release Please PR; it creates the semantic tag and GitHub Release.
4. The published-release workflow builds Linux artifacts, injects the tag version, and uploads `SHA256SUMS`.
5. Verify the release assets and checksums before distribution.

Release Please is the sole owner of `CHANGELOG.md` and release notes. Do not use
GitHub's generated notes as a second source. Use Conventional Commit prefixes such as
`feat:`, `fix:`, `docs:`, and `chore:` so release notes and version bumps remain predictable.
The workflow requires the `RELEASE_PLEASE_TOKEN` repository secret; keep release
automation credentials in GitHub Actions secrets, never in tracked files. The token
must be allowed to open release PRs and publish tags/releases so the artifact workflow
can react to the published release.

Database migrations must be documented in release notes. Releases must never contain the physical course library, local databases, credentials, or cache files.
