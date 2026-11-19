import { expect } from '@playwright/test';
import { mkdir, rename, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { TINY_WEBM_BASE64 } from './fixtures/tiny-webm';
import { createTest, signIn } from './web-next-support';

const test = createTest(async (course) => {
	for (const lesson of ['01 Lesson', '02 Next lesson']) {
		await mkdir(join(course, lesson), { recursive: true });
		await writeFile(join(course, lesson, 'Lesson.webm'), Buffer.from(TINY_WEBM_BASE64, 'base64'));
	}
});

test('renaming a course folder marks the old course media missing', async ({ page, origin, libraryDir }) => {
	await signIn(page, origin);
	const refresh = page.getByRole('button', { name: 'Refresh courses' });
	const updated = page.getByText('Catalog updated.', { exact: true });
	await refresh.click();
	await expect(updated).toBeVisible();

	await rename(join(libraryDir, 'Course'), join(libraryDir, 'Moved course'));
	const rescan = page.waitForResponse((response) =>
		new URL(response.url()).pathname === '/api/admin/catalog/rescan' && response.request().method() === 'POST'
	);
	await refresh.click();
	expect((await rescan).ok()).toBe(true);
	await expect(updated).toBeVisible();
	await page.getByRole('link', { name: 'Course', exact: true }).click();
	await expect(page.getByText('Missing media', { exact: true })).toHaveCount(2);
});
