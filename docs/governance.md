# Repository governance

## Required workflow

1. Open an issue using the Bug Report or Feature Request form before non-trivial work.
2. Create a focused branch from `main`.
3. Open a pull request using the repository template.
4. Pass the required CI checks and obtain the required review.
5. Resolve all conversations and squash-merge through GitHub.

Direct pushes, force-pushes, and branch deletion on `main` are not allowed. Branch
protection is configured on GitHub and this document records the project rule.

## Template policy

- Blank issues are disabled.
- Bugs use `.github/ISSUE_TEMPLATE/bug.yml`.
- Features use `.github/ISSUE_TEMPLATE/feature.yml`.
- Security reports use the private security policy, never a public issue.
- Pull requests use `.github/pull_request_template.md`.

## Repository boundaries

- Never commit course media, credentials, private homelab details, databases, or generated output.
- `/mnt/cursos` is external read-only data and must not be accessed by automated tests.
- Changes affecting security, storage, deployment, migrations, or UX architecture require explicit review.
- Clean-room research may inform product behavior, but not copied code, assets, branding, text, or layouts.

## Ownership and releases

`.github/CODEOWNERS` defines review ownership. Releases are tag-driven through the release
workflow, with `CHANGELOG.md` and `docs/releases.md` updated as part of the change.

## Dependency updates and CI cost

Dependabot groups compatible npm, Go-module, and GitHub Actions patch/minor updates;
major updates remain individual so peer incompatibilities are visible. Updates are
staggered weekly (Actions monthly), with a small open-PR cap and cooldown for routine
updates. Security alerts remain eligible for immediate updates and must not be merged
until their compatibility and required checks are reviewed.

The SPA CI job owns the dependency install, checks, and build. Its short-lived build
artifact is reused by Go tests, including both current and planned executable output
paths. The container build remains a separate deployment smoke test and uses the
GitHub Actions build cache. Concurrent outdated runs are cancelled. Required check
names remain stable for branch protection.
