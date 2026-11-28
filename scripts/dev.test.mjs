import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

function fixture(t) {
	const root = mkdtempSync(path.join(tmpdir(), 'learnvault dev '));
	t.after(() => rmSync(root, { recursive: true, force: true }));
	const scripts = path.join(root, 'scripts');
	const bin = path.join(root, 'node_modules', '.bin');
	mkdirSync(scripts);
	mkdirSync(bin, { recursive: true });
	copyFileSync(new URL('./dev.mjs', import.meta.url), path.join(scripts, 'dev.mjs'));
	writeFileSync(path.join(bin, 'pnpm'), '#!/bin/sh\nexec /bin/sleep 1\n', { mode: 0o755 });
	return { root, bin, script: path.join(scripts, 'dev.mjs') };
}

test('desktop development starts the native CLI from a path with spaces', { skip: process.platform === 'win32' }, (t) => {
	const { root, bin, script } = fixture(t);
	const output = path.join(root, 'crates', 'desktop', 'frontend-dist');
	mkdirSync(output, { recursive: true });
	writeFileSync(path.join(output, '200.html'), '');
	writeFileSync(path.join(bin, 'tauri'), '#!/bin/sh\nprintf "tauri %s\\n" "$1"\n', { mode: 0o755 });

	const result = spawnSync(process.execPath, [script, 'tauri'], {
		env: { ...process.env, PATH: bin },
		encoding: 'utf8',
		timeout: 4_000
	});
	assert.equal(result.status, 0, result.stderr);
	assert.match(result.stdout, /tauri dev/);
});

test('development reports a missing executable without an unhandled error', { skip: process.platform === 'win32' }, (t) => {
	const { bin, script } = fixture(t);
	const result = spawnSync(process.execPath, [script, 'localhost'], {
		env: { ...process.env, PATH: bin },
		encoding: 'utf8',
		timeout: 4_000
	});
	assert.equal(result.status, 1);
	assert.match(result.stderr, /Could not start cargo.*ENOENT/);
	assert.doesNotMatch(result.stderr, /Unhandled 'error'/);
});
