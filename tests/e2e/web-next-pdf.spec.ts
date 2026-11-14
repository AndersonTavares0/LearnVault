import { expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { makePdf } from './fixtures/make-pdf';
import { collectCspViolations, createTest, expectNoAxeViolations, signIn } from './web-next-support';

// A course folder that holds only documents: the catalog keeps the PDF as a loose material of
// the course folder, so the course page itself has to link it to the reader.
const test = createTest(async (library) => {
	await mkdir(join(library, 'Reading'), { recursive: true });
	await writeFile(join(library, 'Reading', 'Ownership notes.pdf'), makePdf(['Ownership moves values.', 'Borrowing shares a value.']));
});

test('opens the PDF of a course that only holds materials and keeps its marks', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.getByRole('link', { name: 'library' }).first().click();
	await expect(page.getByRole('heading', { level: 1, name: 'library' })).toBeVisible();
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
	await page.getByRole('button', { name: 'Highlight' }).click();
	await expect(page.getByRole('list').getByText('Ownership moves values.')).toBeVisible({ timeout: 10_000 });
	await expectNoAxeViolations(page, 'pdf lesson');

	// The mark is saved on the server, so it is still listed after a reload of the material.
	await page.reload();
	await expect(page.locator('.pdf-text-layer')).toContainText('Ownership moves values.', { timeout: 15_000 });
	await expect(page.getByRole('list').getByText('Ownership moves values.')).toBeVisible();
	expect(violations).toEqual([]);
});
