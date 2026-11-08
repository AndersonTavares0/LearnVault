# Plugin: Protected Branch Guard

`main-branch-guard.ts` is a local OpenCode plugin that prevents the agent from modifying the
repository when the current branch is `main` or `master`.

## Features

- Blocks agent file-writing tools (`edit`, `write`, `apply_patch`/`patch`) for files within
  the repository (for patch tools, target paths are extracted from `*** Add File`,
  `*** Update File`, `*** Delete File`, and `*** Move to` headers). Path resolution ensures
  escaped segments (`..`) are properly guarded, including dotfiles.
- Blocks mutating git shell commands (`bash` or `shell`): `commit`, `push`, `add`, `merge`,
  `rebase`, `reset`, `revert`, `stash`, `tag`, `checkout`/`switch` without creating a branch,
  and `gh pr merge`.
- Blocks shell calls whose command line cannot be parsed (missing or unexpected argument format):
  fails closed rather than assuming ambiguous commands are safe.
- Permits read operations, test execution, builds, and branch creation
  (`git checkout -b` / `git switch -c`).
- Operates in **fail-closed** mode: if current branch confirmation fails, mutations are blocked.
- Reads branch status via `node:child_process` (without relying on runtime `$` shell) checking
  both `worktree` and `directory`. On failure, writes diagnostics to
  `learnvault-main-branch-guard.log` in the system temporary directory and references it in the
  error message returned to the agent.

## Runtime Tool Names

The guard validates the exact tool name to detect mutations. OpenCode runtime tool names evolved:
the shell tool appears as `bash` in older versions and `shell` starting in OpenCode v2; patch tools
appear as `apply_patch` or `patch`. The guard recognizes both variants.

Verified with **OpenCode v2.0.32**.

## Agent Instructions on Interception

When intercepted, the agent is instructed **not to circumvent** protections with alternate
commands: it must inform the user and request permission to create a working branch.

## Limitations (Preventive Defense)

- Acts as a preventive safety rail against agent mistakes: **does not replace** GitHub branch
  protection rules or mandatory pull request reviews.
- Changes to the plugin take effect only after restarting OpenCode.

## Module Contract

The plugin file exports **only** the plugin function. OpenCode loaders treat each export as a
plugin and require a function (or server object); extraneous exports prevent the module from
loading. The test suite `main-branch-guard.test.mjs` verifies this invariant.

## Verification

```sh
node --test .opencode/plugins/main-branch-guard.test.mjs
```

CI executes this check on every pull request.
