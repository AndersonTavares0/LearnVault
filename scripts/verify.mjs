// Checks de PR da SPA e do Rust em um único comando.
//
// Uso: node scripts/verify.mjs [--fast]
//   --fast pula os builds e o audit, mantendo type-check, testes e lints.
//
// O crate desktop fica fora dos membros padrão do workspace; use `pnpm dev:tauri`
// ou `cargo test -p learnvault-desktop` quando a mudança tocar o desktop.
//
// Checks de Go não entram aqui; use `go -C services/go test ./...` e
// `go -C services/go vet ./...` quando a mudança tocar o módulo de referência.

import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const fast = process.argv.includes('--fast');

const steps = [
	{ label: 'pnpm run check', command: 'pnpm', args: ['run', 'check'] },
	{ label: 'pnpm run build', command: 'pnpm', args: ['run', 'build'], slow: true },
	{ label: 'pnpm run build:desktop', command: 'pnpm', args: ['run', 'build:desktop'], slow: true },
	{ label: 'pnpm --filter learnvault-web test', command: 'pnpm', args: ['--filter', 'learnvault-web', 'test'] },
	{ label: 'pnpm audit --audit-level=low', command: 'pnpm', args: ['audit', '--audit-level=low'], slow: true },
	{ label: 'cargo fmt --all -- --check', command: 'cargo', args: ['fmt', '--all', '--', '--check'] },
	{ label: 'cargo clippy --all-targets -- -D warnings', command: 'cargo', args: ['clippy', '--all-targets', '--', '-D', 'warnings'] },
	{ label: 'cargo test', command: 'cargo', args: ['test'] }
];

const selected = fast ? steps.filter((step) => !step.slow) : steps;

console.log(`Verificação LearnVault (${fast ? 'rápida: sem builds e sem audit' : 'completa'})`);
console.log('Go e desktop ficam de fora; use os comandos dedicados quando a mudança os tocar.\n');

for (const [index, step] of selected.entries()) {
	console.log(`[${index + 1}/${selected.length}] ${step.label}`);
	const code = await new Promise((resolve) => {
		const child = spawn(step.command, step.args, {
			cwd: root,
			stdio: 'inherit',
			shell: process.platform === 'win32'
		});
		child.on('exit', (exitCode, signal) => resolve(exitCode ?? (signal ? 1 : 0)));
	});

	if (code !== 0) {
		console.error(`\nFalhou: ${step.label}\n`);
		process.exit(1);
	}
}

console.log('\nTodos os checks passaram.');
