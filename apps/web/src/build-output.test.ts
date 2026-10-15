import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('production shell contract', () => {
	it('runs the real build and emits the Go fallback shell', () => {
		execFileSync('pnpm', ['run', 'build'], { cwd: process.cwd(), stdio: 'pipe', shell: true });
		const shell = resolve(process.cwd(), '../../services/go/cmd/learnvault/web/build/200.html');
		expect(existsSync(shell)).toBe(true);
		expect(readFileSync(shell, 'utf8')).toContain('/_app/immutable/');
	});
	it('runs the desktop build into a separate shell directory', () => {
		execFileSync('pnpm', ['run', 'build:desktop'], { cwd: process.cwd(), stdio: 'pipe', shell: true });
		const shell = resolve(process.cwd(), '../../services/go/cmd/learnvault/web/desktop-build/200.html');
		expect(existsSync(shell)).toBe(true);
		expect(readFileSync(shell, 'utf8')).toContain('/_app/immutable/');
	});
});
