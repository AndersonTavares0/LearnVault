# Project agent configuration

This directory contains LearnVault-specific working rules and reusable guidance
for AI coding agents. Keep these instructions aligned with `CONTRIBUTING.md`,
`docs/architecture.md`, and `docs/governance.md`.

## OpenCode integration

- Root `AGENTS.md` is the automatically discovered project instruction entry point.
- Read `repository.md` before making or reviewing changes.
- OpenCode-specific profiles are stored in `.opencode/agents/` and are read-only
  unless a future profile explicitly documents a different role.
- Do not configure a model or provider in this repository; inherit the user's
  configured model and credentials.

## Safety

- Never access or modify `/mnt/cursos`; tests use temporary directories and fake
  fixtures only.
- Never add secrets, course files, database contents, or homelab-specific private
  data to commits, logs, or prompts.
- Do not bypass branch protection, review requirements, or required CI checks.
