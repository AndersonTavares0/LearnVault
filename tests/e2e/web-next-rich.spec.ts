import { expect, type Page } from '@playwright/test';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { TINY_VP9_WEBM_BASE64, TINY_WEBM_BASE64 } from './fixtures/tiny-webm';
import { makeHostilePdfs, makePdf } from './fixtures/make-pdf';
import { collectCspViolations, createTest, expectNoAxeViolations, setTheme, signIn } from './web-next-support';

// The parts of the React app that need real media: the player's control bar, notes with formatting,
// the PDF editor and the dark theme. The library has one playable video lesson and one PDF lesson.
const test = createTest(async (library) => {
	await mkdir(join(library, '01 Video'), { recursive: true });
	// VP9 video-only WebM works both natively and through ffmpeg's `-c:v copy -f mp4` smart remux in Chromium.
	await writeFile(join(library, '01 Video', 'Lesson.webm'), Buffer.from(TINY_VP9_WEBM_BASE64, 'base64'));
	// A language-suffixed sidecar subtitle must reach the lesson as a selectable caption track.
	await writeFile(join(library, '01 Video', 'Lesson.en.vtt'), 'WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nHello\n');
	// A lesson that holds two videos: switching between them changes the player's source.
	await mkdir(join(library, '03 Pair', 'Both'), { recursive: true });
	await writeFile(join(library, '03 Pair', 'Both', 'a.webm'), Buffer.from(TINY_WEBM_BASE64, 'base64'));
	await writeFile(join(library, '03 Pair', 'Both', 'b.webm'), Buffer.from(TINY_WEBM_BASE64, 'base64'));
	for (const [name, bytes] of Object.entries(makeHostilePdfs())) {
		await mkdir(join(library, '04 Hostile', name), { recursive: true });
		await writeFile(join(library, '04 Hostile', name, `${name}.pdf`), bytes);
	}
	await mkdir(join(library, '02 Reading', 'Guide'), { recursive: true });
	await writeFile(join(library, '02 Reading', 'Guide', 'Guide.pdf'), makePdf(['Exercise 1', 'Write a function longest that takes two slices']));
});

// A lesson video that has no subtitle file is normal in a course folder, so the captions rule is left out here.
const NO_CAPTIONS = ['video-caption'];

async function openCourse(page: Page, origin: string) {
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.getByRole('link', { name: 'Course' }).first().click();
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
	await setTheme(page, 'light');
	const video = page.locator('video');
	expect(await video.getAttribute('controls')).toBeNull();
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect.poll(() => video.evaluate((element: HTMLVideoElement) => !element.paused)).toBe(true);
	await page.getByRole('button', { name: 'Pause' }).click();
	await page.getByLabel('Speed').selectOption('1.5');
	expect(await video.evaluate((element: HTMLVideoElement) => element.playbackRate)).toBe(1.5);
	// The sidecar subtitle is offered, can be hidden, and hiding removes the track.
	// Subtitles are chosen in the player settings menu.
	const chooseSubtitle = async (name: string) => {
		await page.getByRole('button', { name: 'Player settings' }).click();
		await page.getByRole('menuitem', { name: 'Subtitles' }).click();
		await page.getByRole('menuitemradio', { name, exact: true }).click();
		// A radio choice leaves the menus open: the first Escape closes the Subtitles submenu, the second the settings menu.
		await page.keyboard.press('Escape');
		await page.keyboard.press('Escape');
		await expect(page.locator('[role="menu"][data-open]')).toHaveCount(0);
	};
	await expect(page.locator('video track')).toHaveCount(1);
	await chooseSubtitle('Off');
	await expect(page.locator('video track')).toHaveCount(0);
	await chooseSubtitle('Lesson.en.vtt');
	await expect(page.locator('video track')).toHaveCount(1);
	await expectNoAxeViolations(page, 'lesson light', NO_CAPTIONS);

	// Shortcuts act while the player has focus.
	await page.getByRole('group', { name: 'Media player' }).focus();
	await page.keyboard.press('k');
	await expect.poll(() => video.evaluate((element: HTMLVideoElement) => !element.paused)).toBe(true);
	await page.keyboard.press('k');

	await page.getByRole('button', { name: 'Cinema mode' }).click();
	await page.reload();
	await expect(page.getByRole('button', { name: 'Cinema mode' })).toHaveAttribute('aria-pressed', 'true');
	await setTheme(page, 'dark');
	await expectNoAxeViolations(page, 'lesson dark', NO_CAPTIONS);
	// The server keeps the speed and subtitle language for the user, not the browser.
	await page.evaluate(() => localStorage.clear());
	await page.reload();
	await expect(page.getByLabel('Speed')).toHaveValue('1.5');
	await expect(page.locator('video track')).toHaveCount(1);
	expect(violations).toEqual([]);
});

