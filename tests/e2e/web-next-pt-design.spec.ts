import { expect } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { TINY_VP9_WEBM_BASE64 } from './fixtures/tiny-webm';
import { createTest, expectNoAxeViolations, signIn } from './web-next-support';

const test = createTest(async (course) => {
	for (const title of ['01 Captação de som', '02 Montagem e tratamento']) {
		const folder = join(course, title);
		await mkdir(folder, { recursive: true });
		await writeFile(join(folder, `${title}.webm`), Buffer.from(TINY_VP9_WEBM_BASE64, 'base64'));
	}
});

test('Portuguese study screens reflow in light and dark themes', async ({ page, origin }, info) => {
	test.setTimeout(60_000);
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.')).toBeVisible();
	const courseLink = await page.getByRole('link', { name: 'Course', exact: true }).first().getAttribute('href');
	await page.goto(new URL(courseLink!, origin).href);
	const lessonLink = await page.getByRole('link', { name: 'Start course' }).getAttribute('href');
	await page.goto(`${origin}/#/settings`);
	await page.getByRole('button', { name: 'Português (Brasil)' }).click();
	await expect(page.locator('html')).toHaveAttribute('lang', 'pt-BR');

	for (const theme of ['dark', 'light']) {
		if (theme === 'light') await page.getByRole('button', { name: /tema claro/i }).click();
		await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
		for (const width of [1440, 375]) {
			await page.setViewportSize({ width, height: 900 });
			for (const [surface, location, heading] of [
				['biblioteca', '#/', 'Meu aprendizado'], ['curso', courseLink!, 'Course'],
				['aula', lessonLink!, /Captação de som/i], ['revisao', '#/review', 'Cartões para revisar'],
				['progresso', '#/progress', 'Seu progresso'], ['notas', '#/notes', 'Notas']
			]) {
				await page.goto(new URL(location, origin).href);
				await expect(page.getByRole('heading', { level: 1, name: heading, exact: true })).toBeVisible();
				if (surface === 'aula') await expect(page.getByRole('slider', { name: /posição|buscar/i })).toBeVisible();
				if (surface === 'progresso') await expect(page.getByText('Nível 1 · 0 de 100 XP para o próximo nível')).toBeVisible();
				await page.evaluate(() => document.fonts.ready.then(() => undefined));
				const context = `${surface}-${theme}-${width}`;
				expect(await page.locator('main').evaluate((main) => main.scrollWidth - main.clientWidth), context).toBeLessThanOrEqual(1);
				await page.screenshot({ path: info.outputPath(`${context}.png`) });
				await expectNoAxeViolations(page, context, surface === 'aula' ? ['video-caption'] : []);
			}
			await page.goto(`${origin}/#/`);
			if (width < 1024) await page.getByRole('button', { name: 'Buscar', exact: true }).click();
			const search = page.getByRole('combobox', { name: 'Buscar notas, cursos e aulas' });
			await search.fill('captação');
			await expect(page.getByRole('option', { name: /captação de som/i }).first()).toBeVisible();
			await expect(page.getByText('1 resultado', { exact: true })).toBeVisible();
			await search.press('ArrowDown');
			const selected = page.getByRole('listbox', { name: 'Resultados da busca' }).getByRole('option', { selected: true });
			await expect(selected).toHaveCSS('outline-width', '2px');
			await page.screenshot({ path: info.outputPath(`busca-${theme}-${width}.png`) });
			await expectNoAxeViolations(page, `busca-${theme}-${width}`);
			await search.press('Escape');
		}
	}
});
