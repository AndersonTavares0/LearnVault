# Contributing to LearnVault

Thank you for contributing to LearnVault. Contributions must preserve the project's focus: a small, sustainable, self-contained study library that never alters the original host course directory.

---

## Before Opening a Pull Request

1. Search existing issues, Discussions, and pull requests.
2. Open a Discussion if there is an architectural question or multiple design alternatives.
3. Open an issue before implementing a new feature or substantial behavior change.
4. For typos, small dependency updates, or straightforward CI fixes, an issue is optional.
5. Keep changes focused and clearly explain the behavior affected.
6. **Strict Invariant:** Never include original course files, credentials, private paths, database files, or generated media.

---

## Development & Verification Checks

Install dependencies once and use the provided scripts:

```sh
pnpm install --frozen-lockfile   # Once per checkout
pnpm dev                         # Rust API (8090) + live reload SPA (5173)
pnpm verify                      # Run all PR checks (--fast skips builds and audit)
```

- `pnpm dev` starts the Rust API at `http://127.0.0.1:8090` and the Vite SPA at `http://localhost:5173`.
- `pnpm verify` executes:
  1. `pnpm run check` (SvelteKit type and component check)
  2. `pnpm run build` (production SPA build)
  3. `pnpm run build:desktop` (embedded desktop SPA build)
  4. `pnpm --filter learnvault-web test` (Vitest unit tests)
  5. `pnpm run test:scripts` (scripts and plugin verification tests)
  6. `pnpm audit --audit-level=low` (dependency security check)
  7. `cargo fmt --all -- --check` (Rust code formatting)
  8. `cargo clippy --all-targets -- -D warnings` (Rust lints)
  9. `cargo test` (core and server workspace test suite)

> **Note:** The desktop Tauri crate is tested separately (`cargo test -p learnvault-desktop` or `pnpm dev:tauri`) because it requires native WebKit/GTK system libraries.

Tests must use synthetic fixtures and temporary directories; they must never access external networks or real course media.

---

## Pull Requests

- Use a clear title following Conventional Commits in English (e.g., `feat(player): add lesson progress persistence`).
- Describe the problem, solution, and validations executed.
- For UI changes, include visual evidence or screenshots **in the PR description/comments only**—never commit binary mockups or screenshots to Git tracking.
- Document database migrations and deployment adjustments when applicable.
- Add or update tests for any new or modified behavior.
- Link the corresponding issue (`Closes #<number>`) when applicable.

---

## Collaboration Workflow

```text
Discussion (if design decisions are open)
        ↓
Issue (feature or behavior change)
        ↓
Feature Branch in the repository
        ↓
Pull Request
        ↓
Review & CI Automated Checks
        ↓
Squash Merge into main
```

1. Assign the issue to yourself before starting.
2. Create a branch from `main` (e.g., `feat/lesson-progress` or `fix/catalog-scan`).
3. Implement changes, verify locally with `pnpm verify`, and push your branch.
4. Open a pull request and link the relevant issue.
5. Ensure all CI checks are green and resolve any reviewer comments.
6. Squash merge into `main`; never push directly to `main`.

---

## Commit Message Style

Follow Conventional Commits in English. Prefixes and descriptions are written in English to ensure clear release notes via Release Please:

```text
feat(player): add lesson progress persistence
fix(auth): handle expired session tokens gracefully
docs: update contribution guide
chore: update dependencies
```

`pnpm` is the required package manager. Do not use `npm` or `yarn` to modify dependencies or lockfiles.

---

## Clean-Room Policy

Research into external platforms may inform general product ergonomics, but never copy code, proprietary assets, branding, or pixel-level layouts. LearnVault maintains an original, calm, content-first visual identity and architecture.
