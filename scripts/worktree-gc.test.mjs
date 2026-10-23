import assert from 'node:assert/strict';
import { test } from 'node:test';
import { formatBytes, parseWorktrees } from './worktree-gc.mjs';

test('parseWorktrees lê o caminho principal e os worktrees vinculados', () => {
	const porcelain = [
		'worktree C:/repo/LearnVault',
		'HEAD abc123',
		'branch refs/heads/main',
		'',
		'worktree C:/repo/outra copia',
		'HEAD def456',
		'branch refs/heads/feat/x',
		''
	].join('\n');
	assert.deepEqual(parseWorktrees(porcelain), [
		'C:\\repo\\LearnVault',
		'C:\\repo\\outra copia'
	]);
});

test('parseWorktrees ignora blocos sem caminho', () => {
	assert.deepEqual(parseWorktrees('\n\n'), []);
});

test('formatBytes usa unidade legível', () => {
	assert.equal(formatBytes(512), '512 B');
	assert.equal(formatBytes(1024), '1.0 KB');
	assert.equal(formatBytes(1024 ** 3 * 2.5), '2.5 GB');
});
