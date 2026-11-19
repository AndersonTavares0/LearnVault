#!/usr/bin/env node
// Safe cleanup of merged worktrees, local branches, and stale remote-tracking refs.
// Dry-run by default: pass --apply to actually prune or remove anything.
// Never removes a worktree or local branch unless GitHub confirms a MERGED PR for that exact
// head branch, the worktree HEAD matches the merged PR head SHA, and the worktree is clean.
// Lists (and never deletes) remote branches without an open PR.

import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

function defaultRun(cmd, args, opts = {}) {
  return execFileSync(cmd, args, {
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    ...opts,
  }).trim();
}

export function parseWorktreeList(rawOutput) {
  if (!rawOutput || typeof rawOutput !== 'string') return [];

  return rawOutput
    .trim()
    .split(/\r?\n\r?\n/)
    .filter(Boolean)
    .map((block) => {
      const entry = {
        path: null,
        head: null,
        branch: null,
        prunable: false,
        bare: false,
        detached: false,
      };
      for (const line of block.split(/\r?\n/)) {
        if (line.startsWith('worktree ')) entry.path = line.slice('worktree '.length);
        else if (line.startsWith('HEAD ')) entry.head = line.slice('HEAD '.length);
        else if (line.startsWith('branch refs/heads/')) {
          entry.branch = line.slice('branch refs/heads/'.length);
        } else if (line === 'prunable' || line.startsWith('prunable ')) entry.prunable = true;
        else if (line === 'bare') entry.bare = true;
        else if (line === 'detached') entry.detached = true;
      }
      return entry;
    });
}

export function parseRemoteHeads(rawOutput) {
  if (!rawOutput || typeof rawOutput !== 'string') return [];

  const heads = [];
  for (const line of rawOutput.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const [sha, ref] = trimmed.split(/\s+/);
    if (!sha || !ref || !ref.startsWith('refs/heads/')) continue;
    heads.push({ sha, branch: ref.slice('refs/heads/'.length) });
  }
  return heads;
}

export function parseCliArgs(argv) {
  let apply = false;
  let help = false;

  for (const arg of argv) {
    if (arg === '--apply') {
      apply = true;
    } else if (arg === '--help' || arg === '-h') {
      help = true;
    } else {
      throw new Error(`Unknown argument: ${arg}`);
    }
  }

  return { apply, help };
}

export function cleanupMergedWorktrees({
  apply = false,
  run = defaultRun,
  log = console.log,
} = {}) {
  const mergedPrs = JSON.parse(
    run('gh', [
      'pr',
      'list',
      '--state',
      'merged',
      '--limit',
      '200',
      '--json',
      'number,headRefName,headRefOid',
    ]),
  );

  const mergedByBranch = new Map();
  for (const pr of mergedPrs) {
    if (!pr?.headRefName) continue;
    const list = mergedByBranch.get(pr.headRefName) ?? [];
    list.push(pr);
    mergedByBranch.set(pr.headRefName, list);
  }

  const openPrs = JSON.parse(
    run('gh', ['pr', 'list', '--state', 'open', '--limit', '100', '--json', 'headRefName']),
  );
  const openBranches = new Set(openPrs.map((pr) => pr?.headRefName).filter(Boolean));

  const worktrees = parseWorktreeList(run('git', ['worktree', 'list', '--porcelain']));
  const remoteHeads = parseRemoteHeads(run('git', ['ls-remote', '--heads', 'origin']));

  log(`Mode: ${apply ? 'APPLY' : 'DRY-RUN (pass --apply to execute)'}`);

  // 1. Prune stale remote-tracking refs and prunable worktree metadata
  log('\n[1] Remote-tracking refs & prunable worktrees:');
  log('  - git fetch --prune origin');
  log('  - git worktree prune');
  if (apply) {
    run('git', ['fetch', '--prune', 'origin']);
    run('git', ['worktree', 'prune']);
  }

  // 2. Inspect active worktrees whose branch matches a MERGED PR (never main, open PR, or dirty)
  const removed = [];
  const skippedDirty = [];
  const skippedHeadMismatch = [];

  log('\n[2] Candidate merged worktrees:');
  for (const wt of worktrees) {
    if (wt.prunable || wt.bare || !wt.path || !wt.branch || wt.branch === 'main') continue;
    if (openBranches.has(wt.branch)) continue;

    const mergedCandidates = mergedByBranch.get(wt.branch);
    if (!mergedCandidates || mergedCandidates.length === 0) continue;

    const dirty = run('git', ['-C', wt.path, 'status', '--porcelain']);
    if (dirty) {
      skippedDirty.push({ path: wt.path, branch: wt.branch, prNumber: mergedCandidates[0].number });
      log(`  SKIP (dirty): ${wt.path} [${wt.branch}] (PR #${mergedCandidates[0].number})`);
      continue;
    }

    const matchingPr = mergedCandidates.find(
      (pr) => Boolean(wt.head) && Boolean(pr.headRefOid) && wt.head === pr.headRefOid,
    );
    if (!matchingPr) {
      const refPr = mergedCandidates[0];
      skippedHeadMismatch.push({
        path: wt.path,
        branch: wt.branch,
        head: wt.head,
        expectedHead: refPr.headRefOid,
        prNumber: refPr.number,
      });
      log(
        `  SKIP (HEAD ${String(wt.head).slice(0, 7)} != merged PR #${refPr.number} ${String(refPr.headRefOid).slice(0, 7)}): ${wt.path} [${wt.branch}]`,
      );
      continue;
    }

    removed.push({
      path: wt.path,
      branch: wt.branch,
      head: wt.head,
      prNumber: matchingPr.number,
    });
    log(
      `  ${apply ? 'REMOVE' : 'WOULD REMOVE'}: ${wt.path} [${wt.branch}] (verified merged PR #${matchingPr.number} @ ${matchingPr.headRefOid.slice(0, 7)})`,
    );
    if (apply) {
      run('git', ['worktree', 'remove', wt.path]);
      run('git', ['branch', '-D', wt.branch]);
    }
  }

  // 3. List (never delete) remote branches without an open PR
  const remoteBranchesWithoutOpenPr = remoteHeads
    .filter(({ branch }) => branch !== 'main' && !openBranches.has(branch))
    .map(({ branch, sha }) => {
      const merged = mergedByBranch.get(branch)?.[0] ?? null;
      return {
        branch,
        sha,
        mergedPrNumber: merged?.number ?? null,
      };
    });

  log('\n[3] Remote branches without an open PR (list only, never deleted):');
  if (remoteBranchesWithoutOpenPr.length === 0) {
    log('  (none)');
  } else {
    for (const remote of remoteBranchesWithoutOpenPr) {
      const note =
        remote.mergedPrNumber !== null
          ? `merged PR #${remote.mergedPrNumber}`
          : 'no matching merged PR in recent list';
      log(`  LIST: origin/${remote.branch} (${remote.sha.slice(0, 7)}, ${note})`);
    }
  }

  return {
    mode: apply ? 'apply' : 'dry-run',
    removed,
    skippedDirty,
    skippedHeadMismatch,
    remoteBranchesWithoutOpenPr,
  };
}

function main() {
  const { apply, help } = parseCliArgs(process.argv.slice(2));
  if (help) {
    console.log(
      'Usage: node scripts/cleanup-merged-worktrees.mjs [--apply]\n\n' +
        'Dry-run by default. Pass --apply to prune stale refs/worktrees and remove verified merged worktrees and their local branches.\n' +
        'Remote branches without an open PR are listed only and never deleted.',
    );
    return;
  }
  cleanupMergedWorktrees({ apply });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
