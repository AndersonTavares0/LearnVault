import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

// A bare `fetch(` call, not a method such as TanStack Query's `query.fetch(` or its definition.
const NETWORK_FETCH = /(?<![\w$.#])fetch\((?![^)]*\)\s*\{)/;

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

function buildOutputsIfNeeded(...scripts: string[]): void {
	if (process.env.LEARNVAULT_PREBUILT_OUTPUTS === '1') return;
	for (const script of scripts) execFileSync('pnpm', ['run', script], { cwd: process.cwd(), stdio: 'pipe', shell: true });
}

describe('production shell contract', () => {
	it('emits both shells with immutable assets', () => {
		buildOutputsIfNeeded('build', 'build:desktop');
		for (const dir of ['dist', 'dist-desktop']) {
			const shell = resolve(process.cwd(), dir, '200.html');
			expect(existsSync(shell)).toBe(true);
			expect(readFileSync(shell, 'utf8')).toContain('/_app/immutable/');
		}
	}, 60_000);

	it('ships a desktop bundle with no network fetch and the custom media scheme', () => {
		buildOutputsIfNeeded('build', 'build:desktop');
		const desktop = readBundles(resolve(process.cwd(), 'dist-desktop'));
		const http = readBundles(resolve(process.cwd(), 'dist'));

		// The desktop app is offline by contract: no code path may open the network.
		expect(desktop).not.toMatch(NETWORK_FETCH);
		expect(http).toMatch(NETWORK_FETCH);
		expect(desktop).toContain('learnvault-media');
		expect(desktop).not.toContain('/api/media?course=');
		expect(http).toContain('/api/media?course=');
		expect(http).not.toContain('learnvault-media');
	}, 60_000);

	it('makes no third-party requests from the shell or styles', () => {
		buildOutputsIfNeeded('build');
		const shell = readFileSync(resolve(process.cwd(), 'dist', '200.html'), 'utf8');
		const css = readdirSync(resolve(process.cwd(), 'dist/_app/immutable')).filter((name) => name.endsWith('.css'))
			.map((name) => readFileSync(resolve(process.cwd(), 'dist/_app/immutable', name), 'utf8')).join('\n');
		// License banners mention URLs but load nothing.
		expect((shell + css).replace(/\/\*[\s\S]*?\*\//g, '')).not.toMatch(/https?:\/\/(?!www\.w3\.org)/);
	}, 60_000);
});
