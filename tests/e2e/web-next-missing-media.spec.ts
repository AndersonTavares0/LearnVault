import { expect, type Page } from '@playwright/test';
import { mkdir, unlink, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { TINY_WEBM_BASE64 } from './fixtures/tiny-webm';
import { createTest, signIn } from './web-next-support';

const videoBytes = Buffer.from(TINY_WEBM_BASE64, 'base64');
const lessonFile = '01 Lesson/Lesson.webm';
const note = 'Remember this lesson after a library refresh.';

const test = createTest(async (course) => {
	for (const folder of ['01 Lesson', '02 Next lesson']) {
		await mkdir(join(course, folder), { recursive: true });
		await writeFile(join(course, folder, 'Lesson.webm'), videoBytes);
	}
});

async function refreshCourses(page: Page) {
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.', { exact: true })).toBeVisible();
}

async function openLessonWithStudyData(page: Page, origin: string) {
	await signIn(page, origin);
	await refreshCourses(page);
	await page.getByRole('link', { name: 'Course', exact: true }).click();
	await page.getByRole('link', { name: 'Start course' }).click();
	await expect(page.getByRole('slider', { name: 'Seek' })).toBeVisible();
	const completion = page.waitForResponse((response) =>
		new URL(response.url()).pathname === '/api/study/completed' && response.request().method() === 'POST'
	);
	await page.getByRole('button', { name: 'Mark as completed' }).click();
	const saved = await completion;
	expect(saved.ok()).toBe(true);
	await expect(page.getByRole('button', { name: 'Completed', exact: true, pressed: true })).toBeVisible();
	await page.getByRole('tab', { name: 'Notes', exact: true }).click();
	await page.getByRole('button', { name: /Write a note at/ }).click();
	await page.getByLabel(/Note at/).fill(note);
	await page.getByRole('button', { name: 'Save note' }).click();
	await expect(page.getByRole('tabpanel', { name: /^Notes/ }).getByText(note, { exact: true })).toBeVisible();
}

async function refreshFromLesson(page: Page) {
	await page.getByRole('link', { name: 'My learning', exact: true }).first().click();
	await refreshCourses(page);
}

test('a missing video leaves saved progress and notes visible on the lesson', async ({ page, origin, libraryDir }) => {
	await openLessonWithStudyData(page, origin);
	const lessonUrl = page.url();

	await unlink(join(libraryDir, 'Course', lessonFile));
	await refreshFromLesson(page);
	await page.goto(lessonUrl);
	await expect(page.getByText('Media unavailable', { exact: true })).toBeVisible();
	await expect(page.locator('video')).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Play', exact: true })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Completed', exact: true, pressed: true })).toBeVisible();
	const notesTab = page.getByRole('tab', { name: /^Notes/ });
	await expect(notesTab).toBeVisible();
	await notesTab.click();
	await expect(page.getByRole('tabpanel', { name: /^Notes/ }).getByText(note, { exact: true })).toBeVisible();
});
