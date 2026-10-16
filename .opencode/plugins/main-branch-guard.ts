/**
 * Guarda preventiva do agente opencode para as branches `main`/`master`.
 *
 * Proteção simples e previsível: bloqueia as ferramentas de escrita e os comandos git
 * mutantes mais comuns quando a branch atual é protegida, e opera em fail-closed quando
 * não é possível confirmar a branch. Não substitui a branch protection do GitHub nem
 * cobre todas as formas de escrita via Bash — veja o README deste diretório.
 */

import path from "node:path"

import type { Plugin } from "@opencode-ai/plugin"

const PROTECTED_BRANCHES = new Set(["main", "master"])

const GIT_MUTATION =
  /\bgit(\s+-(C|c)\s+\S+)?\s+(commit|push|merge|rebase|reset|revert|cherry-pick|am|apply|clean|restore|stash|add|tag)\b/
const GIT_BRANCH_SWITCH =
  /\bgit(\s+-(C|c)\s+\S+)?\s+(checkout|switch)\b(?!\s+(-b|--orphan|-c|--create))/
const GITHUB_PR_MERGE = /\bgh\b[^|;&\n]*\s+pr\s+merge\b/
const PATCH_TARGET = /^\*\*\* (?:Add File|Update File|Delete File|Move to): (.+)$/gm

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

  const patchTouchesWorktree = (patchText: unknown): boolean => {
    if (typeof patchText !== "string" || patchText.length === 0) return true
    const targets = Array.from(patchText.matchAll(PATCH_TARGET), (match) => match[1].trim())
    return targets.length === 0 || targets.some(touchesWorktree)
  }

  const isMutating = (tool: string, args: Record<string, unknown> | undefined): boolean => {
    if (tool === "edit" || tool === "write") return touchesWorktree(args?.filePath)
    if (tool === "apply_patch") return patchTouchesWorktree(args?.patchText)
    if (tool !== "bash") return false
    const command = args?.command
    return (
      typeof command === "string" &&
      (GIT_MUTATION.test(command) || GIT_BRANCH_SWITCH.test(command) || GITHUB_PR_MERGE.test(command))
    )
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
          "Não foi possível confirmar a branch atual (a verificação com git falhou), então esta operação foi bloqueada por segurança. " +
            "Não tente contornar com outros comandos. Informe o usuário e pergunte o que fazer antes de repetir.",
        )
      }
      if (PROTECTED_BRANCHES.has(branch)) {
        deny(
          `A branch "${branch}" é protegida: esta operação pode modificá-la. ` +
            "Não tente contornar com outros comandos nem criar uma branch por conta própria. " +
            "Informe o usuário e pergunte se você pode criar uma branch de trabalho " +
            "(por exemplo, `git checkout -b chore/minha-mudanca`) antes de criar qualquer branch ou repetir a operação.",
        )
      }
    },
  }
}
