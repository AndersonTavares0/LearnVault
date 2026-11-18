import { expect } from '@playwright/test';
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
	await expect(page.getByRole('complementary', { name: 'Course content' })).toBeVisible();
	await page.getByRole('tab', { name: 'Notes' }).click();
	await page.getByRole('button', { name: /Add note at/ }).click();
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

	await page.getByRole('link', { name: 'Course', exact: true }).click();
	await expect(page.getByRole('heading', { level: 1, name: 'Course' })).toBeVisible();
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

test('pins the top bar and scrolls the content region below it', async ({ page, origin }) => {
	await signIn(page, origin);
	await page.goto(`${origin}/#/settings`);
	await expect(page.getByRole('heading', { name: 'Settings' })).toBeVisible();

	// The document no longer scrolls: the region under the bar does (issue #607).
	const bar = page.locator('header');
	const scroller = page.locator('#main');
	expect(await page.evaluate(() => getComputedStyle(document.documentElement).overflowY)).toBe('hidden');
	await page.evaluate(() => window.scrollTo(0, 500));
	expect(await page.evaluate(() => window.scrollY)).toBe(0);
	expect(await scroller.evaluate((main) => getComputedStyle(main).overflowY)).toBe('auto');
	expect(await scroller.evaluate((main) => main.scrollHeight > main.clientHeight)).toBe(true);

	const barBox = (await bar.boundingBox())!;
	const mainBox = (await scroller.boundingBox())!;
	// The scrollbar belongs to the region under the bar, not to the whole window.
	expect(mainBox.y).toBeGreaterThanOrEqual(barBox.y + barBox.height);

	await scroller.evaluate((main) => { main.scrollTop = main.scrollHeight; });
	await expect.poll(() => scroller.evaluate((main) => main.scrollTop)).toBeGreaterThan(0);
	// Scrolling to the end leaves the bar and its account menu in place.
	expect((await bar.boundingBox())!.y).toBe(barBox.y);
	await expect(bar.getByRole('button', { name: 'Account: admin' })).toBeVisible();
	await expectNoAxeViolations(page, 'settings scrolled');

	// The phone tab bar keeps floating over the bottom edge, clear of the last content.
	await page.setViewportSize({ width: 375, height: 700 });
	const tabs = page.getByRole('navigation', { name: 'Sections', exact: true });
	await expect(tabs).toBeVisible();
	await scroller.evaluate((main) => { main.scrollTop = main.scrollHeight; });
	await expect.poll(() => scroller.evaluate((main) => main.scrollTop)).toBeGreaterThan(0);
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
