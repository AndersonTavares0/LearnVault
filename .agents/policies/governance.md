# Governance policy

- Work on a focused branch and update a pull request; never push directly to `main`.
- Do not bypass required reviews, status checks, or conversation resolution. CODEOWNERS
  documents ownership but is not a mandatory approval gate.
- Dependabot upgrades need a compatibility review and current required checks; green checks
  on an outdated base are insufficient.
- Automatic pull request labeling (`.github/labeler.yml` and the `Labeler` workflow) is
  informational only: it is not a required check, does not gate merge, and does not classify
  security or work type. Labels referenced by the configuration must already exist in the
  repository.
- Use `.github/pull_request_template.md` and replace every prompt with factual content.
- The repository is canonical for policy and documentation. Managed Wiki pages are derived
  after merge and must not be edited manually.
- Keep the root limited to tool-required manifests, project policy, and common entrypoints.
  Rust code lives in `crates/` (shared domain in `crates/core`); the current Go service in
  `services/go/` is a self-contained module (use `go -C services/go ...`) and remains as
  reference implementation until the rewrite covers the approved requirements. The SPA lives
  in `apps/web/` as a pnpm workspace member. Deployment lives in `deploy/`; scripts in
  `scripts/`; fixtures in `testdata/`; and reviewed documentation in `docs/`. Community files
  (`CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`) live in `.github/`, the changelog
  lives in `docs/releases/CHANGELOG.md`, and Release Please configuration lives in
  `.github/release-please/`. OpenCode-specific profiles live in `.opencode/agents/` and are
  read-only.
- Do not move a file merely by extension. Root files such as `Cargo.toml`, `Cargo.lock`,
  `rust-toolchain.toml`, the workspace `package.json`, lockfiles, `.gitignore`, `LICENSE`, and
  tool-required entrypoints stay at the root when their tools require that location. The Go
  module is self-sufficient under `services/go/`; the SPA source is under `apps/web/`; only the
  pnpm workspace and Cargo workspace manifests remain at the repo root.
- Use GitHub Flow: issue when required, branch in this repository, pull request, review,
  required checks, and squash merge. Forks are not part of the current workflow.
- Use Discussions for unresolved questions and design alternatives. Features and behavior
  changes require an issue; typos, dependency updates, and simple CI changes do not.
- Keep `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`,
  the Wiki, and editorial documentation in Brazilian Portuguese. Only `LICENSE` stays in
  English. Community files live in `.github/`; the changelog lives in
  `docs/releases/CHANGELOG.md`. Conventional Commit prefixes remain in English while
  descriptions are written in Brazilian Portuguese for Release Please consistency.
