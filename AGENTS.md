# LearnVault agent instructions

## Mandatory rule for every agent

Every agent working in this repository must follow this instruction file, read
[`.agents/repository.md`](.agents/repository.md), and then read the relevant policies,
playbooks, architecture documents, and checklists before changing anything. No agent may
skip, replace, or create a parallel project workflow. The repository rules apply equally
to implementation, review, planning, documentation, and automation agents.

OpenCode loads this file automatically. Project-specific reviewer profiles live in
`.opencode/agents/`; shared policies, playbooks, checklists, and templates live in
`.agents/`.

## Multi-agent collaboration

LearnVault is developed with help from multiple agents and harnesses (OpenCode,
OMP/Pi, Antigravity CLI / Agy), often concurrently. These rules apply to every
agent, without requiring any shared orchestration infrastructure:

- **Verify before acting** — inspect `git status`, branches, worktrees, recent
  history, and in-flight PRs/issues before starting. Never assume you are the only
  active agent.
- **Preserve others' work** — never overwrite, discard, revert, or commit changes
  authored by another agent without their authorization or an explicit maintainer
  instruction.
- **Avoid conflicts** — do not duplicate tasks already in flight and do not edit the
  same files simultaneously. When in doubt, work in an isolated branch or worktree.
- **Coordinate through repository state** — use Git, issues, PRs, Discussions, and
  the documents under `.agents/` as the source of truth for what is happening. Do
  not assume direct communication between agents.
- **Integrate safely** — review external changes, respect ADRs and contracts, and
  run the relevant validations (tests, lints, build) before merging. Do not merge
  with unresolved conflicts, pending checks, or hidden failures: when a test fails,
  fix the real cause and keep it visible.
- **Act responsibly with autonomy** — continue independent tasks without asking for
  confirmation, and only request guidance on genuine conflicts, ambiguous
  ownership, or risk of losing work.
