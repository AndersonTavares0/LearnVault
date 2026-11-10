import AxeBuilder from '@axe-core/playwright';
import { expect, test as base, type Page } from '@playwright/test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

// PDF lessons: a real PDF rendered by pdf.js behind the real Rust server and its CSP.
// Folded into server.spec.ts at cutover (#340).
/** A valid two-page PDF with one line of text per page (Helvetica, so no font is embedded). */
function tinyPdf(lines: string[]): Buffer {
	const objects: string[] = [];
	const kids = lines.map((_, index) => `${3 + index * 2} 0 R`).join(' ');
	objects.push('<</Type/Catalog/Pages 2 0 R>>', `<</Type/Pages/Kids[${kids}]/Count ${lines.length}>>`);
	lines.forEach((line, index) => {
		const font = 3 + lines.length * 2;
		const content = `BT /F1 24 Tf 20 100 Td (${line}) Tj ET`;
		objects.push(`<</Type/Page/Parent 2 0 R/MediaBox[0 0 400 200]/Contents ${4 + index * 2} 0 R/Resources<</Font<</F1 ${font} 0 R>>>>>>`, `<</Length ${content.length}>>\nstream\n${content}\nendstream`);
	});
	objects.push('<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>');
	let body = '%PDF-1.4\n';
	const offsets = objects.map((object, index) => { const at = body.length; body += `${index + 1} 0 obj\n${object}\nendobj\n`; return at; });
	const xref = body.length;
	body += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n${offsets.map((at) => `${String(at).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<</Size ${objects.length + 1}/Root 1 0 R>>\nstartxref\n${xref}\n%%EOF`;
	return Buffer.from(body, 'latin1');
}

const PASSWORD = 'uma senha local muito segura';

async function signIn(page: Page, origin: string) {
	await page.goto(`${origin}/#/login`);
	await page.getByLabel('Username').fill('admin');
	await page.getByLabel('Password', { exact: true }).fill(PASSWORD);
	await page.getByRole('button', { name: 'Sign in' }).click();
	await expect(page.getByRole('heading', { name: 'My learning' })).toBeVisible();
}

/** Fails on any WCAG 2.2 AA issue axe can detect on the current page. */
async function expectNoAxeViolations(page: Page, context: string) {
	const results = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']).analyze();
	expect(results.violations.flatMap((violation) => violation.nodes.map((node) => `${context}: ${violation.id} ${node.target.join(' ')} ${node.failureSummary ?? ''}`))).toEqual([]);
}

function collectCspViolations(page: Page): string[] {
	const violations: string[] = [];
	page.on('console', (message) => { if (message.type() === 'error' && /Content Security Policy/.test(message.text())) violations.push(message.text()); });
	return violations;
}
const test = base.extend<object, { origin: string }>({
	origin: [async ({}, use) => {
		const root = await mkdtemp(join(tmpdir(), 'learnvault-web-next-'));
		const library = join(root, 'library');
		await mkdir(join(library, 'Reading', 'Ownership notes'), { recursive: true });
		await writeFile(join(library, 'Reading', 'Ownership notes', 'Ownership notes.pdf'), tinyPdf(['Ownership moves values.', 'Borrowing shares a value.']));
		const binary = process.env.LEARNVAULT_SERVER_BIN ?? resolve(process.platform === 'win32' ? 'target/debug/learnvault-server.exe' : 'target/debug/learnvault-server');
		const server = spawn(binary, ['--listen-addr', '127.0.0.1:0', '--database', join(root, 'db.sqlite'), '--library-root', library, '--web-root', resolve('apps/web-next/dist'), '--insecure-http'], {
			env: { ...process.env, ADMIN_USERNAME: 'admin', ADMIN_PASSWORD: PASSWORD },
			stdio: ['ignore', 'pipe', 'ignore']
		});
		const closed = once(server, 'close');
		let output = '';
		const address = await new Promise<string>((ok, fail) => {
			server.stdout!.on('data', (chunk) => {
				output += chunk;
				const line = output.split(/\r?\n/).find((entry) => entry.startsWith('LEARNVAULT_READY '));
				if (line) ok(line.slice('LEARNVAULT_READY '.length).trim());
			});
			server.once('close', () => fail(new Error(`server exited: ${output}`)));
		});
		try {
			await use(`http://${address}`);
		} finally {
			server.kill('SIGTERM');
			await closed;
			await rm(root, { recursive: true, force: true });
		}
	}, { scope: 'worker' }]
});


test('reads a PDF lesson in the page, with its text selectable and its page remembered', async ({ page, origin }) => {
	const violations = collectCspViolations(page);
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	// The whole card is the link, so a click in the middle of the cover opens the course.
	await page.locator('li.group\\/card').first().click();
	await expect(page.getByRole('heading', { level: 1, name: 'library' })).toBeVisible();
	// A module with no lesson to continue starts collapsed.
	await page.getByRole('button', { name: 'Expand all' }).click();
	await page.getByRole('link', { name: /Ownership notes/ }).first().click();

	await expect(page.getByText('PDF document', { exact: true })).toBeVisible();
	await expect(page.getByRole('note')).toContainText('the original file is never changed');
	// pdf.js drew the page and laid real text over it.
	await expect(page.locator('.pdf-text-layer')).toContainText('Ownership moves values.', { timeout: 15_000 });
	await expect(page.getByLabel('Page', { exact: true })).toHaveValue('1');
	await expectNoAxeViolations(page, 'pdf lesson');

	await page.getByRole('button', { name: 'Next page' }).click();
	await expect(page.locator('.pdf-text-layer')).toContainText('Borrowing shares a value.', { timeout: 15_000 });
	await page.getByRole('button', { name: 'Zoom in' }).click();
	await expect(page.getByText('125%')).toBeVisible();

	// The page is saved as reading progress, so coming back opens on it.
	await page.waitForTimeout(1500);
	await page.reload();
	await expect(page.getByLabel('Page', { exact: true })).toHaveValue('2');
	expect(violations).toEqual([]);
});
