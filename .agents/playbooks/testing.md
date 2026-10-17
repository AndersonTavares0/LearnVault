# Testing playbook

1. For behavior changes, add or adjust a focused test first and observe a relevant failure
   when practical.
2. Run `gofmt -l services/go`, `go -C services/go test -race ./...`, and
   `go -C services/go vet ./...` for Go changes.
3. Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test --all` for Rust changes.
4. Run `pnpm install --frozen-lockfile`, `pnpm run check`, `pnpm run build`, and
   `pnpm audit --audit-level=low` for SPA changes. `pnpm verify` runs the SPA and Rust
   checks of steps 3 and 4 (core and server) together, plus `pnpm run build:desktop` and the
   SPA tests; `pnpm verify --fast` skips builds and audit during iteration. For the desktop
   crate, use `cargo test -p learnvault-desktop` or `pnpm dev:tauri`. Run `pnpm test:scripts`
   when changing `scripts/verify.mjs`; it pins the `pnpm verify` wiring and failure propagation.
5. Validate the affected Compose, Containerfile, or Quadlet configuration for deployment changes.
6. For substantial changes, measure coverage: `go -C services/go test -cover ./...` for Go and
   `cargo llvm-cov --package learnvault-core --all-targets --fail-under-lines 80` for Rust.
   Keep production coverage at or above 80% without excluding code.
7. Record only commands actually run and their outcome in the pull request.

Rust test wall time is dominated by Argon2id hashing. `Cargo.toml` keeps
`[profile.dev.package.argon2]` at `opt-level = 3` so a dev/test hash costs about 60 ms instead
of about 1.3 s; the override applies to `dev` and `test` builds only and never changes
`profile.release` or published artifacts. Keep it: the CI step `Run Rust tests` measured 108 s
without the override and 15 s with it (runs 39000631382 and 39000535553).
