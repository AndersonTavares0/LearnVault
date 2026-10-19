/**
 * Guarda preventiva do agente opencode para as branches `main`/`master`.
 *
 * Proteção simples e previsível: bloqueia as ferramentas de escrita e os comandos git
 * mutantes mais comuns quando a branch atual é protegida, e opera em fail-closed quando
 * não é possível confirmar a branch. A leitura da branch usa `node:child_process`
 * diretamente — o shell `$` do runtime deixou de devolver a branch de forma confiável em
 * versões recentes do aplicativo, e a guarda não pode depender dele. Quando a verificação
 * falha, um diagnóstico é gravado em arquivo temporário e o caminho aparece no erro.
 * O módulo exporta apenas a função do plugin: o carregador do opencode trata cada export
 * do arquivo como um plugin e exige uma função, então um export a mais derruba o
 * carregamento inteiro.
 * Não substitui a branch protection do GitHub nem cobre todas as formas de escrita via
 * Bash — veja o README deste diretório.
 * A guarda reconhece a ferramenta de shell pelos nomes `bash` e `shell`, e a de patch
 * pelos nomes `apply_patch` e `patch`: o opencode v2 renomeou as duas e, sem os dois
 * nomes, a proteção não dispararia.
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

/** Arquivo de diagnóstico da verificação de branch, fora do repositório. */
const DIAGNOSTIC_LOG = path.join(tmpdir(), "learnvault-main-branch-guard.log")

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

// Diz se um caminho está dentro de uma raiz. Só o primeiro segmento ".." tira o
// arquivo do repositório: um nome que começa com dois pontos (ex.: "..env") continua
// dentro e precisa ser bloqueado como qualquer outro arquivo.
function isInside(base: string, target: string): boolean {
  const relative = path.relative(base, target)
  if (relative === "") return true
  if (path.isAbsolute(relative)) return false
  return relative.split(/[\\/]/)[0] !== ".."
}

export const MainBranchGuard: Plugin = async ({ directory, worktree }) => {
  const root = worktree || directory
  // O caminho recebido do agente é relativo ao diretório do projeto: essa é a base
  // correta para resolvê-lo, mesmo quando o worktree aponta para outro lugar.
  const projectRoot = typeof directory === "string" && directory.length > 0 ? directory : root
  // Em uso normal `worktree` e `directory` são o mesmo repositório; tentar os dois
  // protege contra mudanças de valor entre versões do aplicativo.
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
      "não foi possível confirmar a branch atual",
      `worktree=${worktree}`,
      `directory=${directory}`,
      ...attempts.map((attempt) => `${attempt.root}: exit=${attempt.exitCode} stderr=${attempt.stderr.slice(0, 300)}`)
    ])
    return null
  }

  const touchesWorktree = (filePath: unknown): boolean => {
    if (typeof filePath !== "string" || filePath.length === 0) return true
    // O caminho recebido é resolvido a partir do diretório do projeto e comparado com
    // as raízes conhecidas. Comparar contra uma raiz diferente da que resolve o
    // caminho desligaria a proteção: no caso em que o worktree não é o repositório,
    // todo caminho interno pareceria externo e a escrita passaria.
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
    // Comando ausente ou em formato inesperado é tratado como mutante: a guarda não pode
    // falhar aberta quando não entende o que a ferramenta vai executar.
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
