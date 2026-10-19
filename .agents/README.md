# Project agent configuration

This directory contains LearnVault-specific working rules and reusable guidance
for AI coding agents. Keep these instructions aligned with `.github/CONTRIBUTING.md`,
`docs/architecture/overview.md`, and `docs/governance/overview.md`.

## OpenCode integration

- Root `AGENTS.md` is the automatically discovered project instruction entry point.
- Read `repository.md` before making or reviewing changes, then the relevant policy,
  playbook, and checklist.
- OpenCode-specific profiles are stored in `.opencode/agents/` and are read-only
  unless a future profile explicitly documents a different role.
- Do not configure a model or provider in this repository; inherit the user's
  configured model and credentials.

## Safety

- Never access or modify the user's course library outside the project; it is
  external, read-only data. Tests use temporary directories and fake fixtures only.
- Never add secrets, course files, database contents, or private infrastructure
  details to commits, logs, or prompts.
- Do not bypass branch protection, review requirements, or required CI checks.

## Layout

| Path | Purpose |
| --- | --- |
| `repository.md` | mandatory short index and non-negotiable rules |
| `policies/` | durable safety, governance, and clean-room boundaries |
| `playbooks/` | focused workflows for tests, dependencies, releases, and documentation |
| `checklists/` | reviewable pre-PR checks |
| `templates/` | reusable factual records; never a source of secrets |