test('the player settings menu opens inside the fullscreen player', async ({ page, origin }) => {
	await openVideoLesson(page, origin);
	await page.getByRole('button', { name: 'Fullscreen' }).click();
	await expect.poll(() => page.evaluate(() => Boolean(document.fullscreenElement))).toBe(true);
	await page.getByRole('button', { name: 'Player settings' }).click();
	const item = page.getByRole('menuitem', { name: 'Loop a section' });
	await expect(item).toBeVisible();
	// A menu mounted outside the fullscreen element would be open but invisible.
	expect(await item.evaluate((element) => Boolean(document.fullscreenElement?.contains(element)))).toBe(true);
});

test('every lesson tool fits without horizontal scrolling at narrow widths', async ({ page, origin }) => {
	await openVideoLesson(page, origin);
	const tools = page.getByRole('tablist', { name: 'Lesson tools' });
	// This lesson has a subtitle but no cards: the rail offers Notes, Cards and Contents.
	await expect(tools.getByRole('tab')).toHaveText(['Notes', 'Cards', 'Contents']);
	await page.evaluate(() => document.fonts.ready.then(() => undefined));
	for (const theme of ['light', 'dark'] as const) {
		await setTheme(page, theme);
		for (const width of [320, 390, 768]) {
			await page.setViewportSize({ width, height: 844 });
			const context = `${width}px ${theme}`;
			expect(await tools.evaluate((list) => list.scrollWidth - list.clientWidth), context).toBeLessThanOrEqual(1);
			expect(await page.getByRole('main').evaluate((main) => main.scrollWidth - main.clientWidth), context).toBeLessThanOrEqual(1);
			for (const tab of await tools.getByRole('tab').all()) {
				expect(await tab.evaluate((el) => el.getBoundingClientRect().height), context).toBeGreaterThanOrEqual(44);
			}
		}
	}
});

test('notes take formatting, show it, and are found from the course card', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openVideoLesson(page, origin);
	await page.getByRole('tab', { name: 'Notes' }).click();
	await page.getByRole('button', { name: /Write a note at/ }).click();
	const field = page.getByLabel(/Note at/);
	// The editor opens focused, so typing goes straight in.
	await expect(field).toBeFocused();
	await page.keyboard.type('remember this');
	await field.press('Control+A');
	await page.getByRole('button', { name: 'Bold' }).click();
	await expect(field).toHaveValue('**remember this**');
	await page.getByRole('button', { name: 'Save note' }).click();
	await expect(page.locator('span.font-bold', { hasText: 'remember this' })).toBeVisible();
	await expect(page.getByRole('status').filter({ hasText: 'Note saved' })).toBeVisible();

	await page.goto(`${origin}/#/notes`);
	await expect(page.getByRole('heading', { name: 'Notes', level: 1 })).toBeVisible();
	await expect(page.locator('span.font-bold', { hasText: 'remember this' })).toBeVisible();
	await expectNoAxeViolations(page, 'notebook');
});

test('the PDF editor searches, marks, takes a note and exports the marks', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openCourse(page, origin);
	await setTheme(page, 'light');
	await page.getByRole('button', { name: 'Expand all' }).click();
	await page.getByRole('link', { name: 'Guide' }).first().click();
	await expect(page.getByRole('toolbar', { name: 'PDF controls' })).toBeVisible();
	const text = page.locator('.pdf-text-layer span').first();
	await expect(text).toBeVisible();

	// Finding and annotating are panels of the reader: open them first.
	await page.getByRole('button', { name: 'Find in the document' }).click();
	await page.getByRole('searchbox', { name: 'Search in the document' }).fill('LONGEST');
	await page.getByRole('button', { name: 'Search', exact: true }).click();
	await expect(page.getByText('Found on 1 page: now on page 1.')).toBeVisible();

	await page.getByRole('button', { name: 'Annotate' }).click();
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

	await setTheme(page, 'dark');
	await expectNoAxeViolations(page, 'pdf dark');
	expect(violations).toEqual([]);
});

