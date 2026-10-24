import assert from 'node:assert/strict';
import { basename, isAbsolute } from 'node:path';
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
	const paths = parseWorktrees(porcelain);
	// `resolve` muda o prefixo por plataforma; o nome final é o que importa aqui.
	assert.ok(paths.every((path) => isAbsolute(path)));
	assert.deepEqual(paths.map((path) => basename(path)), ['LearnVault', 'outra copia']);
});

test('parseWorktrees ignora blocos sem caminho', () => {
	assert.deepEqual(parseWorktrees('\n\n'), []);
});

test('formatBytes usa unidade legível', () => {
	assert.equal(formatBytes(512), '512 B');
	assert.equal(formatBytes(1024), '1.0 KB');
	assert.equal(formatBytes(1024 ** 3 * 2.5), '2.5 GB');
});
