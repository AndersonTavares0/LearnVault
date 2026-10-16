// Testes do gate local `pnpm verify` (scripts/verify.mjs).
//
// Cobrem duas garantias que o atalho do Windows (`cmd.exe verify`, saída 0)
// tornaria silenciosamente falsas: o comando executa os checks pretendidos e
// falha com código não zero quando um passo falha ou não pode rodar.
//
// Execução: `pnpm test:scripts` (ou `node --test scripts/verify.test.mjs`).

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

// spawn falso: registra as chamadas e resolve cada filho com o resultado pedido.
function fakeSpawn(results) {
	const calls = [];
	const spawnImpl = (command, args) => {
		const index = calls.length;
		calls.push([command, ...args]);
		const { code = 0, error } = results[index] ?? {};
		const child = new EventEmitter();
		// Emitir em microtask garante que runSteps já anexou os listeners.
		queueMicrotask(() => {
			if (error) {
				child.emit('error', error);
			} else {
				child.emit('close', code, null);
			}
		});
		return child;
	};
	return { spawnImpl, calls };
}

test('o plano padrão roda os checks pretendidos, sem Go nem desktop', () => {
	const labels = buildSteps().map((step) => step.label);

	assert.equal(labels[0], 'pnpm run check');
	assert.ok(labels.includes('pnpm --filter learnvault-web test'));
	assert.ok(labels.includes('cargo fmt --all -- --check'));
	assert.ok(labels.includes('cargo clippy --all-targets -- -D warnings'));
	assert.ok(labels.includes('cargo test'));
	assert.ok(labels.every((label) => !label.startsWith('go ')));
	assert.ok(labels.every((label) => !label.includes('learnvault-desktop')));
});

test('--fast mantém check, testes e lints, e remove builds e audit', () => {
	const labels = buildSteps({ fast: true }).map((step) => step.label);

	assert.ok(labels.includes('pnpm run check'));
	assert.ok(labels.includes('pnpm --filter learnvault-web test'));
	assert.ok(labels.includes('cargo test'));
	assert.ok(!labels.includes('pnpm run build'));
	assert.ok(!labels.includes('pnpm run build:desktop'));
	assert.ok(!labels.includes('pnpm audit --audit-level=low'));
});

test('runSteps devolve 0 e executa na ordem quando tudo passa', async () => {
	const plan = buildSteps({ fast: true });
	const { spawnImpl, calls } = fakeSpawn(plan.map(() => ({ code: 0 })));

	const code = await runSteps(plan, { spawnImpl, log() {}, errorLog() {} });

	assert.equal(code, 0);
	assert.equal(calls.length, plan.length);
	assert.deepEqual(calls[0], ['pnpm', 'run', 'check']);
});

test('runSteps devolve não zero e para no primeiro passo com falha', async () => {
	const plan = buildSteps({ fast: true });
	const { spawnImpl, calls } = fakeSpawn([{ code: 0 }, { code: 2 }]);

	const code = await runSteps(plan, { spawnImpl, log() {}, errorLog() {} });

	assert.equal(code, 1);
	// Falhou no segundo passo: o terceiro não deve rodar.
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
	assert.match(result.stdout, /^pnpm --filter learnvault-web test$/m);
});

test('pnpm verify resolve para o script do projeto (nunca o builtin do cmd.exe)', () => {
	// No Windows `pnpm` é um .cmd; chamar via ComSpec evita `shell: true` e o
	// aviso de depreciação DEP0190 sem reintroduzir o builtin `verify`.
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
