import { expect, type Page } from '@playwright/test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { TINY_WEBM_BASE64 } from './fixtures/tiny-webm';
import { makePdf } from './fixtures/make-pdf';
import { collectCspViolations, createTest, expectNoAxeViolations, signIn } from './web-next-support';

// The parts of the React app that need real media: the player's control bar, notes with formatting,
// the PDF editor and the dark theme. The library has one playable video lesson and one PDF lesson.
const test = createTest(async (library) => {
	await mkdir(join(library, '01 Video'), { recursive: true });
	await writeFile(join(library, '01 Video', 'Lesson.webm'), Buffer.from(TINY_WEBM_BASE64, 'base64'));
	await mkdir(join(library, '02 Reading', 'Guide'), { recursive: true });
	await writeFile(join(library, '02 Reading', 'Guide', 'Guide.pdf'), makePdf(['Exercise 1', 'Write a function longest that takes two slices']));
});

// A lesson video that has no subtitle file is normal in a course folder, so the captions rule is left out here.
const NO_CAPTIONS = ['video-caption'];

async function openCourse(page: Page, origin: string) {
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.getByRole('link', { name: 'library' }).first().click();
}

async function openVideoLesson(page: Page, origin: string) {
	await openCourse(page, origin);
	await page.getByRole('link', { name: 'Start course' }).click();
	await expect(page.getByRole('slider', { name: 'Seek' })).toBeVisible();
}

test('the player has one control bar that works, keeps cinema mode and passes axe in both themes', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openVideoLesson(page, origin);
	const video = page.locator('video');
	expect(await video.getAttribute('controls')).toBeNull();
	await page.getByRole('button', { name: 'Play' }).click();
	await expect.poll(() => video.evaluate((element: HTMLVideoElement) => !element.paused)).toBe(true);
	await page.getByRole('button', { name: 'Pause' }).click();
	await page.getByLabel('Speed').selectOption('1.5');
	expect(await video.evaluate((element: HTMLVideoElement) => element.playbackRate)).toBe(1.5);
	await expectNoAxeViolations(page, 'lesson light', NO_CAPTIONS);

	// Shortcuts act while the player has focus.
	await page.getByRole('group', { name: 'Media player' }).focus();
	await page.keyboard.press('k');
	await expect.poll(() => video.evaluate((element: HTMLVideoElement) => !element.paused)).toBe(true);
	await page.keyboard.press('k');

	await page.getByRole('button', { name: 'Cinema mode' }).click();
	await page.reload();
	await expect(page.getByRole('button', { name: 'Cinema mode' })).toHaveAttribute('aria-pressed', 'true');
	await page.getByRole('button', { name: 'Switch to the dark theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	await expectNoAxeViolations(page, 'lesson dark', NO_CAPTIONS);
	expect(violations).toEqual([]);
});

test('notes take formatting, show it, and are found from the course card', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openVideoLesson(page, origin);
	await page.getByRole('tab', { name: 'Notes' }).click();
	await page.getByRole('button', { name: /Add note at/ }).click();
	const field = page.getByLabel(/Note at/);
	// The editor opens focused, so typing goes straight in.
	await expect(field).toBeFocused();
	await page.keyboard.type('remember this');
	await field.press('Control+A');
	await page.getByRole('button', { name: 'Bold' }).click();
	await expect(field).toHaveValue('**remember this**');
	await page.getByRole('button', { name: 'Save note' }).click();
	await expect(page.locator('span.font-bold', { hasText: 'remember this' })).toBeVisible();
	await expect(page.getByText('Saved · just now')).toBeVisible();

	await page.goto(`${origin}/#/notes`);
	await expect(page.getByRole('heading', { name: /Choose a course to open its Notes/ })).toBeVisible();
	await page.getByRole('link', { name: /^library/ }).click();
	await expect(page.locator('span.font-bold', { hasText: 'remember this' })).toBeVisible();
	await expectNoAxeViolations(page, 'notebook');
});

test('the PDF editor searches, marks, takes a note and exports the marks', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openCourse(page, origin);
	await page.getByRole('button', { name: 'Expand all' }).click();
	await page.getByRole('link', { name: 'Guide' }).first().click();
	await expect(page.getByRole('toolbar', { name: 'PDF controls' })).toBeVisible();
	const text = page.locator('.pdf-text-layer span').first();
	await expect(text).toBeVisible();

	await page.getByRole('searchbox', { name: 'Search in the document' }).fill('LONGEST');
	await page.getByRole('button', { name: 'Search', exact: true }).click();
	await expect(page.getByText('Found on 1 page: now on page 1.')).toBeVisible();

	await text.dblclick();
	await page.getByRole('button', { name: 'Highlight' }).click();
	await expect(page.getByRole('button', { name: /^All \(1\)/ })).toBeVisible();
	await text.dblclick();
	await page.getByRole('button', { name: 'Add note' }).click();
	await page.getByLabel('Note for page 1').fill('check this against the lecture');
	await page.getByRole('button', { name: 'Save note' }).click();
	await expect(page.getByRole('textbox', { name: 'Note for the mark on page 1' }).last()).toHaveValue('check this against the lecture');
	await expectNoAxeViolations(page, 'pdf light');

	const [download] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: 'Export as Markdown' }).click()]);
	const exported = await readFile((await download.path())!, 'utf8');
	expect(exported).toContain('check this against the lecture');

	await page.getByRole('button', { name: 'Switch to the dark theme' }).click();
	await expectNoAxeViolations(page, 'pdf dark');
	expect(violations).toEqual([]);
});

test('every main page passes axe on the dark theme', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Switch to the dark theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	for (const route of ['#/', '#/progress', '#/notes', '#/review', '#/settings', '#/settings#users']) {
		await page.goto(`${origin}/${route}`);
		await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
		await expectNoAxeViolations(page, `dark ${route}`);
	}
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

// Module rows are native disclosures: they open to show their lessons and close again.
test('a module row opens to show its lessons and closes again', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openCourse(page, origin);
	const lesson = page.getByRole('link', { name: /^Lesson/ });
	await page.getByRole('button', { name: 'Collapse all' }).click();
	await expect(lesson).toBeHidden();
	await page.locator('summary', { hasText: 'Video' }).click();
	await expect(lesson).toBeVisible();
	await page.locator('summary', { hasText: 'Video' }).click();
	await expect(lesson).toBeHidden();
});

test('the chosen theme is applied before the app starts, from a script the CSP allows', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	const script = await page.request.get(`${origin}/theme-init.js`);
	expect(script.status()).toBe(200);
	expect(script.headers()['content-type']).toMatch(/javascript/);
	await page.addInitScript(() => localStorage.setItem('learnvault-appearance', JSON.stringify({ theme: 'dark' })));
	// With the app's own script blocked, only theme-init.js can have set the theme.
	await page.route('**/_app/**', (route) => route.abort());
	await page.goto(`${origin}/#/login`);
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	expect(violations).toEqual([]);
});
