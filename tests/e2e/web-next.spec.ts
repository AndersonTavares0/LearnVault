import { expect } from '@playwright/test';
import { mkdir, readdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { basicLibrary, collectCspViolations, createTest, expectNoAxeViolations, PASSWORD, signIn } from './web-next-support';

// Runs the React app (apps/web-next, epic #347) behind the real Rust server and its CSP.
// Folded into server.spec.ts at cutover (#340).
const test = createTest(basicLibrary);

test('signs in, indexes the library and resumes a lesson from My learning', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto(`${origin}/#/notes`);
	await expect(page.getByRole('heading', { name: 'Sign in', level: 1 })).toBeVisible();
	await expectNoAxeViolations(page, 'login');
	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password', { exact: true }).fill('wrong password');
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('alert')).toHaveText('Incorrect username or password.');
	await page.getByLabel('Password', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Sign in' }).click();
	// Sign-in returns to the page that asked for it.
	await expect(page.getByRole('heading', { name: 'Notes', level: 1 })).toBeVisible();

	await page.getByRole('link', { name: 'My learning' }).first().click();
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	// A top-level folder registers as one course; its folders become modules.
	await expect(page.getByRole('heading', { name: 'Course', level: 3 })).toHaveCount(1);
	await expectNoAxeViolations(page, 'home');

	const catalog = await (await page.request.get(`${origin}/api/catalog`)).json();
	const mediaId = JSON.stringify(catalog).match(/"media_id":"([^"]+)"/)?.[1];
	expect(mediaId).toBeTruthy();
	await expect(await page.request.post(`${origin}/api/study/position`, { headers: { Origin: origin }, data: { media_id: mediaId, position_seconds: 5 } })).toBeOK();
	await page.reload();
	const resume = page.getByRole('region', { name: 'Continue learning' });
	await expect(resume.getByRole('link', { name: 'Resume' })).toHaveAttribute('href', /#\/course\/.+\?t=5$/);
	await expectNoAxeViolations(page, 'home with resume');

	await resume.getByRole('link', { name: 'Resume' }).click();
	await expect(page.getByRole('heading', { level: 1, name: 'Ownership' })).toBeVisible();
	// The fixture bytes are not a real video, so the player shows its recovery state.
	await expect(page.getByRole('alert').filter({ hasText: /media/i })).toBeVisible();
	await expect(page.getByRole('tab', { name: 'Contents' })).toBeVisible();
	await page.getByRole('tab', { name: 'Notes' }).click();
	await page.getByRole('button', { name: /Write a note at/ }).click();
	await page.getByLabel(/Note at/).fill('Ownership moves values.');
	await page.getByRole('button', { name: 'Save note' }).click();
	await expect(page.getByText('Ownership moves values.')).toBeVisible();
	await expectNoAxeViolations(page, 'lesson');

	await page.goto(`${origin}/#/progress`);
	await expect(page.getByLabel('At a glance').getByText('Active days')).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Badges' })).toBeVisible();
	await expectNoAxeViolations(page, 'progress');

	await page.goto(`${origin}/#/notes`);
	await expectNoAxeViolations(page, 'notes courses');
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

	// The rail also lists courses in progress, so the click stays in the library.
	await page.getByRole('region', { name: 'Your library' }).getByRole('link', { name: 'Course', exact: true }).click();
	await expect(page.getByRole('heading', { level: 1, name: 'Course' })).toBeVisible();
	// The module holding the lesson to continue starts expanded.
	await expect(page.getByRole('link', { name: /Borrowing/ })).toBeVisible();
	await page.getByRole('button', { name: 'Mark “Borrowing” as completed' }).click();
	await expect(page.getByRole('progressbar', { name: /1 of 2 lessons done/ })).toBeVisible();
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

test('Space activates the focused Close button during flashcard review', async ({ page, origin }) => {
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	const catalog = await (await page.request.get(`${origin}/api/catalog`)).json();
	const mediaId = JSON.stringify(catalog).match(/"media_id":"([^"]+)"/)?.[1];
	expect(mediaId).toBeTruthy();
	await expect(await page.request.post(`${origin}/api/study/flashcards`, {
		headers: { Origin: origin },
		data: { media_id: mediaId, front: 'Test question', back: 'Test answer' }
	})).toBeOK();

	await page.goto(`${origin}/#/review?course=__all`);
	await page.getByRole('button', { name: 'Start review' }).click();
	const showAnswer = page.getByRole('button', { name: 'Show answer' });
	await expect(showAnswer).toBeFocused();
	await page.keyboard.press('Space');
	await expect(page.getByText('Test answer')).toBeVisible();
	await page.getByRole('button', { name: 'Close' }).click();

	const startReview = page.getByRole('button', { name: 'Start review' });
	await startReview.click();
	const close = page.getByRole('button', { name: 'Close' });
	await close.focus();
	await page.keyboard.press('Space');
	await expect(startReview).toBeVisible();
});

test('customizing the look passes axe for every accent and breaks no CSP rule', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	// Reduced motion turns transitions off, so axe measures settled colours, not mid-fade ones.
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

	const brand = new Set<string>();
	// Every accent has to pass on the light page and on the dark one.
	for (const theme of ['Light', 'Dark']) {
		await page.getByRole('group', { name: 'Theme' }).getByRole('button', { name: theme }).click();
		await expect(page.locator('html')).toHaveAttribute('data-theme', theme.toLowerCase());
		for (const accent of ['Ink', 'Brown', 'Amber', 'Forest', 'Terracotta', 'Navy', 'Plum', 'Teal', 'Slate']) {
			await page.getByRole('group', { name: 'Accent color' }).getByRole('button', { name: accent }).click();
			brand.add(await page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--brand').trim()));
			await expectNoAxeViolations(page, `settings/${theme}/${accent}`);
		}
	}
	expect(brand.size).toBe(18);
	await page.getByRole('group', { name: 'Theme' }).getByRole('button', { name: 'Light' }).click();
	for (const choice of [['Corners', 'Round'], ['Density', 'Compact'], ['Text size', 'Large'], ['Font', 'System font']]) {
		await page.getByRole('group', { name: choice[0] }).getByRole('button', { name: choice[1] }).click();
	}
	await expectNoAxeViolations(page, 'settings/customized');
	await page.goto(`${origin}/#/`);
	await expect(page.getByRole('heading', { name: 'My learning' })).toBeVisible();
	await expectNoAxeViolations(page, 'home/customized');

	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-radius', 'round');
	expect(violations).toEqual([]);
});

test('every surface palette passes axe on the light and the dark page', async ({ page, origin }) => {
	// Reduced motion turns transitions off, so axe measures settled colours, not mid-fade ones.
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

	const surfaces = [['Warm paper', 'warm'], ['Cool stone', 'stone'], ['Crisp contrast', 'crisp'], ['Kindle sepia', 'sepia'], ['Forest mist', 'sage'], ['Nord frost', 'nord'], ['Rosé pine', 'rose'], ['Solarized', 'solarized']];
	for (const theme of ['Light', 'Dark']) {
		await page.getByRole('group', { name: 'Theme' }).getByRole('button', { name: theme }).click();
		await expect(page.locator('html')).toHaveAttribute('data-theme', theme.toLowerCase());
		for (const [name, id] of surfaces) {
			await page.getByRole('group', { name: 'Surface palette' }).getByRole('button', { name }).click();
			await expect(page.locator('html')).toHaveAttribute('data-surface', id);
			await expectNoAxeViolations(page, `settings/${theme}/${name}`);
		}
	}
});

test('keeps legacy links working', async ({ page, origin }) => {
	await signIn(page, origin);
	await page.goto(`${origin}/#/statistics`);
	await expect(page).toHaveURL(/#\/progress$/);
	await expect(page.getByRole('heading', { name: 'Progress' })).toBeVisible();
});

test('keeps the sign-in page scrollable on a short viewport', async ({ page, origin }) => {
	// About 400% browser zoom on a 1280 x 1024 window, or a phone held sideways. The document is not
	// locked outside the shell, so the wheel still reaches the button (issue #607).
	await page.setViewportSize({ width: 360, height: 260 });
	await page.goto(`${origin}/#/notes`);
	const signInButton = page.getByRole('button', { name: 'Sign in' });
	await expect(signInButton).toBeVisible();
	expect(await page.evaluate(() => getComputedStyle(document.documentElement).overflowY)).not.toBe('hidden');
	await page.mouse.move(180, 130);
	await page.mouse.wheel(0, 600);
	await expect(signInButton).toBeInViewport();
});

test('pins the side rail or top bar and scrolls the content region beside or below it', async ({ page, origin }) => {
	await page.setViewportSize({ width: 1440, height: 900 });
	await signIn(page, origin);
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

	// The document no longer scrolls: the region beside the rail does (issue #607).
	const bar = page.locator('header[data-app-chrome]');
	const scroller = page.locator('#main');
	expect(await page.evaluate(() => getComputedStyle(document.documentElement).overflowY)).toBe('hidden');
	await page.evaluate(() => window.scrollTo(0, 500));
	expect(await page.evaluate(() => window.scrollY)).toBe(0);
	expect(await scroller.evaluate((main) => getComputedStyle(main).overflowY)).toBe('auto');
	expect(await scroller.evaluate((main) => main.scrollHeight > main.clientHeight)).toBe(true);

	// Desktop: the rail is a fixed column on the left, and the content scrolls to its right.
	const railBox = (await bar.boundingBox())!;
	const mainBox = (await scroller.boundingBox())!;
	expect(railBox.x).toBe(0);
	expect(railBox.width).toBeCloseTo(236, 0);
	expect(mainBox.x).toBeGreaterThanOrEqual(railBox.x + railBox.width);

	await scroller.evaluate((main) => { main.scrollTop = main.scrollHeight; });
	await expect.poll(() => scroller.evaluate((main) => main.scrollTop)).toBeGreaterThan(0);
	// Scrolling to the end leaves the rail and its account menu in place.
	expect((await bar.boundingBox())!).toEqual(railBox);
	await expect(bar.getByRole('button', { name: 'Account: admin' })).toBeVisible();
	await expectNoAxeViolations(page, 'settings scrolled');

	// Phones: the top bar is pinned above the content region, and the phone tab bar floats over the bottom edge.
	await page.setViewportSize({ width: 375, height: 700 });
	const topBar = (await bar.boundingBox())!;
	expect(topBar.y).toBe(0);
	expect((await scroller.boundingBox())!.y).toBeGreaterThanOrEqual(topBar.y + topBar.height);
	await scroller.evaluate((main) => { main.scrollTop = main.scrollHeight; });
	await expect.poll(() => scroller.evaluate((main) => main.scrollTop)).toBeGreaterThan(0);
	expect((await bar.boundingBox())!.y).toBe(topBar.y);
	const tabs = page.getByRole('navigation', { name: 'Sections', exact: true });
	await expect(tabs).toBeVisible();
	const tabsTop = (await tabs.boundingBox())!.y;
	expect(await page.locator('#appearance').evaluate((section) => section.getBoundingClientRect().bottom)).toBeLessThanOrEqual(tabsTop);
});


test('admin settings pass axe and new accounts start with no course access', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.goto(`${origin}/#/settings#users`);
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
	await expect(page.getByRole('heading', { name: 'Sign in', level: 1 })).toBeVisible();
	await page.getByLabel('Username').fill('ana');
	await page.getByLabel('Password', { exact: true }).fill('a long student passphrase');
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByText('Bring in the courses you already have')).toBeVisible();
	await expect(page.getByRole('button', { name: 'Import course' })).toHaveCount(0);
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Accounts and course access' })).toHaveCount(0);
	expect(violations).toEqual([]);
});

