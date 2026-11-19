import { expect, type Page } from '@playwright/test';
import { mkdir, readdir, rename, stat, unlink, writeFile } from 'node:fs/promises';
import { join, relative } from 'node:path';
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

async function manifest(root: string) {
	const paths = (await readdir(root, { recursive: true, withFileTypes: true }))
		.filter((entry) => !entry.isDirectory())
		.map((entry) => relative(root, join(entry.parentPath, entry.name)))
		.sort();
	return Promise.all(paths.map(async (path) => {
		const { size, mtimeMs } = await stat(join(root, path));
		return { path, size, mtimeMs };
	}));
}

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
	const { media_id: mediaId } = await saved.json() as { media_id: string };
	await expect(page.getByRole('button', { name: 'Completed', exact: true, pressed: true })).toBeVisible();
	await page.getByRole('tab', { name: 'Notes', exact: true }).click();
	await page.getByRole('button', { name: /Add note at/ }).click();
	await page.getByLabel(/Note at/).fill(note);
	await page.getByRole('button', { name: 'Save note' }).click();
	await expect(page.getByRole('tabpanel', { name: /^Notes/ }).getByText(note, { exact: true })).toBeVisible();
	return mediaId;
}

async function refreshFromLesson(page: Page) {
	await page.getByRole('link', { name: 'My learning', exact: true }).first().click();
	await refreshCourses(page);
}

async function expectSavedNoteInNotebook(page: Page, origin: string) {
	await page.goto(`${origin}/#/notes`);
	await expect(page.getByRole('heading', { name: 'Notes', level: 1 })).toBeVisible();
	await expect(page.getByText(note, { exact: true })).toBeVisible();
}

test('a deleted lesson video shows recovery and plays again with saved study data after restoration', async ({ page, origin, libraryDir }) => {
	const before = await manifest(libraryDir);
	const videoPath = join(libraryDir, 'Course', lessonFile);
	const mediaId = await openLessonWithStudyData(page, origin);
	const lessonUrl = page.url();
	expect(await manifest(libraryDir)).toEqual(before);

	await unlink(videoPath);
	await refreshFromLesson(page);
	await page.goto(lessonUrl);
	await expect(page.getByText('Media unavailable', { exact: true })).toBeVisible();
	await expect(page.locator('video')).toHaveCount(0);
	const progress = await page.request.get(`${origin}/api/study/progress`, { params: { media_id: mediaId } });
	await expect(progress).toBeOK();
	expect(await progress.json()).toMatchObject({ recorded: true, completed: true });
	await expectSavedNoteInNotebook(page, origin);
	expect(await manifest(libraryDir)).toEqual(before.filter(({ path }) => path !== join('Course', lessonFile)));

	await writeFile(videoPath, videoBytes);
	const restored = await manifest(libraryDir);
	await refreshFromLesson(page);
	await page.goto(lessonUrl);
	await expect(page.getByRole('button', { name: 'Completed', exact: true, pressed: true })).toBeVisible();
	await page.getByRole('tab', { name: /^Notes/ }).click();
	await expect(page.getByRole('tabpanel', { name: /^Notes/ }).getByText(note, { exact: true })).toBeVisible();
	await page.getByRole('button', { name: 'Play', exact: true }).click();
	await expect.poll(() => page.locator('video').evaluate((video: HTMLVideoElement) => video.currentTime)).toBeGreaterThan(0);
	expect(await manifest(libraryDir)).toEqual(restored);
});

async function renameCourseAndRefresh(page: Page, libraryDir: string) {
	await rename(join(libraryDir, 'Course'), join(libraryDir, 'Moved course'));
	await refreshFromLesson(page);
}

test('renaming a course folder preserves access to notes and leaves the renamed files untouched', async ({ page, origin, libraryDir }) => {
	const before = await manifest(libraryDir);
	await openLessonWithStudyData(page, origin);
	await renameCourseAndRefresh(page, libraryDir);
	await expect(page.getByRole('link', { name: 'Course', exact: true })).toBeVisible();
	await expectSavedNoteInNotebook(page, origin);
	expect(await manifest(libraryDir)).toEqual(before.map((file) => ({
		...file,
		path: join('Moved course', file.path.slice('Course'.length + 1))
	})));
});
