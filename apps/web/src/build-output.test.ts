import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

/** Concatena todos os bundles JS sob `root` para inspeção textual. */
function readBundles(root: string): string {
	const contents: string[] = [];
	const walk = (directory: string) => {
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			const path = join(directory, entry.name);
			if (entry.isDirectory()) walk(path);
			else if (entry.name.endsWith('.js')) contents.push(readFileSync(path, 'utf8'));
		}
	};
	walk(root);
	return contents.join('\n');
}

describe('production shell contract', () => {
	it('runs the real build and emits the canonical server shell', () => {
		execFileSync('pnpm', ['run', 'build'], { cwd: process.cwd(), stdio: 'pipe', shell: true });
		const shell = resolve(process.cwd(), '../../crates/server/web/build/200.html');
		expect(existsSync(shell)).toBe(true);
		expect(readFileSync(shell, 'utf8')).toContain('/_app/immutable/');
	});
	it('runs the desktop build into the Tauri frontend directory', () => {
		execFileSync('pnpm', ['run', 'build:desktop'], { cwd: process.cwd(), stdio: 'pipe', shell: true });
		const shell = resolve(process.cwd(), '../../crates/desktop/frontend-dist/200.html');
		expect(existsSync(shell)).toBe(true);
		expect(readFileSync(shell, 'utf8')).toContain('/_app/immutable/');
	});
	it('ships a desktop bundle without any application fetch while keeping SRT conversion on HTTP', () => {
		execFileSync('pnpm', ['run', 'build:desktop'], { cwd: process.cwd(), stdio: 'pipe', shell: true });
		execFileSync('pnpm', ['run', 'build'], { cwd: process.cwd(), stdio: 'pipe', shell: true });

		const desktop = readBundles(resolve(process.cwd(), '../../crates/desktop/frontend-dist'));
		const http = readBundles(resolve(process.cwd(), '../../crates/server/web/build'));

		// O desktop é offline por contrato: nenhum caminho do aplicativo pode abrir rede.
		expect(desktop).not.toContain('fetch(');
		// O HTTP preserva a conversão SRT sob demanda (o único fetch de mídia do player).
		expect(http).toContain('fetch(');
		expect(http).toContain('Falha ao carregar legenda.');
		// A entrega de mídia no desktop usa o protocolo customizado por mediaId,
		// nunca o endpoint HTTP por caminho; o HTTP mantém `/api/media?course=`.
		expect(desktop).toContain('learnvault-media');
		expect(desktop).not.toContain('/api/media?course=');
		expect(http).toContain('/api/media?course=');
		expect(http).not.toContain('learnvault-media');
	}, 60000);
});