test('the sign-in page shows the LearnVault wordmark and the Sapere aude motto in both languages', async ({ page, origin }) => {
	await page.goto(`${origin}/#/login`);
	const motto = page.locator('[lang="la"]', { hasText: 'Sapere aude' });
	await expect(motto).toBeVisible();
	await expect(page.getByText('Dare to know')).toBeVisible();
	await expect(page.getByText('LearnVault', { exact: true })).toBeVisible();

	await page.getByRole('button', { name: /Switch language to/ }).click();
	await expect(page.getByText('Ouse saber')).toBeVisible();
	// The Latin quotation is not translated and keeps its language tag.
	await expect(motto).toBeVisible();
	await expectNoAxeViolations(page, 'login pt-BR');

	// The choice survives a reload.
	await page.reload();
	await expect(page.getByText('Ouse saber')).toBeVisible();
});

test('the sign-in form works from the keyboard alone', async ({ page, origin }) => {
	await page.goto(`${origin}/#/login`);
	await page.getByLabel('Username').focus();
	await page.keyboard.type('admin');
	await page.keyboard.press('Tab');
	await expect(page.getByLabel('Password', { exact: true })).toBeFocused();
	await page.keyboard.type(PASSWORD);
	await page.keyboard.press('Enter');
	await expect(page.getByRole('link', { name: 'My learning' }).first()).toBeVisible();
});

