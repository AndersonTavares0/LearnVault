import { expect, type Page } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { buildPdf } from './fixtures/make-pdf';
import { collectCspViolations, createTest, expectNoAxeViolations, signIn } from './web-next-support';

// Continuous reading, keyboard shortcuts and night mode in the PDF reader, behind the real
// server and its CSP. Fixtures are generated in memory, so no PDF is committed.
/** A valid multi-page PDF with one line of text per page (Helvetica, so no font is embedded). */
function makePages(lines: string[]): Buffer {
	const font = 3 + lines.length * 2;
	const objects = [
		'<< /Type /Catalog /Pages 2 0 R >>',
		`<< /Type /Pages /Kids [${lines.map((_, index) => `${3 + index * 2} 0 R`).join(' ')}] /Count ${lines.length} >>`
	];
	lines.forEach((line, index) => {
		const content = `BT /F1 24 Tf 20 240 Td (${line.replace(/[()\\]/g, '\\$&')}) Tj ET`;
		objects.push(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 300] /Contents ${4 + index * 2} 0 R /Resources << /Font << /F1 ${font} 0 R >> >> >>`);
		objects.push(`<< /Length ${content.length} >>\nstream\n${content}\nendstream`);
	});
	objects.push('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
	return buildPdf(objects);
}

const test = createTest(async (library) => {
	await mkdir(join(library, 'Reading', 'Continuous'), { recursive: true });
	await writeFile(join(library, 'Reading', 'Continuous', 'Continuous.pdf'), makePages([
		'Page one: ownership.',
		'Page two: borrowing.',
		'Page three: lifetimes.',
		'Page four: traits.',
		'Page five: closures.'
	]));
});

const reader = (page: Page) => page.getByRole('group', { name: 'PDF page' });
const pageField = (page: Page) => page.getByLabel('Page', { exact: true });

async function openReader(page: Page, origin: string) {
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.getByRole('link', { name: 'Course' }).first().click();
	await page.getByRole('button', { name: 'Expand all' }).click();
	await page.getByRole('link', { name: /Continuous/ }).first().click();
	await expect(reader(page)).toBeVisible();
	await expect(page.locator('[data-page="1"] .pdf-text-layer')).toContainText('Page one: ownership.', { timeout: 15_000 });
}

test('continuous mode scrolls through the document and reports the page as progress', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await openReader(page, origin);
	// Continuous is the default reading mode.
	await page.getByRole('button', { name: 'Reading mode' }).click();
	await expect(page.getByRole('menuitemradio', { name: 'Continuous scrolling' })).toHaveAttribute('aria-checked', 'true');
	await page.keyboard.press('Escape');
	await reader(page).focus();
	await page.keyboard.press('j');
	await page.keyboard.press('j');
	await expect(pageField(page)).toHaveValue('3');
	await expect(page.locator('.pdf-text-layer').filter({ hasText: 'Page three: lifetimes.' })).toHaveCount(1);
	await expectNoAxeViolations(page, 'pdf reader light');
	// The saved page survives a reload, so the reader lands on it in continuous mode too.
	await page.waitForTimeout(1500);
	await page.reload();
	await expect(pageField(page)).toHaveValue('3');
	await expect(reader(page)).toBeVisible();
	expect(violations).toEqual([]);
});

test('switching to single page keeps the page the reader was on', async ({ page, origin }) => {
	await openReader(page, origin);
	await reader(page).focus();
	await page.keyboard.press('j');
	await page.keyboard.press('j');
	await expect(pageField(page)).toHaveValue('3');
	await page.getByRole('button', { name: 'Reading mode' }).click();
	await page.getByRole('menuitemradio', { name: 'One page at a time' }).click();
	await page.keyboard.press('Escape');
	await expect(pageField(page)).toHaveValue('3');
	await expect(page.locator('.pdf-text-layer')).toHaveCount(1);
	await expect(page.locator('.pdf-text-layer')).toContainText('Page three: lifetimes.');
});

test('marks the selected text while reading continuously', async ({ page, origin }) => {
	await openReader(page, origin);
	const layer = page.locator('[data-page="1"] .pdf-text-layer');
	await page.evaluate(() => {
		const line = document.querySelector('[data-page="1"] .pdf-text-layer span');
		const range = document.createRange();
		range.selectNodeContents(line!);
		const selection = window.getSelection();
		selection?.removeAllRanges();
		selection?.addRange(range);
	});
	await expect(layer).toContainText('Page one: ownership.');
	await page.getByRole('button', { name: 'Annotate' }).click();
	await page.getByRole('button', { name: 'Highlight' }).click();
	await expect(page.getByRole('list').getByText('Page one: ownership.')).toBeVisible({ timeout: 10_000 });
	await expect(page.locator('[data-page="1"] .mix-blend-multiply').first()).toBeVisible();
});

test('? opens the visible shortcut sheet, which traps focus and closes with Escape', async ({ page, origin }) => {
	await openReader(page, origin);
	await reader(page).focus();
	await page.keyboard.press('?');
	const sheet = page.getByRole('dialog', { name: 'Keyboard shortcuts' });
	await expect(sheet).toBeVisible();
	await expect(sheet.getByText('Next / previous page')).toBeVisible();
	// Tabbing stays inside the sheet, so the reader's single-key shortcuts are not in the way.
	await page.keyboard.press('Tab');
	await expect(sheet.getByRole('button', { name: 'Close' })).toBeFocused();
	await expectNoAxeViolations(page, 'pdf shortcut sheet');
	await page.keyboard.press('Escape');
	await expect(sheet).toBeHidden();
});

test('the night theme dims the pages, persists per user, and passes axe in the dark app theme', async ({ page, origin }) => {
	await openReader(page, origin);
	await page.getByRole('button', { name: 'Reading theme' }).click();
	await page.getByRole('menuitemradio', { name: 'Night' }).click();
	await page.keyboard.press('Escape');
	expect(await page.locator('[data-page="1"] canvas').evaluate((element) => getComputedStyle(element).filter)).toContain('invert');
	await page.getByRole('button', { name: 'Switch to the dark theme' }).click();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	await expectNoAxeViolations(page, 'pdf reader dark');
	await page.reload();
	await expect(page.locator('[data-page="1"] canvas')).toBeVisible();
	await page.getByRole('button', { name: 'Reading theme' }).click();
	await expect(page.getByRole('menuitemradio', { name: 'Night' })).toHaveAttribute('aria-checked', 'true');
});
