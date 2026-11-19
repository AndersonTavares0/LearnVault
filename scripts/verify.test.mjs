// Testes do gate local `pnpm verify` (scripts/verify.mjs).
//
// They cover two guarantees that the Windows shortcut (`cmd.exe verify`, exit 0)
// would silently make false: the command runs the intended checks and
// fails with a non-zero code when a step fails or cannot run.
//
// Run: `pnpm test:scripts` (or `node --test scripts/verify.test.mjs`).

import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { EventEmitter } from 'node:events';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { buildSteps, runSteps } from './verify.mjs';

const scriptsDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(scriptsDir, '..');

// fake spawn: records the calls and resolves each child with the requested result.
function fakeSpawn(results) {
	const calls = [];
	const optionsList = [];
	const spawnImpl = (command, args, options) => {
		const index = calls.length;
		calls.push([command, ...args]);
		optionsList.push(options);
		const { code = 0, error } = results[index] ?? {};
		const child = new EventEmitter();
		// Emitting in a microtask guarantees that runSteps has already attached the listeners.
		queueMicrotask(() => {
			if (error) {
				child.emit('error', error);
			} else {
				child.emit('close', code, null);
			}
		});
		return child;
	};
	return { spawnImpl, calls, optionsList };
}

test('the default plan runs the intended checks, without Go or desktop', () => {
	const labels = buildSteps().map((step) => step.label);

	assert.equal(labels[0], 'pnpm run check');
	assert.ok(labels.includes('pnpm --filter learnvault-web-next lint'));
	assert.ok(labels.includes('pnpm --filter learnvault-web-next test'));
	assert.ok(labels.includes('cargo fmt --all -- --check'));
	assert.ok(labels.includes('cargo clippy --all-targets -- -D warnings'));
	assert.ok(labels.includes('cargo test'));
	assert.ok(labels.every((label) => !label.startsWith('go ')));
	assert.ok(labels.every((label) => !label.includes('learnvault-desktop')));
});

test('--fast keeps check, tests and lints, and removes builds and audit', () => {
	const labels = buildSteps({ fast: true }).map((step) => step.label);

	assert.ok(labels.includes('pnpm run check'));
	assert.ok(labels.includes('cargo test'));
	assert.ok(!labels.includes('pnpm run build'));
	assert.ok(!labels.includes('pnpm run build:desktop'));
	assert.ok(!labels.includes('pnpm audit --audit-level=low'));
});

test('full plan reuses prebuilt SPA outputs in Vitest while --fast leaves Vitest to build them', async () => {
	const fullPlan = buildSteps({ fast: false });
	const fullTestIndex = fullPlan.findIndex((step) => step.label === 'pnpm --filter learnvault-web-next test');
	const fullBuildIndex = fullPlan.findIndex((step) => step.label === 'pnpm run build');
	const fullDesktopBuildIndex = fullPlan.findIndex((step) => step.label === 'pnpm run build:desktop');

	assert.ok(fullBuildIndex >= 0 && fullDesktopBuildIndex >= 0, 'full plan must build both SPA targets');
	assert.ok(fullBuildIndex < fullTestIndex && fullDesktopBuildIndex < fullTestIndex, 'both builds must run before Vitest');
	assert.deepEqual(fullPlan[fullTestIndex].env, { LEARNVAULT_PREBUILT_OUTPUTS: '1' });

	const { spawnImpl, optionsList } = fakeSpawn(fullPlan.map(() => ({ code: 0 })));
	const code = await runSteps(fullPlan, { spawnImpl, log() {}, errorLog() {} });
	assert.equal(code, 0);
	assert.equal(optionsList[fullTestIndex]?.env?.LEARNVAULT_PREBUILT_OUTPUTS, '1');

	const fastPlan = buildSteps({ fast: true });
	const fastTestStep = fastPlan.find((step) => step.label === 'pnpm --filter learnvault-web-next test');
	assert.equal(fastTestStep?.env, undefined, '--fast skips pre-builds, so Vitest must not assume prebuilt outputs');
});

test('runSteps returns 0 and runs in order when everything passes', async () => {
	const plan = buildSteps({ fast: true });
	const { spawnImpl, calls } = fakeSpawn(plan.map(() => ({ code: 0 })));

	const code = await runSteps(plan, { spawnImpl, log() {}, errorLog() {} });

	assert.equal(code, 0);
	assert.equal(calls.length, plan.length);
	assert.deepEqual(calls[0], ['pnpm', 'run', 'check']);
});

test('runSteps returns non-zero and stops at the first failing step', async () => {
	const plan = buildSteps({ fast: true });
	const { spawnImpl, calls } = fakeSpawn([{ code: 0 }, { code: 2 }]);

	const code = await runSteps(plan, { spawnImpl, log() {}, errorLog() {} });

	assert.equal(code, 1);
	// It failed on the second step: the third must not run.
	assert.equal(calls.length, 2);
});

test('ferramenta ausente (erro de spawn) falha em vez de travar', async () => {
	const plan = buildSteps({ fast: true });
	const error = Object.assign(new Error('spawn pnpm ENOENT'), { code: 'ENOENT' });
	const { spawnImpl } = fakeSpawn([{ error }]);

	const code = await runSteps(plan, { spawnImpl, log() {}, errorLog() {} });

	assert.equal(code, 1);
});

test('package.json liga pnpm verify ao script real', () => {
	const pkg = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));

	assert.equal(pkg.scripts.verify, 'node scripts/verify.mjs');
	assert.match(pkg.scripts['test:scripts'], /node --test/);
});

test('node scripts/verify.mjs --list anuncia os comandos pretendidos', () => {
	const result = spawnSync(process.execPath, [path.join(scriptsDir, 'verify.mjs'), '--list'], {
		cwd: root,
		encoding: 'utf8'
	});

	assert.equal(result.status, 0, result.stderr);
	assert.match(result.stdout, /^pnpm run check$/m);
	assert.match(result.stdout, /^pnpm --filter learnvault-web-next test$/m);
});

test('pnpm verify resolves to the project script (never the cmd.exe builtin)', () => {
	// On Windows `pnpm` is a .cmd; calling it through ComSpec avoids `shell: true` and the
	// DEP0190 deprecation warning without reintroducing the `verify` builtin.
	const isWindows = process.platform === 'win32';
	const command = isWindows ? process.env.ComSpec : 'pnpm';
	const args = isWindows
		? ['/d', '/s', '/c', 'pnpm', 'run', 'verify', '--list']
		: ['run', 'verify', '--list'];
	const result = spawnSync(command, args, { cwd: root, encoding: 'utf8' });

	assert.equal(result.status, 0, result.stderr);
	assert.match(result.stdout, /pnpm run check/);
	assert.doesNotMatch(result.stdout, /VERIFY is (on|off)/i);
});
