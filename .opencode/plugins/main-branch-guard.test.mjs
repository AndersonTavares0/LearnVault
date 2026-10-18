import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { after, test } from "node:test"

const { DIAGNOSTIC_LOG, MainBranchGuard } = await import(new URL("./main-branch-guard.ts", import.meta.url).href)

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
