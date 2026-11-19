import { expect } from '@playwright/test';
import { basicLibrary, collectCspViolations, createTest, expectNoAxeViolations, signIn } from './web-next-support';

const test = createTest(basicLibrary);

test('shows the installed version and accessible seal in About', async ({ page, origin }, testInfo) => {
	const violations = collectCspViolations(page);
	const externalRequests: string[] = [];
	page.on('request', (request) => {
		if (/^https?:/.test(request.url()) && new URL(request.url()).origin !== origin) externalRequests.push(request.url());
	});
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	const { version } = await (await page.request.get(`${origin}/api/healthz`)).json();
	expect(version).toBeTruthy();

	for (const theme of ['Light', 'Dark']) {
		await page.setViewportSize({ width: 1440, height: 900 });
		await page.goto(`${origin}/#/settings`);
		await page.getByRole('button', { name: 'Appearance', exact: true }).click();
		await page.getByRole('group', { name: 'Theme' }).getByRole('button', { name: theme }).click();
		await expect(page.locator('html')).toHaveAttribute('data-theme', theme.toLowerCase());

		for (const width of [1440, 320]) {
			await page.setViewportSize({ width, height: 900 });
			if (width === 1440) await page.getByRole('button', { name: 'About', exact: true }).click();
			else await page.getByLabel('Settings section', { exact: true }).selectOption('about');

			const about = page.getByRole('region', { name: 'About', exact: true });
			await expect(about.getByText(`Version ${version}`, { exact: true })).toBeVisible();
			await expect(about.getByText('Sapere aude', { exact: true })).toHaveAttribute('lang', 'la');
			await expect(about.getByText('Dare to know', { exact: true })).toBeVisible();
			await expect(about.getByText('LearnVault only reads your course folders.')).toBeVisible();
			await expect(about.getByRole('link', { name: 'Project repository' })).toHaveAttribute('href', 'https://github.com/AndersonTavares0/LearnVault');

			const seal = about.getByRole('img', { name: 'LearnVault seal' });
			await expect(seal).toBeVisible();
			await expect(seal).toHaveCSS('width', '160px');
			await expect(seal).toHaveCSS('height', '160px');
			await expect(seal).toHaveAttribute('src', theme === 'Dark' ? /learnvault-seal-dark-/ : /learnvault-seal-(?!dark-)/);
			await expect.poll(() => seal.evaluate((img: HTMLImageElement) => img.naturalWidth)).toBeGreaterThan(0);
			expect(await page.locator('#main').evaluate((main) => main.scrollWidth <= main.clientWidth)).toBe(true);
			await expectNoAxeViolations(page, `settings/about/${theme}/${width}`);
			await page.screenshot({ path: testInfo.outputPath(`about-${theme.toLowerCase()}-${width}.png`) });
		}
	}

	await page.getByLabel('Settings section', { exact: true }).selectOption('appearance');
	await page.getByRole('button', { name: /Português/ }).click();
	await page.getByLabel('Seção dos ajustes', { exact: true }).selectOption('about');
	const sobre = page.getByRole('region', { name: 'Sobre', exact: true });
	await expect(sobre.getByText(`Versão ${version}`, { exact: true })).toBeVisible();
	await expect(sobre.getByRole('img', { name: 'Selo do LearnVault' })).toBeVisible();
	await expect(sobre.getByText('Ouse saber', { exact: true })).toBeVisible();
	await expect(sobre.getByText('O LearnVault apenas lê suas pastas de cursos.')).toBeVisible();
	await expect(sobre.getByRole('link', { name: 'Repositório do projeto' })).toBeVisible();

	expect(externalRequests).toEqual([]);
	expect(violations).toEqual([]);
});
