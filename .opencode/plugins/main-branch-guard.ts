/**
 * Preventive guard for the opencode agent on the `main`/`master` branches.
 *
 * Simple and predictable protection: it blocks the write tools and the most common
 * mutating git commands when the current branch is protected, and it fails closed when
 * the branch cannot be confirmed. The branch is read with `node:child_process`
 * directly — the runtime's `$` shell stopped returning the branch reliably in recent
 * versions of the application, and the guard cannot depend on it. When the check
 * fails, a diagnostic is written to a temporary file and its path appears in the error.
 * The module exports only the plugin function: opencode's loader treats every export
 * of the file as a plugin and requires a function, so one extra export brings down the
 * whole load.
 * It does not replace GitHub's branch protection nor cover every way of writing through
 * the shell (`bash`/`shell`) — see the README of this directory.
 * The guard recognizes the shell tool by the names `bash` and `shell`, and the patch tool
 * by the names `apply_patch` and `patch`: opencode v2 renamed both and, without both
 * names, the protection would not fire.
 */

import { execFile } from "node:child_process"
import { appendFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import type { Plugin } from "@opencode-ai/plugin"

const PROTECTED_BRANCHES = new Set(["main", "master"])

// Nomes das ferramentas no runtime. O opencode v2 chama a ferramenta de shell de
// "shell" e a de patch de "patch"; os nomes antigos seguem reconhecidos porque
// runtimes anteriores ainda os usam. Sem isso, a guarda nunca enxerga o comando.
const SHELL_TOOLS = new Set(["bash", "shell"])
const PATCH_TOOLS = new Set(["apply_patch", "patch"])

const GIT_MUTATION =
  /\bgit(\s+-(C|c)\s+\S+)?\s+(commit|push|merge|rebase|reset|revert|cherry-pick|am|apply|clean|restore|stash|add|tag)\b/
const GIT_BRANCH_SWITCH =
  /\bgit(\s+-(C|c)\s+\S+)?\s+(checkout|switch)\b(?!\s+(-b|--orphan|-c|--create))/
const GITHUB_PR_MERGE = /\bgh\b[^|;&\n]*\s+pr\s+merge\b/
const PATCH_TARGET = /^\*\*\* (?:Add File|Update File|Delete File|Move to): (.+)$/gm

/** Diagnostic file of the branch check, outside the repository. */
const DIAGNOSTIC_LOG = path.join(tmpdir(), "learnvault-main-branch-guard.log")

type GitResult = { root: string; exitCode: number; stdout: string; stderr: string }

// Runs git without an intermediate shell and always resolves: the guard never throws from here.
function runGit(root: string, args: string[]): Promise<GitResult> {
  return new Promise((resolve) => {
    execFile(
      "git",
      ["-C", root, ...args],
      { encoding: "utf8", windowsHide: true, timeout: 5_000 },
      (error, stdout, stderr) => {
        if (error === null) {
          resolve({ root, exitCode: 0, stdout: stdout ?? "", stderr: stderr ?? "" })
          return
        }
        const code = (error as { code?: unknown }).code
        resolve({
          root,
          exitCode: typeof code === "number" ? code : 1,
          stdout: stdout ?? "",
          stderr: `${stderr ?? ""}${error.message}`.trim()
        })
      }
    )
  })
}

// Best-effort diagnostic: it must never break the guard nor touch the repository.
function diagnose(lines: string[]): void {
  try {
    appendFileSync(DIAGNOSTIC_LOG, `${new Date().toISOString()} ${lines.join(" | ")}\n`)
  } catch {
    // Without a diagnostic, the guard still fails closed.
  }
}

// Tells whether a path is inside a root. Only a first ".." segment takes the
// file out of the repository: a name that starts with two dots (e.g. "..env") is still
// inside and must be blocked like any other file.
function isInside(base: string, target: string): boolean {
  const relative = path.relative(base, target)
  if (relative === "") return true
  if (path.isAbsolute(relative)) return false
  return relative.split(/[\\/]/)[0] !== ".."
}

export const MainBranchGuard: Plugin = async ({ directory, worktree }) => {
  const root = worktree || directory
  // The path received from the agent is relative to the project directory: that is the
  // correct base to resolve it, even when the worktree points somewhere else.
  const projectRoot = typeof directory === "string" && directory.length > 0 ? directory : root
  // In normal use `worktree` and `directory` are the same repository; trying both
  // protects against value changes between versions of the application.
  const roots = [worktree, directory].filter(
    (value, index, all): value is string =>
      typeof value === "string" && value.length > 0 && all.indexOf(value) === index
  )

  const currentBranch = async (): Promise<string | null> => {
    const attempts: GitResult[] = []
    for (const candidate of roots) {
      const result = await runGit(candidate, ["branch", "--show-current"])
      attempts.push(result)
      const branch = result.stdout.trim()
      if (result.exitCode === 0 && branch) return branch
    }
    diagnose([
      "could not confirm the current branch",
      `worktree=${worktree}`,
      `directory=${directory}`,
      ...attempts.map((attempt) => `${attempt.root}: exit=${attempt.exitCode} stderr=${attempt.stderr.slice(0, 300)}`)
    ])
    return null
  }

  const touchesWorktree = (filePath: unknown): boolean => {
    if (typeof filePath !== "string" || filePath.length === 0) return true
    // The received path is resolved from the project directory and compared with
    // the known roots. Comparing against a root different from the one that resolves the
    // path would switch the protection off: when the worktree is not the repository,
    // every internal path would look external and the write would go through.
    const target = path.resolve(projectRoot, filePath)
    return roots.some((base) => isInside(base, target))
  }

  const patchTouchesWorktree = (patchText: unknown): boolean => {
    if (typeof patchText !== "string" || patchText.length === 0) return true
    const targets = Array.from(patchText.matchAll(PATCH_TARGET), (match) => match[1].trim())
    return targets.length === 0 || targets.some(touchesWorktree)
  }

  const isMutating = (tool: string, args: Record<string, unknown> | undefined): boolean => {
    if (tool === "edit" || tool === "write") return touchesWorktree(args?.filePath)
    if (PATCH_TOOLS.has(tool)) return patchTouchesWorktree(args?.patchText)
    if (!SHELL_TOOLS.has(tool)) return false
    const command = args?.command
    // A missing command, or one in an unexpected format, is treated as mutating: the guard cannot
    // fail open when it does not understand what the tool is going to run.
    if (typeof command !== "string") return true
    return GIT_MUTATION.test(command) || GIT_BRANCH_SWITCH.test(command) || GITHUB_PR_MERGE.test(command)
  }

  const deny = (reason: string): never => {
    throw new Error(reason)
  }

  return {
    "tool.execute.before": async (input, output) => {
      const args = output.args as Record<string, unknown> | undefined
      if (!isMutating(input.tool, args)) return

      const branch = await currentBranch()
      if (branch === null) {
        deny(
          "Could not confirm the current branch (the git check failed), so this operation was blocked for safety. " +
            `The diagnostic is at ${DIAGNOSTIC_LOG}. ` +
            "Do not try to work around it with other commands. Inform the user and ask what to do before repeating.",
        )
      }
      if (PROTECTED_BRANCHES.has(branch)) {
        deny(
          `The branch "${branch}" is protected: this operation could modify it. ` +
            "Do not try to work around it with other commands nor create a branch on your own. " +
            "Inform the user and ask whether you may create a working branch " +
            "(for example, `git checkout -b chore/my-change`) before creating any branch or repeating the operation.",
        )
      }
    },
  }
}