test('every main page passes axe on the dark theme', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await setTheme(page, 'dark');
	for (const route of ['#/', '#/progress', '#/notes', '#/review', '#/history', '#/settings', '#/settings#users']) {
		await page.goto(`${origin}/${route}`);
		await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
		await expectNoAxeViolations(page, `dark ${route}`);
	}
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('main pages keep their content inside narrow and desktop viewports', async ({ page, origin }, testInfo) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	for (const width of [320, 390, 768, 1440]) {
		await page.setViewportSize({ width, height: 900 });
		for (const route of ['/', '/progress', '/notes', '/review', '/history', '/settings', '/settings#users']) {
			await page.goto(`${origin}/#${route}`);
			await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
			// Measured before switching section: Appearance is the first one, and the one that overflowed.
			// The content region scrolls now, so a control wider than its card widens it without widening the document.
			expect(await page.locator('#main').evaluate((element) => element.scrollWidth - element.clientWidth), `${width}px ${route}: content region scrolls sideways`).toBeLessThanOrEqual(1);
			if (route === '/settings' && width < 1024) {
				await page.getByLabel('Settings section', { exact: true }).selectOption('users');
				await expect(page.getByRole('heading', { name: 'Accounts and course access' })).toBeVisible();
			}
			expect(await page.locator('main section, main form').evaluateAll((elements) =>
				elements.filter((element) => {
					const box = element.getBoundingClientRect();
					return box.width > 0 && (box.left < -1 || box.right > innerWidth + 1);
				}).map((element) => element.getAttribute('aria-labelledby') || element.id || element.tagName)
			), `${width}px ${route}: clipped content`).toEqual([]);
			await page.screenshot({ path: testInfo.outputPath(`${width}-${route.replaceAll('/', '').replaceAll('#', '-') || 'home'}.png`), fullPage: true });
			expect(await page.evaluate(() => document.documentElement.scrollWidth), `${width}px ${route}`).toBeLessThanOrEqual(width);
		}
	}
});

test('the Portuguese library keeps its active navigation in both Lamplight themes', async ({ page, origin }, testInfo) => {
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.goto(`${origin}/#/settings`);
	await page.getByRole('button', { name: 'Português (Brasil)' }).click();
	await page.goto(`${origin}/#/`);
	for (const theme of ['dark', 'light']) {
		if (theme === 'light') await page.getByRole('button', { name: /tema claro/i }).click();
		await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
		for (const width of [390, 1440]) {
			await page.setViewportSize({ width, height: 900 });
			const nav = page.getByRole('navigation', { name: width === 390 ? 'Seções' : 'Principal', exact: true });
			await expect(nav.locator('a[aria-current="page"]')).toHaveText(/Biblioteca|Meu aprendizado/);
			const filters = page.getByRole('group', { name: 'Mostrar', exact: true });
			for (const filter of await filters.getByRole('button').all()) {
				await expect(filter).toBeInViewport({ ratio: 1 });
			}
			await expectNoAxeViolations(page, `library pt-BR ${theme} ${width}`);
			await page.screenshot({ path: testInfo.outputPath(`library-pt-${theme}-${width}.png`) });
		}
	}
});

test('mobile settings stay usable in Portuguese and the dark theme', async ({ page, origin }, testInfo) => {
	await signIn(page, origin);
	await setTheme(page, 'dark');
	await page.goto(`${origin}/#/settings`);
	await page.getByRole('button', { name: 'Português (Brasil)' }).click();
	for (const width of [320, 390]) {
		await page.setViewportSize({ width, height: 900 });
		await page.getByLabel('Seção dos ajustes', { exact: true }).selectOption('users');
		await expect(page.getByRole('heading', { name: 'Contas e acesso a cursos' })).toBeVisible();
		await expect(page.getByRole('navigation', { name: 'Seções', exact: true }).getByRole('link', { name: 'Meu aprendizado' })).toContainText('Biblioteca');
		expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
		await page.screenshot({ path: testInfo.outputPath(`${width}-settings-pt-dark.png`), fullPage: true });
		await expectNoAxeViolations(page, `mobile settings pt-BR dark ${width}`);
	}
});

