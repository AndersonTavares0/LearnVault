import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { after, test } from "node:test"

const guardModule = await import(new URL("./main-branch-guard.ts", import.meta.url).href)
const { MainBranchGuard } = guardModule

// Same path the plugin uses. The constant is not exported because opencode's loader
// treats every export of the module as a plugin and requires a function.
const DIAGNOSTIC_LOG = path.join(tmpdir(), "learnvault-main-branch-guard.log")

const OUTSIDE = path.join(tmpdir(), "learnvault-guard-test-outside.ts")

// Real temporary repositories: the guard reads the branch through real git, as in
// production — it was a shell mock that hid the environment regression that motivated
// the switch to `node:child_process`.
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
  // Mirrors opencode's getServerPlugin(): every export must be a function or an
  // object with a `server` function; one extra export brings down the load of the file.
  for (const value of Object.values(guardModule)) {
    const isPlugin =
      typeof value === "function" || (value !== null && typeof value === "object" && typeof value.server === "function")
    assert.equal(isPlugin, true, "every export of the module must be a plugin function")
  }
})

test("blocks file tools that touch the worktree on protected branches", async () => {
  const hook = await hookFor({ branch: "main" })
  const cases = [
    ["edit", { filePath: "README.md" }],
    ["write", { filePath: ".opencode/plugins/novo.ts" }],
    ["apply_patch", { patchText: patchText("apps/web-next/src/app.tsx") }],
    ["apply_patch", {}],
  ]
  for (const [tool, args] of cases) {
    const result = await attempt(hook, tool, args)
    assert.equal(result.blocked, true, `${tool} deveria ser bloqueado`)
    assert.match(result.message, /protected/)
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

  // The agent's path is relative to the project directory: even with the worktree
  // pointing elsewhere, a write inside the repository must be blocked.
  for (const filePath of ["README.md", path.join(repo, "README.md")]) {
    const result = await attempt(hook, "edit", { filePath })
    assert.equal(result.blocked, true, `"${filePath}" deveria ser bloqueado`)
    assert.match(result.message, /protected/)
  }

  // Outside the repository it is still allowed.
  const outside = await attempt(hook, "edit", { filePath: OUTSIDE })
  assert.equal(outside.blocked, false)
})

test("fails closed and writes a diagnostic when the branch cannot be verified", async () => {
  const directory = plainDirectory()
  const hooks = await MainBranchGuard({ directory, worktree: directory })
  const hook = hooks["tool.execute.before"]

  const edit = await attempt(hook, "edit", { filePath: "README.md" })
  assert.equal(edit.blocked, true)
  assert.match(edit.message, /Could not confirm/)
  assert.ok(edit.message.includes(DIAGNOSTIC_LOG))

  const commit = await attempt(hook, "bash", { command: 'git commit -m "x"' })
  assert.equal(commit.blocked, true)

  const status = await attempt(hook, "bash", { command: "git status" })
  assert.equal(status.blocked, false)

  const read = await attempt(hook, "read", { filePath: "README.md" })
  assert.equal(read.blocked, false)

  // The diagnostic records the reason for future debugging, without touching the repository.
  assert.match(readFileSync(DIAGNOSTIC_LOG, "utf8"), /could not confirm the current branch/)
})

test("instructs the agent to ask the user before creating a branch", async () => {
  const hook = await hookFor({ branch: "main" })
  const result = await attempt(hook, "edit", { filePath: "README.md" })
  assert.equal(result.blocked, true)
  assert.match(result.message, /Inform the user/)
  assert.match(result.message, /before creating/)
})

test("blocks writes to a file whose name starts with two dots inside the worktree", async () => {
  const repo = repoWithBranch("main")
  const hooks = await MainBranchGuard({ directory: repo, worktree: repo })
  const hook = hooks["tool.execute.before"]

  // "..env" is inside the repository: the name starts with two dots, but does not go up a level.
  for (const filePath of ["..env", path.join(repo, "..env")]) {
    const result = await attempt(hook, "edit", { filePath })
    assert.equal(result.blocked, true, `"${filePath}" deveria ser bloqueado`)
    assert.match(result.message, /protected/)
  }

  // Going up a level leaves the worktree and is still allowed.
  const outside = await attempt(hook, "edit", { filePath: path.join(repo, "..", "fora.ts") })
  assert.equal(outside.blocked, false)
})

test("fails closed when a bash call has no readable command", async () => {
  const hook = await hookFor({ branch: "main" })
  for (const args of [{}, { command: 42 }, { command: null }, { command: {} }, { command: ["git", "commit"] }]) {
    const result = await attempt(hook, "bash", args)
    assert.equal(result.blocked, true, "bash without a readable command should be blocked")
    assert.match(result.message, /protected/)
  }

  // On a working branch the guard does not block: the protection is for the protected branch.
  const featureHook = await hookFor({ branch: "feat/x" })
  const allowed = await attempt(featureHook, "bash", { command: 42 })
  assert.equal(allowed.blocked, false)
})

test("blocks git mutations when the shell tool is named shell", async () => {
  const hook = await hookFor({ branch: "main" })
  const commands = ['git commit -m "x"', "git push origin main", "git add .", "gh pr merge 86 --squash"]
  for (const command of commands) {
    const result = await attempt(hook, "shell", { command })
    assert.equal(result.blocked, true, `"${command}" deveria ser bloqueado`)
    assert.match(result.message, /protected/)
  }

  // Reading is still allowed and an unreadable command fails closed.
  const read = await attempt(hook, "shell", { command: "git status" })
  assert.equal(read.blocked, false)
  const unreadable = await attempt(hook, "shell", {})
  assert.equal(unreadable.blocked, true)
})

test("blocks patch writes when the patch tool is named patch", async () => {
  const hook = await hookFor({ branch: "main" })
  const blocked = await attempt(hook, "patch", { patchText: patchText("apps/web-next/src/app.tsx") })
  assert.equal(blocked.blocked, true, "escrita via patch deveria ser bloqueada")
  assert.match(blocked.message, /protected/)

  // A patch that points outside the repository is still allowed.
  const outside = await attempt(hook, "patch", { patchText: patchText(OUTSIDE) })
  assert.equal(outside.blocked, false)
})

test("fails closed when a patch call has no readable patch text", async () => {
  const hook = await hookFor({ branch: "main" })
  for (const args of [{}, { patchText: 42 }, { patchText: null }, { patchText: {} }]) {
    const result = await attempt(hook, "patch", args)
    assert.equal(result.blocked, true, "a patch without readable text should be blocked")
    assert.match(result.message, /protected/)
  }

  // On a working branch the guard does not block: the protection is for the protected branch.
  const featureHook = await hookFor({ branch: "feat/x" })
  const allowed = await attempt(featureHook, "patch", {})
  assert.equal(allowed.blocked, false)
})

test("treats a path on another Windows drive as outside the worktree", { skip: process.platform !== "win32" }, async () => {
  const repo = repoWithBranch("main")
  const hooks = await MainBranchGuard({ directory: repo, worktree: repo })
  const hook = hooks["tool.execute.before"]

  // Another drive is not the same repository: `path.relative` returns an
  // absolute path and a write there stays allowed.
  const current = path.parse(repo).root
  const other = current.toUpperCase().startsWith("C:") ? "Z:\\" : "C:\\"
  const result = await attempt(hook, "edit", { filePath: path.join(other, "fora.ts") })
  assert.equal(result.blocked, false)
})