test('an unknown address lands on a working page instead of a blank screen', async ({ page, origin }) => {
	const errors: string[] = [];
	page.on('pageerror', (error) => errors.push(error.message));
	await signIn(page, origin);
	await page.goto(`${origin}/#/this/route/does/not/exist`);
	await expect(page.locator('#main')).toBeVisible();
	await expect(page.getByRole('link', { name: 'My learning' }).first()).toBeVisible();
	expect(errors).toEqual([]);
});

test('an expired session sends the visitor back to sign in and returns them to the page they were on', async ({ page, origin }) => {
	await signIn(page, origin);
	await page.getByRole('link', { name: 'My learning' }).first().waitFor();
	// The session row disappears on the server (expiry, or a sign-out in another tab).
	await page.request.post(`${origin}/api/logout`, { headers: { Origin: origin } });
	let sessionChecks = 0;
	page.on('request', (request) => { if (new URL(request.url()).pathname === '/api/me') sessionChecks += 1; });
	await page.getByRole('link', { name: 'Notes' }).first().click();
	await expect(page.getByRole('heading', { name: 'Sign in', level: 1 })).toBeVisible();
	// One check answers the question; a loop of checks was the bug.
	expect(sessionChecks).toBeLessThanOrEqual(2);
	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('heading', { name: 'Notes', level: 1 })).toBeVisible();
});

