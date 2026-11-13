import { cleanup, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { setLocale } from '@/lib/i18n';
import type { Catalog, CatalogNode, FileEntry, StudyProgress } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const media = (id: string): FileEntry => ({ name: `${id}.mp4`, path: `m/${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' });
const lesson = (id: string, title: string): CatalogNode => ({ title, path: `m/${id}`, role: 'lesson', files: [media(id)], looseFiles: [], children: [] });
const catalog: Catalog = {
	complete: true,
	looseFiles: [],
	nodes: [{ title: 'Rust from zero', path: 'rust', role: 'course', courseId: 'rust', files: [], looseFiles: [], children: [lesson('r1', 'Ownership'), lesson('r2', 'Borrowing')] }]
};
const progress = (mediaId: string, positionSeconds: number, completed: boolean, updatedAt: number): StudyProgress => ({ mediaId, positionSeconds, completed, updatedAt, recorded: true });

afterEach(() => {
	cleanup();
	setLocale('en');
});

describe('Watch history', () => {
	it('lists watched lessons by recency with position, completion and a resume link', async () => {
		renderApp('/history', { catalog, recent: [progress('r2', 95, false, 2), progress('r1', 0, true, 1)] });

		expect(await screen.findByRole('heading', { name: 'Watch history' })).toBeTruthy();
		const rows = within(screen.getByRole('list', { name: 'Watched lessons' })).getAllByRole('listitem');
		expect(rows.map((row) => row.textContent)).toEqual([
			expect.stringContaining('Borrowing'),
			expect.stringContaining('Ownership')
		]);
		expect(rows[0].textContent).toContain('Stopped at 1:35');
		expect(rows[0].textContent).toContain('1/2');
		expect(rows[0].querySelector('a[href="/course/rust/m%2Fr2?t=95"]')).toBeTruthy();
		expect(rows[1].textContent).toContain('Completed');
		expect(rows[1].querySelector('a')?.getAttribute('href')).toBe('/course/rust/m%2Fr1');
		expect(screen.getAllByRole('progressbar', { name: /Course progress/ })[0].getAttribute('aria-valuenow')).toBe('1');
	});

	it('explains an empty history', async () => {
		renderApp('/history', { catalog });
		expect(await screen.findByText('No lessons watched yet')).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Browse your library' }).getAttribute('href')).toBe('/');
	});
});
