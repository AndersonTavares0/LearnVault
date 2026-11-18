// LearnVault development shortcuts.
//
// Usage: node scripts/dev.mjs [localhost|tauri]
//   localhost (default): Rust server + Vite with a `/api` proxy.
//   tauri:             continuous rebuild of the desktop frontend + offline Tauri application.

import { spawn, spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';

const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)));
const isWindows = process.platform === 'win32';
const mode = process.argv[2] ?? 'localhost';

if (!['localhost', 'tauri'].includes(mode)) {
	console.error(`Unknown mode: ${mode}. Use "localhost" (default) or "tauri".`);
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
		console.error(`\nA process exited (${signal ?? `code ${code}`}); stopping the others.`);
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
	console.log('LearnVault in development:');
	console.log(`  API (Rust):  http://${listenAddr}`);
	console.log('  SPA (Vite):  http://localhost:5174');
	console.log('  With no configuration, the first sign-in uses admin / admin.');
	console.log('  The first run compiles the server and can take a while.\n');
	run('cargo', [
		'run', '-p', 'learnvault-server', '--',
		'--listen-addr', listenAddr,
		'--insecure-http'
	]);
	run('pnpm', ['--filter', 'learnvault-web-next', 'dev'], {
		env: {
			...process.env,
			LEARNVAULT_API_TARGET: process.env.LEARNVAULT_API_TARGET ?? `http://${listenAddr}`
		}
	});
} else {
	const shell = path.join(root, 'crates', 'desktop', 'frontend-dist', '200.html');
	const tauri = path.join(root, 'node_modules', '.bin', isWindows ? 'tauri.cmd' : 'tauri');

	if (!existsSync(tauri)) {
		console.error('Tauri CLI not found. Run "pnpm install" at the repository root.');
		process.exit(1);
	}

	console.log('LearnVault desktop (Tauri), no network ports.');
	console.log('The first build compiles the desktop crate; SPA edits reload on their own.');
	console.log('First use: choose "Import course" and pick a course folder.\n');

	run('pnpm', ['--filter', 'learnvault-web-next', 'dev:tauri']);

	console.log('Waiting for the initial desktop frontend build...');
	const deadline = Date.now() + 120_000;
	while (!existsSync(shell)) {
		if (Date.now() > deadline) {
			console.error('The desktop frontend was not generated within 2 minutes.');
			shutdown(1);
			break;
		}
		await delay(500);
	}

	if (!exiting) {
		run(`"${tauri}"`, ['dev'], { cwd: path.join(root, 'crates', 'desktop') });
	}
}
