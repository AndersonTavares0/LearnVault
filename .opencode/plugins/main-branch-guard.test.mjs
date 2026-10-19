import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { after, test } from "node:test"

const guardModule = await import(new URL("./main-branch-guard.ts", import.meta.url).href)
const { MainBranchGuard } = guardModule

// Mesmo caminho usado pelo plugin. A constante não é exportada porque o carregador do
// opencode trata todo export do módulo como plugin e exige uma função.
const DIAGNOSTIC_LOG = path.join(tmpdir(), "learnvault-main-branch-guard.log")

const OUTSIDE = path.join(tmpdir(), "learnvault-guard-test-outside.ts")

// Repositórios reais temporários: a guarda lê a branch pelo git de verdade, como em
// produção — foi um mock de shell que escondeu a regressão de ambiente que motivou a
// troca para `node:child_process`.
const tempRoots = []

function repoWithBranch(branch) {
  const root = mkdtempSync(path.join(tmpdir(), "learnvault-guard-repo-"))
  execFileSync("git", ["init", "-q"], { cwd: root })
  execFileSync("git", ["checkout", "-q", "-b", branch], { cwd: root })
  tempRoots.push(root)
  return root
}

function plainDirectory() {
  const root = mkdtempSync(path.join(tmpdir(), "learnvault-guard-plain-"))
  tempRoots.push(root)
  return root
}

after(() => {
  for (const root of tempRoots) rmSync(root, { recursive: true, force: true })
})

const hookFor = async ({ branch = "main", worktree, directory } = {}) => {
  const repo = repoWithBranch(branch)
  const hooks = await MainBranchGuard({ directory: directory ?? repo, worktree: worktree ?? repo })
  return hooks["tool.execute.before"]
}

const attempt = async (hook, tool, args) => {
  try {
    await hook({ tool, sessionID: "test", callID: "test" }, { args })
    return { blocked: false, message: "" }
  } catch (error) {
    return { blocked: true, message: error instanceof Error ? error.message : String(error) }
  }
}

const patchText = (...targets) => targets.map((target) => `*** Update File: ${target}`).join("\n")

test("exports only plugin functions, as the opencode loader requires", async () => {
  // Espelha getServerPlugin() do opencode: todo export precisa ser uma função ou um
  // objeto com `server` função; um export a mais derruba o carregamento do arquivo.
  for (const value of Object.values(guardModule)) {
    const isPlugin =
      typeof value === "function" || (value !== null && typeof value === "object" && typeof value.server === "function")
    assert.equal(isPlugin, true, "todo export do módulo precisa ser uma função de plugin")
  }
})

test("blocks file tools that touch the worktree on protected branches", async () => {
  const hook = await hookFor({ branch: "main" })
  const cases = [
    ["edit", { filePath: "README.md" }],
    ["write", { filePath: ".opencode/plugins/novo.ts" }],
    ["apply_patch", { patchText: patchText("apps/web/src/App.svelte") }],
    ["apply_patch", {}],
  ]
  for (const [tool, args] of cases) {
    const result = await attempt(hook, tool, args)
    assert.equal(result.blocked, true, `${tool} deveria ser bloqueado`)
    assert.match(result.message, /protegida/)
  }
})

test("allows file tools outside the worktree on protected branches", async () => {
  const hook = await hookFor({ branch: "main" })
  const edit = await attempt(hook, "edit", { filePath: OUTSIDE })
  assert.equal(edit.blocked, false)
  const applyPatch = await attempt(hook, "apply_patch", { patchText: patchText(OUTSIDE) })
  assert.equal(applyPatch.blocked, false)
})

test("blocks git mutations on protected branches", async () => {
  const hook = await hookFor({ branch: "main" })
  const commands = [
    'git commit -m "x"',
    "git push origin main",
    "git add .",
    "git checkout main",
    "git switch main",
    "gh pr merge 86 --squash",
  ]
  for (const command of commands) {
    const result = await attempt(hook, "bash", { command })
    assert.equal(result.blocked, true, `"${command}" deveria ser bloqueado`)
  }
})

test("allows branch creation and read-only commands on protected branches", async () => {
  const hook = await hookFor({ branch: "main" })
  const commands = ["git checkout -b feat/x", "git switch -c feat/x", "git status", "git log --oneline", "cargo test --all"]
  for (const command of commands) {
    const result = await attempt(hook, "bash", { command })
    assert.equal(result.blocked, false, `"${command}" deveria ser permitido`)
  }
  const read = await attempt(hook, "read", { filePath: "README.md" })
  assert.equal(read.blocked, false)
})

test("allows mutations on a feature branch", async () => {
  const hook = await hookFor({ branch: "feat/x" })
  const edit = await attempt(hook, "edit", { filePath: "README.md" })
  assert.equal(edit.blocked, false)
  const commit = await attempt(hook, "bash", { command: 'git commit -m "x"' })
  assert.equal(commit.blocked, false)
  const applyPatch = await attempt(hook, "apply_patch", { patchText: patchText("README.md") })
  assert.equal(applyPatch.blocked, false)
})

