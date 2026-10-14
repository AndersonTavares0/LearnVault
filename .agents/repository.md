# Repository rules for coding agents

## Product and architecture

- LearnVault is a self-hosted study library: Go service, SQLite, and SvelteKit SPA.
- Preserve the clean-room product policy: do not copy another product's code,
  assets, brand, proprietary text, or pixel-level layout.
- Keep Docker/Podman as optional deployment adapters; do not make them a runtime
  requirement for native packages.
- Read `docs/architecture.md` and `ROADMAP.md` before architectural or roadmap work.

## Data safety

- `/mnt/cursos` is external, read-only user data. Never inspect, scan, write, move,
  rename, duplicate, or include it in a backup during agent work.
- Tests must use `t.TempDir()`, fake fixtures under `testdata/`, or in-memory fakes.
- Never place generated data, credentials, personal study data, or homelab secrets
  in source control.
- Preserve authenticated media access and read-only library behavior.

## Engineering workflow

- Make focused changes and tests first for behavior changes; demonstrate the
  failing test before implementing when practical, then rerun it after the fix.
- Go changes: `gofmt`, `go test -race ./...`, and `go vet ./...`.
- SPA changes: `npm run check`, `npm run build`, and `npm audit --audit-level=low`.
- Deployment changes: validate the affected Compose/Containerfile/Quadlet config.
- For substantial changes, check Go coverage using `go test -cover ./...`; preserve
  the current 80% overall coverage baseline without excluding production code.
- Do not claim a command passed unless it was actually run; record the exact
  command and outcome in the PR body.

## GitHub governance

- Work on a focused branch and open/update a PR; never push directly to `main`.
- Do not bypass required reviews, CODEOWNERS, status checks, or conversation
  resolution, even when explicitly asked to merge.
- Dependabot upgrades require compatibility review and passing required checks;
  a green check on an outdated base is not sufficient.
- Use `.github/pull_request_template.md` and replace every prompt with factual
  content before opening/updating a PR.
