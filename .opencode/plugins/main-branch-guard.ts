/**
 * Guarda preventiva do agente opencode para as branches `main`/`master`.
 *
 * Proteção simples e previsível: bloqueia as ferramentas de escrita e os comandos git
 * mutantes mais comuns quando a branch atual é protegida, e opera em fail-closed quando
 * não é possível confirmar a branch. A leitura da branch usa `node:child_process`
 * diretamente — o shell `$` do runtime deixou de devolver a branch de forma confiável em
 * versões recentes do aplicativo, e a guarda não pode depender dele. Quando a verificação
 * falha, um diagnóstico é gravado em arquivo temporário e o caminho aparece no erro.
 * Não substitui a branch protection do GitHub nem cobre todas as formas de escrita via
 * Bash — veja o README deste diretório.
 */

import { execFile } from "node:child_process"
import { appendFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import type { Plugin } from "@opencode-ai/plugin"

const PROTECTED_BRANCHES = new Set(["main", "master"])

const GIT_MUTATION =
  /\bgit(\s+-(C|c)\s+\S+)?\s+(commit|push|merge|rebase|reset|revert|cherry-pick|am|apply|clean|restore|stash|add|tag)\b/
const GIT_BRANCH_SWITCH =
  /\bgit(\s+-(C|c)\s+\S+)?\s+(checkout|switch)\b(?!\s+(-b|--orphan|-c|--create))/
const GITHUB_PR_MERGE = /\bgh\b[^|;&\n]*\s+pr\s+merge\b/
const PATCH_TARGET = /^\*\*\* (?:Add File|Update File|Delete File|Move to): (.+)$/gm

/** Arquivo de diagnóstico da verificação de branch, fora do repositório. */
export const DIAGNOSTIC_LOG = path.join(tmpdir(), "learnvault-main-branch-guard.log")

type GitResult = { root: string; exitCode: number; stdout: string; stderr: string }

// Executa git sem shell intermediário e resolve sempre: a guarda nunca lança daqui.
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

// Diagnóstico best-effort: nunca pode quebrar a guarda nem tocar o repositório.
function diagnose(lines: string[]): void {
  try {
    appendFileSync(DIAGNOSTIC_LOG, `${new Date().toISOString()} ${lines.join(" | ")}\n`)
  } catch {
    // Sem diagnóstico, a guarda continua fail-closed.
  }
}

export const MainBranchGuard: Plugin = async ({ directory, worktree }) => {
  const root = worktree || directory
  // Em uso normal `worktree` e `directory` são o mesmo repositório; tentar os dois
  // protege contra mudanças de valor entre versões do aplicativo.
  const roots = [worktree, directory].filter(
    (value, index, all) => typeof value === "string" && value.length > 0 && all.indexOf(value) === index
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
      "não foi possível confirmar a branch atual",
      `worktree=${worktree}`,
      `directory=${directory}`,
      ...attempts.map((attempt) => `${attempt.root}: exit=${attempt.exitCode} stderr=${attempt.stderr.slice(0, 300)}`)
    ])
    return null
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
            `O diagnóstico está em ${DIAGNOSTIC_LOG}. ` +
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