// Module rows are native disclosures: they open to show their lessons and close again.
test('a module row opens to show its lessons and closes again', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openCourse(page, origin);
	const lesson = page.getByRole('link', { name: /^Open lesson.*Video/ });
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

/** The bar's Play/Pause button must always say what the video is doing. */
async function expectBarMatchesVideo(page: Page) {
	await expect.poll(async () => {
		const paused = await page.locator('video').evaluate((element: HTMLVideoElement) => element.paused);
		const shows = (await page.getByRole('button', { name: 'Pause', exact: true }).count()) > 0 ? 'Pause' : 'Play';
		return `${paused ? 'paused' : 'playing'}/${shows}`;
	}).toMatch(/^(paused\/Play|playing\/Pause)$/);
}

async function openPair(page: Page, origin: string) {
	await openCourse(page, origin);
	await page.getByRole('button', { name: 'Expand all' }).click();
	await page.getByRole('link', { name: /^Both/ }).first().click();
	await expect(page.getByRole('slider', { name: 'Seek' })).toBeVisible();
}

test('switching to another video of the lesson keeps the bar in step and clears the loop', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openPair(page, origin);
	const video = page.locator('video');
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect.poll(() => video.evaluate((element: HTMLVideoElement) => !element.paused)).toBe(true);
	await page.getByRole('button', { name: 'Player settings' }).click();
	await page.getByRole('menuitem', { name: 'Loop a section' }).click();
	const loopChip = page.getByRole('button', { name: /^Loop from|^Looping/ });
	await expect(loopChip).toHaveAttribute('aria-pressed', 'true');

	await page.getByLabel('Video', { exact: true }).selectOption({ index: 1 });
	await expect(page.getByRole('slider', { name: 'Seek' })).toBeVisible();
	await expectBarMatchesVideo(page);
	await expect(loopChip).toHaveCount(0);
	// The new video plays from the bar.
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect.poll(() => video.evaluate((element: HTMLVideoElement) => !element.paused)).toBe(true);
	await expectBarMatchesVideo(page);
});

test('turning transcoding on and seeking keeps the bar in step with the video', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await openVideoLesson(page, origin);
	const capabilities = await page.request.get(`${origin}/api/media/capabilities`);
	expect(capabilities.ok()).toBe(true);
	const { transcoding } = (await capabilities.json()) as { transcoding?: boolean };
	if (process.env.CI) {
		expect(transcoding, 'CI e2e-server must have ffmpeg installed so transcoding is tested').toBe(true);
	}
	test.skip(!transcoding, 'the server has no ffmpeg, so transcoding is not offered');
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await page.getByRole('button', { name: 'Player settings' }).click();
	const transcode = page.getByRole('menuitem', { name: 'Play with transcoding' });
	await expect(transcode).toBeVisible();
	await transcode.click();
	await expect(page.locator('video')).toHaveAttribute('src', /^\/api\/media\/transcode\?/);
	await expectBarMatchesVideo(page);
	await page.getByRole('slider', { name: 'Seek' }).focus();
	await page.keyboard.press('ArrowRight');
	await expectBarMatchesVideo(page);
	await expect(page.getByRole('slider', { name: 'Seek' })).toBeVisible();
});

