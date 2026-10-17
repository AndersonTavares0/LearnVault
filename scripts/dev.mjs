// Atalhos de desenvolvimento do LearnVault.
//
// Uso: node scripts/dev.mjs [localhost|tauri]
//   localhost (padrão): servidor Rust + Vite com proxy de `/api`.
//   tauri:             rebuild contínuo do frontend desktop + aplicativo Tauri offline.

import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const isWindows = process.platform === 'win32';
const mode = process.argv[2] ?? 'localhost';

if (!['localhost', 'tauri'].includes(mode)) {
	console.error(`Modo desconhecido: ${mode}. Use "localhost" (padrão) ou "tauri".`);
	process.exit(1);
}

const children = new Set();
let exiting = false;

function run(command, args, options = {}) {
	const child = spawn(command, args, {
		cwd: root,
		stdio: 'inherit',
		shell: isWindows,
		detached: !isWindows,
		...options
	});
	children.add(child);
	child.on('exit', (code, signal) => {
		children.delete(child);
		if (exiting) return;
		console.error(`\nUm processo encerrou (${signal ?? `código ${code}`}); encerrando os demais.`);
		shutdown(code ?? 1);
	});
	return child;
}

function kill(child) {
	if (child.exitCode !== null || child.signalCode !== null) return;
	try {
		if (isWindows) {
			spawnSync('taskkill', ['/pid', String(child.pid), '/T', '/F'], { stdio: 'ignore' });
		} else {
			process.kill(-child.pid, 'SIGTERM');
		}
	} catch {
		child.kill('SIGTERM');
	}
}

function shutdown(code) {
	if (exiting) return;
	exiting = true;
	for (const child of children) kill(child);
	setTimeout(() => process.exit(code), 200);
}

process.on('SIGINT', () => shutdown(0));
process.on('SIGTERM', () => shutdown(0));

if (mode === 'localhost') {
	const listenAddr = process.env.LEARNVAULT_LISTEN_ADDR ?? '127.0.0.1:8090';
	console.log('LearnVault em desenvolvimento:');
	console.log(`  API (Rust):  http://${listenAddr}`);
	console.log('  SPA (Vite):  http://localhost:5173');
	console.log('  Sem configuração, o primeiro acesso usa admin / admin.');
	console.log('  A primeira execução compila o servidor e pode demorar.\n');
	run('cargo', [
		'run', '-p', 'learnvault-server', '--',
		'--listen-addr', listenAddr,
		'--insecure-http'
	]);
	run('pnpm', ['--filter', 'learnvault-web', 'dev'], {
		env: {
			...process.env,
			LEARNVAULT_API_TARGET: process.env.LEARNVAULT_API_TARGET ?? `http://${listenAddr}`
		}
	});
} else {
	const shell = path.join(root, 'crates', 'desktop', 'frontend-dist', '200.html');
	const tauri = path.join(root, 'node_modules', '.bin', isWindows ? 'tauri.cmd' : 'tauri');

	if (!existsSync(tauri)) {
		console.error('Tauri CLI não encontrado. Rode "pnpm install" na raiz do repositório.');
		process.exit(1);
	}

	console.log('LearnVault desktop (Tauri), sem portas de rede.');
	console.log('O primeiro build compila o crate desktop; edições da SPA recarregam sozinhas.');
	console.log('Primeiro uso: use "Importar curso" e escolha a pasta de um curso.\n');

	run('pnpm', ['--filter', 'learnvault-web', 'dev:tauri']);

	console.log('Aguardando o build inicial do frontend desktop...');
	const deadline = Date.now() + 120_000;
	while (!existsSync(shell)) {
		if (Date.now() > deadline) {
			console.error('O frontend desktop não foi gerado em 2 minutos.');
			shutdown(1);
			break;
		}
		await delay(500);
	}

	if (!exiting) {
		run(`"${tauri}"`, ['dev'], { cwd: path.join(root, 'crates', 'desktop') });
	}
}
