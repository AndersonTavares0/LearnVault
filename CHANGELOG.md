# Changelog

All notable changes to LearnVault will be documented here.

The format follows Keep a Changelog and releases use Semantic Versioning.

## [Unreleased]

### Added

- Initial project governance and contribution documentation.
- Repository policies, playbooks, checklists, and ADR template under `.agents/`.
- Canonical documentation layout and controlled Wiki synchronization workflow.

### Planned

- Study-focused player and progress persistence.
- Course curation, thumbnails, search, notes, and favorites.

### Changed

- Standardized the repository around Go 1.25, Node.js 24, pnpm 11.28.5, and a frozen pnpm lockfile.
- Kept generated SPA output, runtime data, caches, secrets, and local tooling out of Git.

### Security

- Preserved the external course library as read-only and excluded it from source control,
  tests, and backups.
