import { expect } from '@playwright/test';
import { basicLibrary, createTest, expectNoAxeViolations, signIn } from './web-next-support';

const test = createTest(basicLibrary);

test('progress heatmap scroll region is keyboard-accessible at 390 px', async ({ page, origin }) => {
	await page.setViewportSize({ width: 390, height: 844 });
	await page.emulateMedia({ reducedMotion: 'reduce' });

	await signIn(page, origin);
	await page.goto(`${origin}/#/progress`);
	await expect(page.getByRole('heading', { level: 1, name: 'Your progress' })).toBeVisible();
	await expect(page.getByRole('heading', { level: 2, name: 'Study days' })).toBeVisible();

	await expectNoAxeViolations(page, 'progress-390');
});
