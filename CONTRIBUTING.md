# Contributing to LearnVault

Thank you for helping improve LearnVault. Contributions should preserve the project's focus: a small, maintainable, self-hosted study library that never modifies the source course library.

## Before opening a pull request

1. Search existing issues and pull requests.
2. For a feature that affects data, security, deployment, or UX architecture, open an issue first.
3. Keep the change focused and explain the user-visible behavior.
4. Never include course files, credentials, private homelab addresses, database files, or generated media.

## Development checks

```sh
npm ci
npm run check
npm run build
npm audit --audit-level=low
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
described in [docs/governance.md](docs/governance.md); correct any drift before merging.

## Commit style

Use short Conventional Commit-style messages: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, or `chore:`.

## Clean-room policy

Existing course platforms may be studied for general product patterns, but do not copy their code, assets, branding, proprietary text, or pixel-level layouts. New UI must have LearnVault's own visual identity.
