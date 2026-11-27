import { expect, type Page } from '@playwright/test';
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { TINY_WEBM_BASE64 } from './fixtures/tiny-webm';
import { createTest, signIn } from './web-next-support';

const LONG_COURSE_TITLE =
	'Complete Distributed Systems Architecture, Fault-Tolerant Consensus Protocols, Raft, Paxos, and Production Cloud Engineering Masterclass Edition';

const test = createTest(async (course) => {
	const longCourse = join(dirname(course), LONG_COURSE_TITLE);
	for (const lesson of ['01 Consensus', '02 Replication']) {
		await mkdir(join(longCourse, lesson), { recursive: true });
		await writeFile(join(longCourse, lesson, 'Lesson.webm'), Buffer.from(TINY_WEBM_BASE64, 'base64'));
	}
});

async function assertFocusedControlNotObscured(page: Page, expectedLabel: string) {
	const result = await page.evaluate(() => {
		const active = document.activeElement as HTMLElement | null;
		if (!active) return { ok: false, reason: 'no activeElement' };

		const rect = active.getBoundingClientRect();
		const cx = rect.left + rect.width / 2;
		const cy = rect.top + rect.height / 2;
		const hit = document.elementFromPoint(cx, cy) as HTMLElement | null;
		const nav = document.querySelector('nav[aria-label="Sections"]') as HTMLElement | null;
		const navRect = nav?.getBoundingClientRect() ?? null;

		const hitIsControl = Boolean(hit && (hit === active || active.contains(hit)));
		const hitIsNav = Boolean(hit && nav && (hit === nav || nav.contains(hit)));

		return {
			ok: hitIsControl && !hitIsNav && (navRect ? rect.bottom <= navRect.top : true),
			activeTag: active.tagName,
			activeText: (active.getAttribute('aria-label') || active.textContent || '').trim(),
			hitTag: hit?.tagName ?? null,
			hitText: (hit?.getAttribute('aria-label') || hit?.textContent || '').trim(),
			hitIsControl,
			hitIsNav,
			rectBottom: Math.round(rect.bottom),
			navTop: navRect ? Math.round(navRect.top) : null,
		};
	});

	expect(
		result.ok,
		`Focused control "${expectedLabel}" was obscured: ${JSON.stringify(result)}`,
	).toBe(true);
}

test('mobile bottom navigation does not obscure focused course controls (issue #642)', async ({
	page,
	origin,
}) => {
	await page.setViewportSize({ width: 390, height: 760 });
	await page.emulateMedia({ reducedMotion: 'reduce' });

	await signIn(page, origin);
	await page.getByRole('button', { name: 'Refresh courses' }).click();
	await expect(page.getByText('Catalog updated.', { exact: true })).toBeVisible();

	await page.getByRole('link', { name: new RegExp(LONG_COURSE_TITLE) }).first().click();
	await expect(page.getByRole('heading', { level: 1, name: LONG_COURSE_TITLE })).toBeVisible();
	await expect(page.getByRole('navigation', { name: 'Sections' })).toBeVisible();

	// Reload so sequential focus starts from the top of the document, then Tab -> Skip to content.
	await page.reload();
	await expect(page.getByRole('heading', { level: 1, name: LONG_COURSE_TITLE })).toBeVisible();
	await page.keyboard.press('Tab');
	await expect(page.getByRole('link', { name: 'Skip to content' })).toBeFocused();
	await page.keyboard.press('Enter');
	await expect(page.locator('#main')).toBeFocused();

	const targets = ['Start course', 'Favorite course', 'Manage course'];
	const seen = new Set<string>();

	for (let step = 0; step < 15 && seen.size < targets.length; step++) {
		await page.keyboard.press('Tab');
		const label = await page.evaluate(() => {
			const el = document.activeElement as HTMLElement | null;
			return (el?.getAttribute('aria-label') || el?.textContent || '').trim();
		});
		const matched = targets.find((target) => label.includes(target));
		if (matched && !seen.has(matched)) {
			seen.add(matched);
			await assertFocusedControlNotObscured(page, matched);
		}
	}

	expect([...seen]).toEqual(targets);
});

test('phone search opens focused, keeps Tab inside, and returns focus on dismissal', async ({ page, origin }) => {
	await page.setViewportSize({ width: 390, height: 760 });
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await signIn(page, origin);

	const tab = page.getByRole('navigation', { name: 'Sections' }).getByRole('button', { name: 'Search' });
	const search = page.getByRole('combobox', { name: 'Search notes, courses and lessons' });
	const close = page.getByRole('button', { name: 'Close search' });

	await tab.click();
	await expect(search).toBeFocused();
	await expect(search).toBeVisible();

	await page.keyboard.press('Tab');
	await expect(close).toBeFocused();
	await page.keyboard.press('Tab');
	await expect(search).toBeFocused();
	await page.keyboard.press('Shift+Tab');
	await expect(close).toBeFocused();

	await page.keyboard.press('Escape');
	await expect(tab).toBeFocused();
	await expect(search).toBeHidden();

	// The shortcut also opens the full-screen palette on a phone, where the rail field is hidden.
	await page.keyboard.press('/');
	await expect(search).toBeFocused();
	await expect(search).toBeVisible();
	await close.click();
	await expect(tab).toBeFocused();

	await page.setViewportSize({ width: 1280, height: 800 });
	await page.keyboard.press('/');
	await expect(search).toBeFocused();
	await expect(search).toBeVisible();
});
