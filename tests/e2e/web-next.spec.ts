import AxeBuilder from '@axe-core/playwright';
import { expect, test as base } from '@playwright/test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

// Runs the React app (apps/web-next, epic #347) behind the real Rust server and its CSP.
// Folded into server.spec.ts at cutover (#340).
const test = base.extend<object, { origin: string }>({
	origin: [async ({}, use) => {
		const root = await mkdtemp(join(tmpdir(), 'learnvault-web-next-'));
		const library = join(root, 'library');
		await mkdir(library, { recursive: true });
		const binary = process.env.LEARNVAULT_SERVER_BIN ?? resolve(process.platform === 'win32' ? 'target/debug/learnvault-server.exe' : 'target/debug/learnvault-server');
		const server = spawn(binary, ['--listen-addr', '127.0.0.1:0', '--database', join(root, 'db.sqlite'), '--library-root', library, '--web-root', resolve('apps/web-next/dist'), '--insecure-http'], {
			env: { ...process.env, ADMIN_USERNAME: 'admin', ADMIN_PASSWORD: 'uma senha local muito segura' },
			stdio: ['ignore', 'pipe', 'ignore']
		});
		const closed = once(server, 'close');
		let output = '';
		const address = await new Promise<string>((ok, fail) => {
			server.stdout!.on('data', (chunk) => {
				output += chunk;
				const line = output.split(/\r?\n/).find((entry) => entry.startsWith('LEARNVAULT_READY '));
				if (line) ok(line.slice('LEARNVAULT_READY '.length).trim());
			});
			server.once('close', () => fail(new Error(`server exited: ${output}`)));
		});
		try {
			await use(`http://${address}`);
		} finally {
			server.kill('SIGTERM');
			await closed;
			await rm(root, { recursive: true, force: true });
		}
	}, { scope: 'worker' }]
});

test('serves the React shell with no CSP violations or axe issues in every palette and theme', async ({ page, origin }) => {
	const violations: string[] = [];
	page.on('console', (message) => { if (message.type() === 'error' && /Content Security Policy/.test(message.text())) violations.push(message.text()); });
	// Reduced motion disables transitions, so axe measures settled colors, not mid-fade ones.
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

	const primary = new Set<string>();
	for (const palette of ['Stone + Emerald', 'Zinc + Teal', 'Zinc + Indigo']) {
		await page.getByRole('button', { name: palette }).click();
		for (const theme of ['Light', 'Dark']) {
			await page.getByRole('button', { name: theme, exact: true }).click();
			primary.add(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--primary')));
			const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']).analyze();
			expect(results.violations.flatMap((violation) => violation.nodes.map((node) => `${palette}/${theme}: ${violation.id} ${node.target.join(" ")} ${node.failureSummary ?? ""}`))).toEqual([]);
		}
	}
	expect(primary.size).toBe(6);

	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-palette', 'indigo');
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	expect(violations).toEqual([]);
});

test('keeps legacy links working', async ({ page, origin }) => {
	await page.goto(`${origin}/#/statistics`);
	await expect(page).toHaveURL(/#\/progress$/);
	await expect(page.getByRole('heading', { name: 'Progress' })).toBeVisible();
});
