import assert from 'node:assert/strict';
import test from 'node:test';

import {
  cleanupMergedWorktrees,
  parseCliArgs,
  parseRemoteHeads,
  parseWorktreeList,
} from './cleanup-merged-worktrees.mjs';

function makeStubRunner({
  mergedPrs = [],
  openPrs = [],
  worktreePorcelain = '',
  remoteHeads = '',
  dirtyByPath = {},
} = {}) {
  const calls = [];

  function run(cmd, args) {
    calls.push([cmd, ...args]);

    if (cmd === 'gh' && args[0] === 'pr' && args[1] === 'list') {
      if (args.includes('merged')) return JSON.stringify(mergedPrs);
      if (args.includes('open')) return JSON.stringify(openPrs);
    }

    if (cmd === 'git' && args[0] === 'worktree' && args[1] === 'list') {
      return worktreePorcelain;
    }

    if (cmd === 'git' && args[0] === 'ls-remote') {
      return remoteHeads;
    }

    if (cmd === 'git' && args[0] === '-C' && args[2] === 'status' && args[3] === '--porcelain') {
      return dirtyByPath[args[1]] ?? '';
    }

    return '';
  }

  return { run, calls };
}

test('parseWorktreeList parses active, detached, bare, and prunable entries', () => {
  const raw = [
    'worktree /repo/main',
    'HEAD 1111111111111111111111111111111111111111',
    'branch refs/heads/main',
    '',
    'worktree /repo/wt-merged',
    'HEAD 2222222222222222222222222222222222222222',
    'branch refs/heads/fix/old-bug',
    '',
    'worktree /tmp/missing',
    'HEAD 3333333333333333333333333333333333333333',
    'branch refs/heads/chore/gone',
    'prunable gitdir file points to non-existent location',
    '',
    'worktree /repo/detached',
    'HEAD 4444444444444444444444444444444444444444',
    'detached',
  ].join('\n');

  assert.deepEqual(parseWorktreeList(raw), [
    {
      path: '/repo/main',
      head: '1111111111111111111111111111111111111111',
      branch: 'main',
      prunable: false,
      bare: false,
      detached: false,
    },
    {
      path: '/repo/wt-merged',
      head: '2222222222222222222222222222222222222222',
      branch: 'fix/old-bug',
      prunable: false,
      bare: false,
      detached: false,
    },
    {
      path: '/tmp/missing',
      head: '3333333333333333333333333333333333333333',
      branch: 'chore/gone',
      prunable: true,
      bare: false,
      detached: false,
    },
    {
      path: '/repo/detached',
      head: '4444444444444444444444444444444444444444',
      branch: null,
      prunable: false,
      bare: false,
      detached: true,
    },
  ]);
});

