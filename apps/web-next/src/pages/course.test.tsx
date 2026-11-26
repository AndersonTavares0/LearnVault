import { cleanup, fireEvent, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Catalog, CatalogNode, FileEntry } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const file = (name: string, kind: string, availability = 'available'): FileEntry => ({ name, path: `rust/${name}`, size: 1, mediaId: kind === 'media' ? name : undefined, kind, availability });
const lesson = (title: string, files: FileEntry[]): CatalogNode => ({ title, path: `rust/${title}`, role: 'lesson', files, looseFiles: [], children: [] });
const module = (title: string, children: CatalogNode[]): CatalogNode => ({ title, path: `rust/${title}`, role: 'module', files: [], looseFiles: [], children });
const catalog: Catalog = {
	complete: true,
	looseFiles: [],
	nodes: [{
		title: 'Rust from zero', path: 'rust', role: 'course', courseId: 'rust', files: [], looseFiles: [],
		children: [
			module('Basics', [lesson('Ownership', [file('own.mp4', 'media'), file('slides.pdf', 'material')]), lesson('Borrowing', [file('borrow.mp4', 'media')])]),
			module('Advanced', [lesson('Lifetimes', [file('life.mp4', 'media', 'missing')])])
		]
	}]
};

afterEach(cleanup);

describe('course page', () => {
	it('shows progress, resumes the last lesson and lists modules with per-lesson state', async () => {
		renderApp('/course/rust', { catalog, recent: [{ mediaId: 'borrow.mp4', positionSeconds: 70, completed: false, updatedAt: 2, recorded: true }, { mediaId: 'own.mp4', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		expect(await screen.findByRole('heading', { level: 1, name: 'Rust from zero' })).toBeTruthy();
		expect(screen.getByRole('progressbar', { name: '1 of 2 lessons done' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Resume Borrowing at 1:10' }).getAttribute('href')).toBe('/course/rust/rust%2FBorrowing?t=70');
		expect(screen.getByText('Resume at 1:10')).toBeTruthy();
		expect(screen.getByText('Done')).toBeTruthy();
		expect(screen.getByText('1 of 2 done')).toBeTruthy();
		expect(screen.getByRole('group', { name: 'Course at a glance' }).textContent).toContain('2 lessons');
		expect(screen.getByRole('button', { name: 'Mark “Ownership” as not completed' }).getAttribute('aria-pressed')).toBe('true');
		expect(screen.getByRole('link', { name: /^Borrowing/ }).hasAttribute('aria-current')).toBe(false);
		expect(screen.getByText('Missing media')).toBeTruthy();
	});

	it('returns to the library with the filter and search the learner left', async () => {
		localStorage.setItem('learnvault-library-return', 'filter=in_progress&q=rust');
		renderApp('/course/rust', { catalog });
		expect((await within(await screen.findByRole('main')).findByRole('link', { name: 'My learning' })).getAttribute('href')).toBe('/?filter=in_progress&q=rust');
		localStorage.removeItem('learnvault-library-return');
	});

	it('lists course materials with a download link', async () => {
		renderApp('/course/rust', { catalog });
		fireEvent.click(await screen.findByRole('tab', { name: /Materials/ }));
		const panel = screen.getByRole('tabpanel');
		expect(within(panel).getByText('slides.pdf')).toBeTruthy();
		expect(within(panel).getByRole('link', { name: 'Download slides.pdf' })).toBeTruthy();
	});

	it('links the PDFs that no lesson holds, including inside modules', async () => {
		const pdf = (path: string): FileEntry => ({ name: path.split('/').pop()!, path, size: 1, mediaId: `id-${path}`, kind: 'material', availability: 'available' });
		const reading: Catalog = { complete: true, looseFiles: [], nodes: [{
			title: 'Reading', path: 'reading', role: 'course', courseId: 'reading', files: [], looseFiles: [pdf('reading/Ownership notes.pdf')],
			children: [{ title: 'Guides', path: 'reading/Guides', role: 'category', files: [], looseFiles: [pdf('reading/Guides/Borrowing.pdf')], children: [] }]
		}] };
		renderApp('/course/reading', { catalog: reading });
		expect((await screen.findByRole('link', { name: 'Ownership notes.pdf' })).getAttribute('href')).toBe('/course/reading/reading%2FOwnership%20notes.pdf');
		expect(screen.getByRole('link', { name: 'Borrowing.pdf' }).getAttribute('href')).toBe('/course/reading/reading%2FGuides%2FBorrowing.pdf');
		// A course made only of documents reads as a list, not as a broken course.
		expect(screen.getByRole('heading', { name: /Reading list/ })).toBeTruthy();
		expect(screen.queryByText('The structure of this course was hard to read.')).toBeNull();
		expect(screen.queryByText('No lessons available')).toBeNull();
		expect(screen.getByRole('link', { name: 'Start reading' }).getAttribute('href')).toBe('/course/reading/reading%2FOwnership%20notes.pdf');
	});

	it('offers course management to admins only', async () => {
		renderApp('/course/rust', { catalog, user: { userId: 'u2', username: 'ana', role: 'user', isAdmin: false, mustChangeCredentials: false } });
		await screen.findByRole('heading', { level: 1, name: 'Rust from zero' });
		expect(screen.queryByRole('button', { name: 'Manage course' })).toBeNull();
		cleanup();
		renderApp('/course/rust', { catalog });
		expect(await screen.findByRole('button', { name: 'Manage course' })).toBeTruthy();
	});

	it('does not offer local-only management for a course that lives on a remote source', async () => {
		const remote: Catalog = { complete: true, looseFiles: [], nodes: [{ ...catalog.nodes[0], courseId: 'remote-1', title: 'Redes', remote: true }] };
		renderApp('/course/remote-1', { catalog: remote });
		await screen.findByRole('heading', { level: 1, name: 'Redes' });
		expect(screen.queryByRole('button', { name: 'Manage course' })).toBeNull();
	});

	it('explains a course that is gone', async () => {
		renderApp('/course/unknown', { catalog });
		expect(await screen.findByRole('heading', { name: 'Course not found' })).toBeTruthy();
	});

	it.each([true, false])('shows detailed cover art, but keeps the monogram for a flat image (detail: %s)', async (detail) => {
		const withCover: Catalog = { complete: true, looseFiles: [], nodes: [{ ...catalog.nodes[0], coverVersion: 3 }] };
		renderApp('/course/rust', { catalog: withCover });
		const heading = await screen.findByRole('heading', { level: 1, name: 'Rust from zero' });
		const hero = heading.closest('[data-course-hero]')!;
		const covers = hero.querySelectorAll<HTMLImageElement>('img[src*="/cover"]');
		expect(covers).toHaveLength(1);
		expect(within(hero as HTMLElement).getByText('RF')).toBeTruthy();
		const data = new Uint8ClampedArray(8 * 8 * 4);
		if (detail) data[4] = 255;
		const read = vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
			drawImage: vi.fn(),
			getImageData: () => ({ data })
		} as unknown as CanvasRenderingContext2D);
		try {
			fireEvent.load(covers[0]);
			expect(covers[0].hasAttribute('data-ready')).toBe(detail);
		} finally {
			read.mockRestore();
		}
	});
});
