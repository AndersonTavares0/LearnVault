import AxeBuilder from '@axe-core/playwright';
import { expect, test as base, type Page } from '@playwright/test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

// Runs the React app (apps/web-next, epic #347) behind the real Rust server and its CSP.
// Folded into server.spec.ts at cutover (#340).
const PASSWORD = 'uma senha local muito segura';

async function signIn(page: Page, origin: string) {
	await page.goto(`${origin}/#/login`);
	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('heading', { name: 'My learning' })).toBeVisible();
}

/** Fails on any WCAG 2.2 AA issue axe can detect on the current page. */
async function expectNoAxeViolations(page: Page, context: string) {
	const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']).analyze();
	expect(results.violations.flatMap((violation) => violation.nodes.map((node) => `${context}: ${violation.id} ${node.target.join(' ')} ${node.failureSummary ?? ''}`))).toEqual([]);
}

function collectCspViolations(page: Page): string[] {
	const violations: string[] = [];
	page.on('console', (message) => { if (message.type() === 'error' && /Content Security Policy/.test(message.text())) violations.push(message.text()); });
	return violations;
}
const test = base.extend<object, { origin: string }>({
	origin: [async ({}, use) => {
		const root = await mkdtemp(join(tmpdir(), 'learnvault-web-next-'));
		const library = join(root, 'library');
		for (const lesson of ['01 Ownership', '02 Borrowing']) {
			await mkdir(join(library, 'Rust from zero', lesson), { recursive: true });
			await writeFile(join(library, 'Rust from zero', lesson, 'Lesson.mp4'), Buffer.from('video-fixture'));
		}
		const binary = process.env.LEARNVAULT_SERVER_BIN ?? resolve(process.platform === 'win32' ? 'target/debug/learnvault-server.exe' : 'target/debug/learnvault-server');
		const server = spawn(binary, ['--listen-addr', '127.0.0.1:0', '--database', join(root, 'db.sqlite'), '--library-root', library, '--web-root', resolve('apps/web-next/dist'), '--insecure-http'], {
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
	}, { scope: 'worker' }]
});

test('signs in, indexes the library and resumes a lesson from My learning', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto(`${origin}/#/notes`);
	await expect(page.getByRole('heading', { name: 'Sign in to LearnVault' })).toBeVisible();
	await expectNoAxeViolations(page, 'login');
	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password', { exact: true }).fill('wrong password');
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('alert')).toHaveText('Incorrect username or password.');
	await page.getByLabel('Password', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Sign in' }).click();
	// Sign-in returns to the page that asked for it.
	await expect(page.getByRole('heading', { name: 'Notes' })).toBeVisible();

	await page.getByRole('link', { name: 'My learning' }).first().click();
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	// The library root registers as one course; its folders become modules.
	await expect(page.getByRole('heading', { name: 'library', level: 3 })).toBeVisible();
	await expectNoAxeViolations(page, 'home');

	const catalog = await (await page.request.get(`${origin}/api/catalog`)).json();
	const mediaId = JSON.stringify(catalog).match(/"media_id":"([^"]+)"/)?.[1];
	expect(mediaId).toBeTruthy();
	await expect(await page.request.post(`${origin}/api/study/position`, { headers: { Origin: origin }, data: { media_id: mediaId, position_seconds: 5 } })).toBeOK();
	await page.reload();
	const resume = page.getByRole('region', { name: 'Continue studying' });
	await expect(resume.getByRole('link', { name: 'Resume lesson' })).toHaveAttribute('href', /#\/course\/.+\?t=5$/);
	await expectNoAxeViolations(page, 'home with resume');

	await resume.getByRole('link', { name: 'Resume lesson' }).click();
	await expect(page.getByRole('heading', { level: 1, name: 'Ownership' })).toBeVisible();
	// The fixture bytes are not a real video, so the player shows its recovery state.
	await expect(page.getByRole('alert').filter({ hasText: /media/i })).toBeVisible();
	await expect(page.getByRole('complementary', { name: 'Course content' })).toBeVisible();
	await page.getByRole('tab', { name: 'Notes' }).click();
	await page.getByRole('button', { name: /Add note at/ }).click();
	await page.getByLabel(/Note at/).fill('Ownership moves values.');
	await page.getByRole('button', { name: 'Save note' }).click();
	await expect(page.getByText('Ownership moves values.')).toBeVisible();
	await expectNoAxeViolations(page, 'lesson');

	await page.goto(`${origin}/#/progress`);
	await expect(page.getByRole('heading', { name: 'Weekly goal' })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Your courses' })).toBeVisible();
	await expectNoAxeViolations(page, 'progress');

	await page.goto(`${origin}/#/notes`);
	await expect(page.getByText('Ownership moves values.')).toBeVisible();
	await expectNoAxeViolations(page, 'notes');
	await page.getByRole('button', { name: /^Pomodoro:/ }).click();
	const timer = page.getByRole('dialog', { name: 'Pomodoro' });
	await timer.getByRole('button', { name: 'Start' }).click();
	await expect(timer.getByRole('button', { name: 'Pause' })).toBeVisible();
	await expectNoAxeViolations(page, 'pomodoro');
	await timer.getByRole('button', { name: 'Reset' }).click();
	await page.keyboard.press('Escape');
	await expect(timer).toBeHidden();
	await page.goto(`${origin}/#/`);

	await page.getByRole('link', { name: 'library' }).click();
	await expect(page.getByRole('heading', { level: 1, name: 'library' })).toBeVisible();
	// The module holding the lesson to continue starts expanded.
	await expect(page.getByRole('link', { name: /Borrowing/ })).toBeVisible();
	await page.getByRole('button', { name: 'Mark “Borrowing” as completed' }).click();
	await expect(page.getByRole('progressbar', { name: /1 of 2 lessons completed/ })).toBeVisible();
	await expectNoAxeViolations(page, 'course');
	await page.getByRole('tab', { name: /Materials/ }).click();
	await expect(page.getByText('No support materials in this course.')).toBeVisible();
	await page.getByRole('button', { name: 'Manage course' }).click();
	await page.getByRole('menuitem', { name: 'Remove course' }).click();
	const dialog = page.getByRole('alertdialog');
	await expect(dialog).toContainText('The original files on disk are not deleted');
	await expectNoAxeViolations(page, 'remove dialog');
	await dialog.getByRole('button', { name: 'Cancel' }).click();
	await expect(dialog).toBeHidden();
	await page.goto(`${origin}/#/`);

	await page.setViewportSize({ width: 375, height: 800 });
	await expect(page.getByRole('navigation', { name: 'Sections' })).toBeVisible();
	await expect(page.getByRole('navigation', { name: 'Main' })).toBeHidden();
	await expectNoAxeViolations(page, 'home mobile');
	expect(violations).toEqual([]);
});

test('switches palette and theme with no CSP violations or axe issues', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	// Reduced motion disables transitions, so axe measures settled colors, not mid-fade ones.
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

	const primary = new Set<string>();
	for (const palette of ['Stone + Emerald', 'Zinc + Teal', 'Zinc + Indigo']) {
		await page.getByRole('button', { name: palette }).click();
		for (const theme of ['Light', 'Dark']) {
			await page.getByRole('button', { name: theme, exact: true }).click();
			primary.add(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--primary')));
			await expectNoAxeViolations(page, `${palette}/${theme}`);
		}
	}
	expect(primary.size).toBe(6);

	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-palette', 'indigo');
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	expect(violations).toEqual([]);
});

test('keeps legacy links working', async ({ page, origin }) => {
	await signIn(page, origin);
	await page.goto(`${origin}/#/statistics`);
	await expect(page).toHaveURL(/#\/progress$/);
	await expect(page.getByRole('heading', { name: 'Progress' })).toBeVisible();
});

test('admin settings pass axe and new accounts start with no course access', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Accounts and course access' })).toBeVisible();
	await expectNoAxeViolations(page, 'settings');

	const form = page.locator('form', { has: page.getByRole('heading', { name: 'New account' }) });
	await expect(form.getByRole('checkbox', { name: /All courses/ })).not.toBeChecked();
	await form.getByLabel('Username').fill('ana');
	await form.getByLabel('Password (12+ characters)', { exact: true }).fill('a long student passphrase');
	await form.getByRole('button', { name: 'Create account' }).click();
	await expect(page.getByText('Account “ana” created.')).toBeVisible();

	await page.getByRole('button', { name: 'Account: admin' }).click();
	await page.getByRole('menuitem', { name: 'Sign out' }).click();
	await expect(page.getByRole('heading', { name: 'Sign in to LearnVault' })).toBeVisible();
	await page.getByLabel('Username').fill('ana');
	await page.getByLabel('Password', { exact: true }).fill('a long student passphrase');
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByText('No courses yet')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Import course' })).toHaveCount(0);
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Accounts and course access' })).toHaveCount(0);
	expect(violations).toEqual([]);
});
