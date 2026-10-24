#!/usr/bin/env node
// Cada worktree ganha seu próprio target/. Sem coleta, o cache do Cargo cresce
// vários GB por worktree. Este script apaga somente o build (regenerável).
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, rmSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export function parseWorktrees(porcelain) {
	return porcelain
		.trim()
		.split(/\r?\n\r?\n/)
		.map((block) => block.split(/\r?\n/).find((line) => line.startsWith('worktree '))?.slice('worktree '.length))
		.filter(Boolean)
		.map((path) => resolve(path));
}

export function formatBytes(bytes) {
	if (bytes < 1024) return `${bytes} B`;
	const units = ['KB', 'MB', 'GB', 'TB'];
	let value = bytes / 1024;
	let unit = 0;
	while (value >= 1024 && unit < units.length - 1) {
		value /= 1024;
		unit += 1;
	}
	return `${value.toFixed(1)} ${units[unit]}`;
}

function dirSize(dir) {
	let total = 0;
	const stack = [dir];
	while (stack.length) {
		const current = stack.pop();
		let entries;
		try {
			entries = readdirSync(current, { withFileTypes: true });
		} catch {
			continue;
		}
		for (const entry of entries) {
			const path = join(current, entry.name);
			if (entry.isDirectory()) stack.push(path);
			else if (entry.isFile()) {
				try {
					total += statSync(path).size;
				} catch {
					// Arquivo removido durante a varredura: ignore.
				}
			}
		}
	}
	return total;
}

function buildRunning() {
	try {
		if (process.platform === 'win32') {
			const output = execFileSync('tasklist', ['/NH'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
			return /^(cargo|rustc)\.exe/im.test(output);
		}
		execFileSync('pgrep', ['-x', 'cargo'], { stdio: 'ignore' });
		return true;
	} catch {
		return false;
	}
}

function main() {
	const dryRun = process.argv.includes('--dry-run');
	const root = resolve(execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim());
	const porcelain = execFileSync('git', ['worktree', 'list', '--porcelain'], { cwd: root, encoding: 'utf8' });
	const linked = parseWorktrees(porcelain).filter((path) => path !== root);
	if (buildRunning()) {
		console.error('cargo/rustc está em execução. Aguarde o build terminar.');
		process.exitCode = 1;
		return;
	}
	let freed = 0;
	let removed = 0;
	for (const path of linked) {
		const target = join(path, 'target');
		if (!existsSync(target)) continue;
		const bytes = dirSize(target);
		if (!dryRun) rmSync(target, { recursive: true, force: true, maxRetries: 3 });
		freed += bytes;
		removed += 1;
		console.log(`${dryRun ? 'removeria' : 'removido'} ${formatBytes(bytes)}  ${path}`);
	}
	console.log(`Total ${dryRun ? 'liberável' : 'liberado'}: ${formatBytes(freed)} em ${removed} worktree(s).`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main();
