# Governance policy

- Work on a focused branch and update a pull request; never push directly to `main`.
- Do not bypass required reviews, status checks, or conversation resolution. CODEOWNERS
  documents ownership but is not a mandatory approval gate.
- Dependabot upgrades need a compatibility review and current required checks; green checks
  on an outdated base are insufficient.
- Use `.github/pull_request_template.md` and replace every prompt with factual content.
- The repository is canonical for policy and documentation. Managed Wiki pages are derived
  after merge and must not be edited manually.
- Keep the root limited to tool-required manifests, project policy, and common entrypoints.
  Put application code in `cmd/`, `internal/`, and `web/`; deployment in `deploy/`;
  scripts in `scripts/`; fixtures in `testdata/`; and reviewed documentation in `docs/`.
- Do not move a file merely by extension. Root files such as `go.mod`, `package.json`,
  lockfiles, Svelte/Vite configs, `.gitignore`, and GitHub-recognized policy files stay
  at the root when their tools require that location.
