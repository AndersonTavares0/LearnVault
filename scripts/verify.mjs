// Checks de PR da SPA e do Rust em um único comando.
//
// Uso: node scripts/verify.mjs [--fast] [--list]
//   --fast  pula os builds e o audit, mantendo type-check, testes e lints.
//   --list  imprime os comandos que seriam executados e sai sem rodar nada.
//
// Este script é o destino de `pnpm verify`. O alias precisa existir porque no
// Windows `verify` é um comando interno do cmd.exe que sai com código 0 sem
// verificar nada; sem a entrada em package.json o `pnpm verify` cairia nele e
// reportaria sucesso falso. Falhas de spawn (ferramenta ausente) também viram
// erro explícito, nunca sucesso silencioso.
//
// O crate desktop fica fora dos membros padrão do workspace; use `pnpm dev:tauri`
// ou `cargo test -p learnvault-desktop` quando a mudança tocar o desktop.

import { spawn } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)));

export function buildSteps({ fast = false } = {}) {
	const steps = [
		{ label: 'pnpm run check', command: 'pnpm', args: ['run', 'check'] },
		{ label: 'pnpm run build', command: 'pnpm', args: ['run', 'build'], slow: true },
		{ label: 'pnpm run build:desktop', command: 'pnpm', args: ['run', 'build:desktop'], slow: true },
		{ label: 'pnpm --filter learnvault-web test', command: 'pnpm', args: ['--filter', 'learnvault-web', 'test'] },
		{ label: 'pnpm audit --audit-level=low', command: 'pnpm', args: ['audit', '--audit-level=low', '--ignore', 'GHSA-gcqx-w48m-w2h4'], slow: true },
		{ label: 'cargo fmt --all -- --check', command: 'cargo', args: ['fmt', '--all', '--', '--check'] },
		{ label: 'cargo clippy --all-targets -- -D warnings', command: 'cargo', args: ['clippy', '--all-targets', '--', '-D', 'warnings'] },
		{ label: 'cargo test', command: 'cargo', args: ['test'] }
	];

	return fast ? steps.filter((step) => !step.slow) : steps;
}

// Executa os passos em sequência e devolve o código de saída (0 = tudo passou).
// Interrompe no primeiro erro e nunca devolve 0 quando um passo não roda.
export async function runSteps(steps, {
	cwd = root,
	spawnImpl = spawn,
	log = console.log,
	errorLog = console.error
} = {}) {
	for (const [index, step] of steps.entries()) {
		log(`[${index + 1}/${steps.length}] ${step.label}`);
		const code = await new Promise((resolve) => {
			const child = spawnImpl(step.command, step.args, {
				cwd,
				stdio: 'inherit',
				shell: process.platform === 'win32'
			});
			// Ferramenta ausente (ENOENT) emite 'error' sem 'exit'; sem este tratador a
			// promessa nunca resolveria e `pnpm verify` travaria em vez de falhar.
			child.on('error', () => resolve(1));
			child.on('close', (exitCode, signal) => resolve(exitCode ?? (signal ? 1 : 0)));
		});

		if (code !== 0) {
			errorLog(`\nFalhou: ${step.label}\n`);
			return 1;
		}
	}

	return 0;
}

const isMain = process.argv[1] !== undefined
	&& path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isMain) {
	const args = process.argv.slice(2);
	const fast = args.includes('--fast');
	const steps = buildSteps({ fast });

	if (args.includes('--list')) {
		for (const step of steps) {
			console.log(step.label);
		}
		process.exit(0);
	}

	console.log(`Verificação LearnVault (${fast ? 'rápida: sem builds e sem audit' : 'completa'})`);
	console.log('Go e desktop ficam de fora; use os comandos dedicados quando a mudança os tocar.\n');

	const code = await runSteps(steps);

	if (code === 0) {
		console.log('\nTodos os checks passaram.');
	}
	process.exit(code);
}