test('parseRemoteHeads extracts branch names and SHAs from ls-remote output', () => {
  const raw = [
    'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\trefs/heads/main',
    'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\trefs/heads/feat/orphan',
    'cccccccccccccccccccccccccccccccccccccccc\trefs/tags/v0.9.4',
  ].join('\n');

  assert.deepEqual(parseRemoteHeads(raw), [
    { sha: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', branch: 'main' },
    { sha: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', branch: 'feat/orphan' },
  ]);
});

test('parseCliArgs defaults to dry-run and rejects unknown flags', () => {
  assert.deepEqual(parseCliArgs([]), { apply: false, help: false });
  assert.deepEqual(parseCliArgs(['--apply']), { apply: true, help: false });
  assert.deepEqual(parseCliArgs(['--help']), { apply: false, help: true });
  assert.throws(() => parseCliArgs(['--force']), /Unknown argument: --force/);
});

test('dry-run by default never executes mutating git commands and lists orphan remote branches', () => {
  const fixture = {
    mergedPrs: [
      { number: 601, headRefName: 'fix/merged-clean', headRefOid: 'aaaaaaa111111111111111111111111111111111' },
      { number: 602, headRefName: 'fix/merged-dirty', headRefOid: 'bbbbbbb222222222222222222222222222222222' },
      { number: 603, headRefName: 'fix/merged-ahead', headRefOid: 'ccccccc333333333333333333333333333333333' },
      { number: 604, headRefName: 'feat/reused-open', headRefOid: 'ddddddd444444444444444444444444444444444' },
    ],
    openPrs: [{ headRefName: 'feat/reused-open' }, { headRefName: 'feat/active-pr' }],
    worktreePorcelain: [
      'worktree /repo/main',
      'HEAD 0000000000000000000000000000000000000000',
      'branch refs/heads/main',
      '',
      'worktree /repo/wt-clean',
      'HEAD aaaaaaa111111111111111111111111111111111',
      'branch refs/heads/fix/merged-clean',
      '',
      'worktree /repo/wt-dirty',
      'HEAD bbbbbbb222222222222222222222222222222222',
      'branch refs/heads/fix/merged-dirty',
      '',
      'worktree /repo/wt-ahead',
      'HEAD eeeeeee555555555555555555555555555555555',
      'branch refs/heads/fix/merged-ahead',
      '',
      'worktree /repo/wt-open',
      'HEAD ddddddd444444444444444444444444444444444',
      'branch refs/heads/feat/reused-open',
    ].join('\n'),
    remoteHeads: [
      '0000000000000000000000000000000000000000\trefs/heads/main',
      'ddddddd444444444444444444444444444444444\trefs/heads/feat/reused-open',
      'fffffff666666666666666666666666666666666\trefs/heads/feat/active-pr',
      'aaaaaaa111111111111111111111111111111111\trefs/heads/fix/merged-clean',
      '9999999999999999999999999999999999999999\trefs/heads/docs/orphan-remote',
    ].join('\n'),
    dirtyByPath: {
      '/repo/wt-dirty': ' M README.md',
    },
  };

  const { run, calls } = makeStubRunner(fixture);
  const logs = [];
  const result = cleanupMergedWorktrees({ run, log: (line) => logs.push(line) });

  assert.equal(result.mode, 'dry-run');
  assert.deepEqual(result.removed, [
    {
      path: '/repo/wt-clean',
      branch: 'fix/merged-clean',
      head: 'aaaaaaa111111111111111111111111111111111',
      prNumber: 601,
    },
  ]);
  assert.deepEqual(result.skippedDirty, [
    { path: '/repo/wt-dirty', branch: 'fix/merged-dirty', prNumber: 602 },
  ]);
  assert.deepEqual(result.skippedHeadMismatch, [
    {
      path: '/repo/wt-ahead',
      branch: 'fix/merged-ahead',
      head: 'eeeeeee555555555555555555555555555555555',
      expectedHead: 'ccccccc333333333333333333333333333333333',
      prNumber: 603,
    },
  ]);
  assert.deepEqual(result.remoteBranchesWithoutOpenPr, [
    {
      branch: 'fix/merged-clean',
      sha: 'aaaaaaa111111111111111111111111111111111',
      mergedPrNumber: 601,
    },
    {
      branch: 'docs/orphan-remote',
      sha: '9999999999999999999999999999999999999999',
      mergedPrNumber: null,
    },
  ]);

  assert.equal(
    calls.some(
      (c) =>
        (c[0] === 'git' && c[1] === 'fetch') ||
        (c[0] === 'git' && c[1] === 'worktree' && c[2] !== 'list') ||
        (c[0] === 'git' && c[1] === 'branch') ||
        (c[0] === 'git' && c[1] === 'push'),
    ),
    false,
    `Unexpected mutating call in dry-run: ${JSON.stringify(calls)}`,
  );
  assert.ok(logs.some((line) => line.includes('DRY-RUN')));
});

test('apply mode removes only verified clean merged worktrees and never deletes remote branches', () => {
  const { run, calls } = makeStubRunner({
    mergedPrs: [
      { number: 610, headRefName: 'fix/verified', headRefOid: '1234567890abcdef1234567890abcdef12345678' },
      { number: 611, headRefName: 'fix/dirty', headRefOid: 'abcdef1234567890abcdef1234567890abcdef12' },
      { number: 612, headRefName: 'fix/new-commits', headRefOid: '1111111111111111111111111111111111111111' },
      { number: 613, headRefName: 'main', headRefOid: '0000000000000000000000000000000000000000' },
    ],
    openPrs: [{ headRefName: 'feat/open' }],
    worktreePorcelain: [
      'worktree /repo/main',
      'HEAD 0000000000000000000000000000000000000000',
      'branch refs/heads/main',
      '',
      'worktree /repo/wt-verified',
      'HEAD 1234567890abcdef1234567890abcdef12345678',
      'branch refs/heads/fix/verified',
      '',
      'worktree /repo/wt-dirty',
      'HEAD abcdef1234567890abcdef1234567890abcdef12',
      'branch refs/heads/fix/dirty',
      '',
      'worktree /repo/wt-ahead',
      'HEAD 2222222222222222222222222222222222222222',
      'branch refs/heads/fix/new-commits',
      '',
      'worktree /repo/wt-open',
      'HEAD 3333333333333333333333333333333333333333',
      'branch refs/heads/feat/open',
      '',
      'worktree /repo/wt-unmerged',
      'HEAD 4444444444444444444444444444444444444444',
      'branch refs/heads/chore/local-only',
    ].join('\n'),
    remoteHeads: [
      '0000000000000000000000000000000000000000\trefs/heads/main',
      '3333333333333333333333333333333333333333\trefs/heads/feat/open',
      '5555555555555555555555555555555555555555\trefs/heads/chore/stale-remote',
    ].join('\n'),
    dirtyByPath: {
      '/repo/wt-dirty': '?? untracked.txt',
    },
  });

  const result = cleanupMergedWorktrees({ apply: true, run, log: () => {} });

  assert.equal(result.mode, 'apply');
  assert.deepEqual(
    result.removed.map((entry) => entry.branch),
    ['fix/verified'],
  );
  assert.deepEqual(result.remoteBranchesWithoutOpenPr, [
    {
      branch: 'chore/stale-remote',
      sha: '5555555555555555555555555555555555555555',
      mergedPrNumber: null,
    },
  ]);

  assert.deepEqual(
    calls.filter(
      (c) =>
        (c[0] === 'git' && c[1] === 'fetch') ||
        (c[0] === 'git' && c[1] === 'worktree' && c[2] !== 'list') ||
        (c[0] === 'git' && c[1] === 'branch') ||
        (c[0] === 'git' && c[1] === 'push'),
    ),
    [
      ['git', 'fetch', '--prune', 'origin'],
      ['git', 'worktree', 'prune'],
      ['git', 'worktree', 'remove', '/repo/wt-verified'],
      ['git', 'branch', '-D', 'fix/verified'],
    ],
  );
});
