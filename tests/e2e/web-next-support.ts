import AxeBuilder from '@axe-core/playwright';
import { expect, test as base, type Page } from '@playwright/test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

// Runs the React app (apps/web-next, epic #347) behind the real Rust server and its CSP.
// The web-next build is the server web root after cutover (#340).
export const PASSWORD = 'uma senha local muito segura';

export async function signIn(page: Page, origin: string) {
	await page.goto(`${origin}/#/login`);
	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('heading', { name: 'My learning' })).toBeVisible();
}

/** Fails on any WCAG 2.2 AA issue axe can detect on the current page. */
export async function expectNoAxeViolations(page: Page, context: string, disableRules: string[] = []) {
	const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']).disableRules(disableRules).analyze();
	expect(results.violations.flatMap((violation) => violation.nodes.map((node) => `${context}: ${violation.id} ${node.target.join(' ')} ${node.failureSummary ?? ''}`))).toEqual([]);
}

export function collectCspViolations(page: Page): string[] {
	const violations: string[] = [];
	page.on('console', (message) => { if (message.type() === 'error' && /Content Security Policy/.test(message.text())) violations.push(message.text()); });
	return violations;
}
/** The default library: two lessons whose media is not playable (enough for navigation tests). */
export async function basicLibrary(library: string) {
	for (const lesson of ['01 Ownership', '02 Borrowing']) {
		await mkdir(join(library, 'Rust from zero', lesson), { recursive: true });
		await writeFile(join(library, 'Rust from zero', lesson, 'Lesson.mp4'), Buffer.from('video-fixture'));
	}
}

/** One server per test: the sign-in limiter counts attempts per address, so a shared server runs out of them. */
export const createTest = (prepare: (library: string) => Promise<void>) => base.extend<{ origin: string }>({
	origin: async ({}, use) => {
		const root = await mkdtemp(join(tmpdir(), 'learnvault-web-next-'));
		const library = join(root, 'library');
		// Each top-level folder under the library root is registered as a course, so the
		// fixtures live inside one folder and their own folders become its modules.
		await prepare(join(library, 'Course'));
		const binary = process.env.LEARNVAULT_SERVER_BIN ?? resolve(process.platform === 'win32' ? 'target/debug/learnvault-server.exe' : 'target/debug/learnvault-server');
		const server = spawn(binary, ['--listen-addr', '127.0.0.1:0', '--database', join(root, 'db.sqlite'), '--library-root', library, '--web-root', resolve('crates/server/web/build'), '--insecure-http'], {
			env: { ...process.env, ADMIN_USERNAME: 'admin', ADMIN_PASSWORD: PASSWORD },
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
	}
});
