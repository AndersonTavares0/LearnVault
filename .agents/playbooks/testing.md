# Testing playbook

1. For behavior changes, add or adjust a focused test first and observe a relevant failure
   when practical.
2. In shells where Cargo is not in the default non-interactive PATH, set `export PATH="$HOME/.cargo/bin:$PATH"`.
3. Run `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test --all` for Rust changes.
4. Run `pnpm run check`, `pnpm --filter learnvault-web test`, and `pnpm run build` for SPA changes.
   `pnpm verify` runs the full verification pipeline; `pnpm verify --fast` skips builds and audit during fast iterations.
5. Run `pnpm exec playwright test` for end-to-end browser integration tests (verifying authentication, navigation, player, and i18n persistence).
6. For the desktop crate, use `cargo test -p learnvault-desktop` or `pnpm dev:tauri`.
7. Run `pnpm run test:scripts` when changing anything under `scripts/`; it runs
   `node --test "scripts/*.test.mjs"` and CI executes it on every pull request.
8. Validate the affected Compose, Containerfile, or Quadlet configuration for deployment changes.
9. For substantial changes, measure coverage:
   `cargo llvm-cov --package learnvault-core --all-targets --fail-under-lines 80` for Rust.
   Keep production coverage at or above 80% without excluding code.
10. Record only commands actually run and their outcome in the pull request.

Rust test wall time is dominated by Argon2id hashing. `Cargo.toml` keeps the Argon2id hot path
fast in dev/test: `opt-level = 3` and `debug = false` for the `argon2` crate and for `blake2`,
the compression function it calls, so a dev/test hash costs about 60 ms instead of about 1.3 s.
Those overrides apply to `dev` and `test` builds only and never change `profile.release` or
published artifacts. Keep them: the CI step `Run Rust tests` measured 108 s without the override
and 15 s with it.
