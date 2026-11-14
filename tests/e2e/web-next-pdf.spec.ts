import { expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { makePdf } from './fixtures/make-pdf';
import { collectCspViolations, createTest, expectNoAxeViolations, signIn } from './web-next-support';

// A course folder that holds only documents: the catalog keeps the PDF as a loose material of
// the course folder, so the course page itself has to link it to the reader.
const test = createTest(async (library) => {
	await mkdir(library, { recursive: true });
	await writeFile(join(library, 'Ownership notes.pdf'), makePdf(['Ownership moves values.', 'Borrowing shares a value.']));
});

test('opens the PDF of a course that only holds materials and keeps its marks', async ({ page, origin }, testInfo) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.getByRole('link', { name: 'Course', exact: true }).first().click();
	await expect(page.getByRole('heading', { level: 1, name: 'Course' })).toBeVisible();
	await expectNoAxeViolations(page, 'course of documents');
	await page.getByRole('link', { name: 'Ownership notes.pdf' }).click();

	await expect(page.getByText('PDF document', { exact: true })).toBeVisible();
	await expect(page.getByRole('note')).toContainText('the original file is never changed');
	// pdf.js drew the page and laid real text over it.
	await expect(page.locator('.pdf-text-layer')).toContainText('Ownership moves values.', { timeout: 15_000 });
	await expect(page.getByLabel('Page', { exact: true })).toHaveValue('1');
	// Select the first line of text and highlight it. The mark is saved, drawn over the page, and listed.
	await page.evaluate(() => {
		const line = document.querySelector('.pdf-text-layer span');
		const range = document.createRange();
		range.selectNodeContents(line!);
		const selection = window.getSelection();
		selection?.removeAllRanges();
		selection?.addRange(range);
	});
	const savedResponse = page.waitForResponse((response) => response.url().endsWith('/api/study/pdf-annotations') && response.request().method() === 'POST');
	await page.getByRole('button', { name: 'Highlight' }).click();
	const saved = await (await savedResponse).json();
	await expect(page.getByRole('list').getByText('Ownership moves values.')).toBeVisible({ timeout: 10_000 });
	await expectNoAxeViolations(page, 'pdf lesson');

	// The mark is saved on the server, so it is still listed after a reload of the material.
	await page.reload();
	await expect(page.locator('.pdf-text-layer')).toContainText('Ownership moves values.', { timeout: 15_000 });
	await expect(page.getByRole('list').getByText('Ownership moves values.')).toBeVisible();

	// Existing marks may contain broken ToUnicode extraction. Keep their geometry and notes.
	const brokenQuote = 'HUD\u0003GR';
	const legacy = await page.request.post(`${origin}/api/study/pdf-annotations`, {
		headers: { Origin: origin },
		data: { media_id: saved.media_id, page: saved.page, kind: saved.kind, color: saved.color, rects: saved.rects, quote: brokenQuote, content: 'Keep this note' }
	});
	await expect(legacy).toBeOK();
	await page.reload();
	await expect(page.locator('.pdf-text-layer')).toContainText('Ownership moves values.', { timeout: 15_000 });
	await expect(page.getByText('The selected text could not be extracted reliably. The mark is still saved on its page.')).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Note for the mark on page 1' }).last()).toHaveValue('Keep this note');
	await page.setViewportSize({ width: 390, height: 844 });
	await page.screenshot({ path: testInfo.outputPath('unreadable-quote-mobile.png'), fullPage: true });
	await expectNoAxeViolations(page, 'pdf unreadable quote');
	const stored = await page.request.get(`${origin}/api/study/pdf-annotations?media_id=${encodeURIComponent(saved.media_id)}`);
	expect((await stored.json()).items.some((item: { quote: string; content: string }) => item.quote === brokenQuote && item.content === 'Keep this note')).toBeTruthy();
	expect(violations).toEqual([]);
});
