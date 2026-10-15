# Testing playbook

1. For behavior changes, add or adjust a focused test first and observe a relevant failure
   when practical.
2. Run `gofmt`, `go test -race ./...`, and `go vet ./...` for Go changes.
3. Run `pnpm install --frozen-lockfile`, `pnpm run check`, `pnpm run build`, and
   `pnpm audit --audit-level=low` for SPA changes.
4. Validate the affected Compose, Containerfile, or Quadlet configuration for deployment changes.
5. For substantial changes, measure `go test -cover ./...`; keep global production coverage at
   or above 80% without excluding code.
6. Record only commands actually run and their outcome in the pull request.