test("falls back to the project directory when the worktree is not a repository", async () => {
  const hook = await hookFor({ branch: "feat/x", worktree: plainDirectory() })
  const edit = await attempt(hook, "edit", { filePath: "README.md" })
  assert.equal(edit.blocked, false)
})

test("blocks writes to the project directory when the worktree is not a repository", async () => {
  const repo = repoWithBranch("main")
  const hooks = await MainBranchGuard({ directory: repo, worktree: plainDirectory() })
  const hook = hooks["tool.execute.before"]

  // O caminho do agente é relativo ao diretório do projeto: mesmo com o worktree
  // apontando para outro lugar, a escrita dentro do repositório precisa ser bloqueada.
  for (const filePath of ["README.md", path.join(repo, "README.md")]) {
    const result = await attempt(hook, "edit", { filePath })
    assert.equal(result.blocked, true, `"${filePath}" deveria ser bloqueado`)
    assert.match(result.message, /protegida/)
  }

  // Fora do repositório continua permitido.
  const outside = await attempt(hook, "edit", { filePath: OUTSIDE })
  assert.equal(outside.blocked, false)
})

test("fails closed and writes a diagnostic when the branch cannot be verified", async () => {
  const directory = plainDirectory()
  const hooks = await MainBranchGuard({ directory, worktree: directory })
  const hook = hooks["tool.execute.before"]

  const edit = await attempt(hook, "edit", { filePath: "README.md" })
  assert.equal(edit.blocked, true)
  assert.match(edit.message, /Não foi possível confirmar/)
  assert.ok(edit.message.includes(DIAGNOSTIC_LOG))

  const commit = await attempt(hook, "bash", { command: 'git commit -m "x"' })
  assert.equal(commit.blocked, true)

  const status = await attempt(hook, "bash", { command: "git status" })
  assert.equal(status.blocked, false)

  const read = await attempt(hook, "read", { filePath: "README.md" })
  assert.equal(read.blocked, false)

  // O diagnóstico registra o motivo para depuração futura, sem tocar o repositório.
  assert.match(readFileSync(DIAGNOSTIC_LOG, "utf8"), /não foi possível confirmar a branch atual/)
})

test("instructs the agent to ask the user before creating a branch", async () => {
  const hook = await hookFor({ branch: "main" })
  const result = await attempt(hook, "edit", { filePath: "README.md" })
  assert.equal(result.blocked, true)
  assert.match(result.message, /Informe o usuário/)
  assert.match(result.message, /antes de criar/)
})

test("blocks writes to a file whose name starts with two dots inside the worktree", async () => {
  const repo = repoWithBranch("main")
  const hooks = await MainBranchGuard({ directory: repo, worktree: repo })
  const hook = hooks["tool.execute.before"]

  // "..env" está dentro do repositório: o nome começa com dois pontos, mas não sobe de nível.
  for (const filePath of ["..env", path.join(repo, "..env")]) {
    const result = await attempt(hook, "edit", { filePath })
    assert.equal(result.blocked, true, `"${filePath}" deveria ser bloqueado`)
    assert.match(result.message, /protegida/)
  }

  // Subir de nível sai do worktree e continua permitido.
  const outside = await attempt(hook, "edit", { filePath: path.join(repo, "..", "fora.ts") })
  assert.equal(outside.blocked, false)
})

test("fails closed when a bash call has no readable command", async () => {
  const hook = await hookFor({ branch: "main" })
  for (const args of [{}, { command: 42 }, { command: null }, { command: {} }, { command: ["git", "commit"] }]) {
    const result = await attempt(hook, "bash", args)
    assert.equal(result.blocked, true, "bash sem comando legível deveria ser bloqueado")
    assert.match(result.message, /protegida/)
  }

  // Em branch de trabalho a guarda não bloqueia: a proteção é da branch protegida.
  const featureHook = await hookFor({ branch: "feat/x" })
  const allowed = await attempt(featureHook, "bash", { command: 42 })
  assert.equal(allowed.blocked, false)
})

test("treats a path on another Windows drive as outside the worktree", { skip: process.platform !== "win32" }, async () => {
  const repo = repoWithBranch("main")
  const hooks = await MainBranchGuard({ directory: repo, worktree: repo })
  const hook = hooks["tool.execute.before"]

  // Outra unidade não é o mesmo repositório: `path.relative` devolve um caminho
  // absoluto e a escrita ali segue permitida.
  const current = path.parse(repo).root
  const other = current.toUpperCase().startsWith("C:") ? "Z:\\" : "C:\\"
  const result = await attempt(hook, "edit", { filePath: path.join(other, "fora.ts") })
  assert.equal(result.blocked, false)
})
