# Testing playbook

1. For behavior changes, add or adjust a focused test first and observe a relevant failure
   when practical.
2. Run `gofmt -l services/go`, `go -C services/go test -race ./...`, and
   `go -C services/go vet ./...` for Go changes.
3. Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test --all` for Rust changes.
4. Run `pnpm install --frozen-lockfile`, `pnpm run check`, `pnpm run build`, and
   `pnpm audit --audit-level=low` for SPA changes.
5. Validate the affected Compose, Containerfile, or Quadlet configuration for deployment changes.
6. For substantial changes, measure coverage: `go -C services/go test -cover ./...` for Go and
   `cargo llvm-cov --package learnvault-core --all-targets --fail-under-lines 80` for Rust.
   Keep production coverage at or above 80% without excluding code.
7. Record only commands actually run and their outcome in the pull request.
