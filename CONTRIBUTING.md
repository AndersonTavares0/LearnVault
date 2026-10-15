# Contributing to LearnVault

Thank you for helping improve LearnVault. Contributions should preserve the project's focus: a small, maintainable, self-hosted study library that never modifies the source course library.

## Before opening a pull request

1. Search existing issues and pull requests.
2. For a feature that affects data, security, deployment, or UX architecture, open an issue first.
3. Keep the change focused and explain the user-visible behavior.
4. Never include course files, credentials, private homelab addresses, database files, or generated media.

## Development checks

```sh
pnpm install --frozen-lockfile
pnpm run check
pnpm run build
pnpm audit --audit-level=low
go test ./...
go vet ./...
```

Tests must use fixtures and temporary directories; they must not access `/mnt/cursos`.

## Pull requests

- Use a clear title such as `feat: add lesson progress persistence`.
- Describe the problem, solution, and validation performed.
- Include screenshots for UI changes.
- Document migrations and deployment changes.
- Add or update tests for behavior changes.

All changes to `main` require a pull request and passing required checks. Direct pushes
and force-pushes are disabled. The repository settings and enforceable policy are
described in [docs/governance/overview.md](docs/governance/overview.md); correct any drift before merging.

## Issue → branch → pull request → merge

For collaborators such as `WesleyQDev`:

1. Start from a GitHub Issue using the bug or feature form.
2. Create a branch from `main`, for example `feat/lesson-progress` or `fix/catalog-scan`.
3. Make the change and push the branch to this repository.
4. Open a pull request and reference the Issue with `Closes #<number>` when appropriate.
5. Wait for CI, address review comments, and keep the branch focused.
6. Obtain the required review, resolve conversations, and keep all required checks green.
7. Merge through GitHub using squash merge; never push directly to `main`.

This private, user-owned repository uses collaborator branches instead of private forks.
If a fork-based workflow becomes necessary, the repository must first move to an
organization that permits private forks or become public after a separate review.

## Commit style

Use short Conventional Commit-style messages: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, or `chore:`.
Release Please uses these messages to prepare version bumps and changelog entries.

## Clean-room policy

Existing course platforms may be studied for general product patterns, but do not copy their code, assets, branding, proprietary text, or pixel-level layouts. New UI must have LearnVault's own visual identity.