// Security regression: note text is data, never markup. A hostile note must render as plain text.
test('a note with HTML, a script and a javascript link renders as text and runs nothing', async ({ page, origin }) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	const violations = collectCspViolations(page);
	const dialogs: string[] = [];
	page.on('dialog', (dialog) => { dialogs.push(dialog.message()); void dialog.dismiss(); });
	await openVideoLesson(page, origin);
	await page.getByRole('tab', { name: 'Notes' }).click();
	await page.getByRole('button', { name: /Write a note at/ }).click();
	const hostile = '<img src=x onerror="window.pwned=1"> <script>window.pwned=2</script> [click](javascript:window.pwned=3) **bold** and snake__case__name';
	await page.getByLabel(/Note at/).fill(hostile);
	await page.getByRole('button', { name: 'Save note' }).click();
	await expect(page.locator('span.font-bold', { hasText: 'bold' })).toBeVisible();
	const result = await page.evaluate(() => ({
		pwned: (window as unknown as { pwned?: number }).pwned ?? null,
		injected: document.querySelectorAll('main img[src="x"], main script, main a[href^="javascript:"]').length
	}));
	expect(result).toEqual({ pwned: null, injected: 0 });
	// Every character of the payload is on the page as text, including the double underscores inside a name like snake__case__name.
	await expect(page.locator('main')).toContainText('<img src=x onerror="window.pwned=1"> <script>window.pwned=2</script> [click](javascript:window.pwned=3) bold and snake__case__name');
	expect(dialogs).toEqual([]);
	expect(violations).toEqual([]);
});

// Security regression: a PDF is a file from the course folder, so the reader treats it as untrusted.
for (const name of ['script', 'cycle', 'truncated']) {
	test(`a hostile PDF (${name}) shows as text or an error and runs nothing`, async ({ page, origin }) => {
		await page.emulateMedia({ reducedMotion: 'reduce' });
		const violations = collectCspViolations(page);
		const dialogs: string[] = [];
		page.on('dialog', (dialog) => { dialogs.push(dialog.message()); void dialog.dismiss(); });
		const popups: string[] = [];
		page.on('popup', (popup) => popups.push(popup.url()));
		await openCourse(page, origin);
		await page.getByRole('button', { name: 'Expand all' }).click();
		await page.getByRole('link', { name: new RegExp(`^${name}`) }).first().click();
		// The reader either draws the file or says it cannot open it; both are fine, a hang is not.
		await expect(page.getByRole('toolbar', { name: 'PDF controls' }).or(page.getByRole('alert')).first()).toBeVisible({ timeout: 15_000 });
		await page.getByRole('toolbar', { name: 'PDF controls' }).click({ position: { x: 5, y: 5 }, timeout: 2000 }).catch(() => undefined);
		// Click where the hostile link sits and let the page settle.
		await page.locator('.pdf-text-layer').click({ position: { x: 20, y: 20 }, timeout: 2000, force: true }).catch(() => undefined);
		await page.waitForTimeout(500);
		const state = await page.evaluate(() => ({ pwned: (window as unknown as { pdfpwned?: number }).pdfpwned ?? null, responsive: 1 + 1 }));
		expect(state).toEqual({ pwned: null, responsive: 2 });
		expect(dialogs).toEqual([]);
		expect(popups).toEqual([]);
		expect(violations).toEqual([]);
	});
}

test('the player shortcuts mute, change the volume and the speed, and play and pause', async ({ page, origin }) => {
	await openVideoLesson(page, origin);
	const video = page.locator('video');
	const state = () => video.evaluate((element: HTMLVideoElement) => ({ paused: element.paused, muted: element.muted, volume: Math.round(element.volume * 100) / 100, rate: element.playbackRate }));
	await page.getByRole('group', { name: 'Media player' }).focus();

	await page.keyboard.press('m');
	await expect.poll(async () => (await state()).muted).toBe(true);
	await page.keyboard.press('m');
	await expect.poll(async () => (await state()).muted).toBe(false);

	await page.keyboard.press('ArrowDown');
	await expect.poll(async () => (await state()).volume).toBe(0.9);
	await page.keyboard.press('ArrowUp');
	await expect.poll(async () => (await state()).volume).toBe(1);

	await page.keyboard.press(']');
	await expect.poll(async () => (await state()).rate).toBe(1.25);
	await page.keyboard.press('[');
	await expect.poll(async () => (await state()).rate).toBe(1);

	await page.keyboard.press('Space');
	await expect.poll(async () => (await state()).paused).toBe(false);
	await page.keyboard.press('Space');
	await expect.poll(async () => (await state()).paused).toBe(true);

	// A shortcut typed into a field is text, not a command.
	await page.getByRole('tab', { name: 'Notes' }).click();
	await page.getByRole('button', { name: /Write a note at/ }).click();
	await page.getByLabel(/Note at/).fill('m] k');
	expect((await state()).muted).toBe(false);
	expect((await state()).rate).toBe(1);
});
