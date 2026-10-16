import path from "node:path"

import type { Plugin } from "@opencode-ai/plugin"

const PROTECTED_BRANCHES = new Set(["main", "master"])

const FILE_WRITING_TOOLS = new Set(["edit", "write", "patch", "multiedit", "apply_patch"])

const GIT_MUTATION =
  /\bgit(\s+-(C|c)\s+\S+)?\s+(commit|push|merge|rebase|reset|revert|cherry-pick|am|apply|clean|restore|stash|add|tag)\b/
const GIT_BRANCH_SWITCH =
  /\bgit(\s+-(C|c)\s+\S+)?\s+(checkout|switch)\b(?!\s+(-b|--orphan|-c|--create))/
const GITHUB_PR_MERGE = /\bgh\b[^|;&\n]*\s+pr\s+merge\b/

export const MainBranchGuard: Plugin = async ({ $, directory, worktree }) => {
  const root = worktree || directory

  const currentBranch = async (): Promise<string | null> => {
    try {
      const result = await $`git -C ${root} branch --show-current`.quiet().nothrow()
      const branch = (await result.text()).trim()
      return result.exitCode === 0 && branch ? branch : null
    } catch {
      return null
    }
  }

  const touchesWorktree = (filePath: unknown): boolean => {
    if (typeof filePath !== "string" || filePath.length === 0) return true
    const relative = path.relative(root, path.resolve(directory, filePath))
    return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))
  }

  return {
    "tool.execute.before": async (input, output) => {
      const branch = await currentBranch()
      if (!branch || !PROTECTED_BRANCHES.has(branch)) return

      const deny = () => {
        throw new Error(
          `A branch "${branch}" é protegida e o opencode não pode alterá-la. ` +
            "Crie uma branch de trabalho (ex.: git checkout -b feat/minha-mudanca) e repita a operação.",
        )
      }

      if (FILE_WRITING_TOOLS.has(input.tool) && touchesWorktree(output.args?.filePath)) {
        deny()
      }

      if (input.tool === "bash") {
        const command = output.args?.command
        if (typeof command !== "string") return
        if (
          GIT_MUTATION.test(command) ||
          GIT_BRANCH_SWITCH.test(command) ||
          GITHUB_PR_MERGE.test(command)
        ) {
          deny()
        }
      }
    },
  }
}
