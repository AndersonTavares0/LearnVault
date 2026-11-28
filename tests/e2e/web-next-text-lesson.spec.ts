import { expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { collectCspViolations, createTest, expectNoAxeViolations, signIn } from './web-next-support';

// A lesson that holds only a text file shows the text on the page. The text is never read as
// HTML, and a Latin-1 file is decoded correctly.
const test = createTest(async (library) => {
	await mkdir(join(library, 'Module', 'Reading'), { recursive: true });
	await writeFile(join(library, 'Module', 'Reading', 'summary.md'), 'Ownership: cada valor tem um dono.\n<img src=x onerror="document.title=\'hacked\'">\n');
	await mkdir(join(library, 'Module', 'Legacy'), { recursive: true });
	await writeFile(join(library, 'Module', 'Legacy', 'latin1.txt'), Buffer.from([0x41, 0x75, 0x6c, 0x61, 0x20, 0xe7, 0xe3, 0x6f]));
});

test('a lesson with only a text file shows the text, as text', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	await page.getByRole('combobox', { name: 'Search notes, courses and lessons' }).fill('summary');
	await page.getByRole('group', { name: 'Lessons' }).getByRole('option', { name: /summary/ }).click();

	const article = page.getByRole('article', { name: 'summary.md' });
	await expect(article).toContainText('Ownership: cada valor tem um dono.', { timeout: 10_000 });
	await expect(article).toContainText('<img src=x onerror=');
	await expect(page).toHaveTitle(/LearnVault/);
	await expect(page.getByText('This lesson has no media to play.')).toHaveCount(0);
	await expect(page.getByRole('note')).toContainText('the original file is never changed');
	await expectNoAxeViolations(page, 'text lesson');

	await page.getByRole('link', { name: /Legacy/ }).first().click();
	await expect(page.getByRole('article', { name: 'latin1.txt' })).toContainText('Aula ção', { timeout: 10_000 });
	expect(violations).toEqual([]);
});