test('the top bar fits a 320 px phone with every control inside the screen', async ({ page, origin }) => {
	await page.setViewportSize({ width: 320, height: 640 });
	await signIn(page, origin);
	await page.getByRole('link', { name: 'My learning' }).first().waitFor({ state: 'attached' });
	const rights = await page.locator('header button, header a').evaluateAll((elements) => elements.filter((element) => element.getBoundingClientRect().width > 0).map((element) => Math.round(element.getBoundingClientRect().right)));
	expect(Math.max(...rights)).toBeLessThanOrEqual(320);
});

test('imports a course folder from the interface and refuses a folder outside the library', async ({ page, origin, libraryDir }) => {
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Course', level: 3, exact: true })).toHaveCount(1);

	// A second course appears on disk after the server started.
	const folder = join(libraryDir, 'Second course');
	await mkdir(join(folder, 'Part one'), { recursive: true });
	await writeFile(join(folder, 'Part one', 'Intro.mp4'), Buffer.from('video-fixture'));

	await page.getByRole('button', { name: 'Import course' }).click();
	const dialog = page.getByRole('dialog', { name: 'Import a course' });
	await dialog.getByLabel(/folder|path/i).fill(join(libraryDir, '..', '..', 'not-in-the-library'));
	await dialog.getByRole('button', { name: 'Import course' }).click();
	await expect(dialog.getByRole('alert')).toBeVisible();
	await expect(dialog).toBeVisible();

	await dialog.getByLabel(/folder|path/i).fill(folder);
	await dialog.getByRole('button', { name: 'Import course' }).click();
	await expect(dialog).toBeHidden();
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByRole('heading', { name: 'Second course', level: 3 })).toBeVisible();
	await expect(page.getByRole('heading', { name: 'Course', level: 3, exact: true })).toHaveCount(1);
	// The folder on disk was only read.
	expect((await readdir(folder)).sort()).toEqual(['Part one']);
});
