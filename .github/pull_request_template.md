## Summary

<!-- Explain the user/problem impact in a few sentences. Replace this comment; do not leave sections blank. -->

## Governance impact

- [ ] No repository setting, ownership rule, required check, or release-policy change.
- [ ] Governance impact is described below and documented in `docs/governance/overview.md` if needed.

<!-- Explain any governance, permissions, branch protection, or release-policy change. -->

## Changes

- <!-- List the meaningful code, configuration, or documentation changes. -->

## Validation

- <!-- List only commands actually run and their result. Do not check items that were not run. -->
- [ ] `pnpm install --frozen-lockfile`
- [ ] `pnpm run check`
- [ ] `pnpm run build`
- [ ] `pnpm audit --audit-level=low`
- [ ] `go test -race ./...`
- [ ] `go vet ./...`
- [ ] Container/Compose validation (include exact command)

## Safety checklist

- [ ] No course files, credentials, personal data, or homelab secrets are included.
- [ ] Tests use isolated fixtures; `/mnt/cursos` was not accessed or modified.
- [ ] Read-only library behavior is preserved.
- [ ] Migrations and deployment documentation are updated if needed.
- [ ] UI changes include screenshots or a short visual description.

<!-- Remove non-applicable checklist items and state any known limitations. -->
