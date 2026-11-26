import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { transport, type Catalog, type CatalogNode, type FileEntry, type StudyNote } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const media = (id: string): FileEntry => ({ name: `${id}.mp4`, path: `${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' });
const lesson = (id: string, title: string): CatalogNode => ({ title, path: `m/${id}`, role: 'lesson', files: [media(id)], looseFiles: [], children: [] });
const course = (id: string, title: string, lessons: CatalogNode[]): CatalogNode => ({ title, path: id, role: 'course', courseId: id, files: [], looseFiles: [], children: lessons });
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [course('rust', 'Rust from zero', [lesson('r1', 'Ownership'), lesson('r2', 'Borrowing')])] };

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

// The shell is one column: the bar first, then the region that scrolls (issue #607). While the
// document was the scroller, the bar scrolled away and the window scrollbar crossed it.
describe('app shell', () => {
	it('moves focus from the navigation link to the new page heading and on Back', async () => {
		const { router } = renderApp('/');
		const nav = screen.getByRole('navigation', { name: 'Main' });
		const link = within(nav).getByRole('link', { name: 'Progress' });
		expect(document.activeElement).toBe(document.body);
		link.focus();
		fireEvent.click(link);
		const heading = await screen.findByRole('heading', { level: 1, name: 'Your progress' });
		expect(document.activeElement).toBe(heading);
		expect(heading.tabIndex).toBe(-1);

		await act(() => router.navigate(-1));
		expect(document.activeElement).toBe(screen.getByRole('heading', { level: 1, name: 'My learning' }));
	});

	it('keeps the current focus on search and hash changes within a page', async () => {
		const { router } = renderApp('/settings');
		const search = screen.getByRole('combobox', { name: 'Search notes, courses and lessons' });
		search.focus();
		await act(() => router.navigate('/settings#focus'));
		expect(document.activeElement).toBe(search);
		await act(() => router.navigate('/settings?q=study#focus'));
		expect(document.activeElement).toBe(search);
	});

	it.each([false, true])('handles a delayed heading without stealing focus (user moved: %s)', async (userMoved) => {
		let resolveNotes!: (notes: StudyNote[]) => void;
		vi.spyOn(transport, 'listNotes').mockReturnValue(new Promise((resolve) => { resolveNotes = resolve; }));
		const { router } = renderApp('/');
		await act(() => router.navigate('/notes'));
		expect(screen.getByText('Loading notes…')).toBeTruthy();
		expect(document.activeElement).toBe(screen.getByRole('main'));

		const search = screen.getByRole('combobox', { name: 'Search notes, courses and lessons' });
		if (userMoved) search.focus();
		resolveNotes([]);
		const heading = await screen.findByRole('heading', { level: 1, name: 'Notes' });
		await waitFor(() => expect(document.activeElement).toBe(userMoved ? search : heading));
	});

	it('scrolls the content region instead of the document', () => {
		renderApp('/');
		const bar = screen.getByRole('banner');
		const main = screen.getByRole('main');
		const shell = bar.closest('.h-dvh')!;

		expect(shell.className).toContain('h-dvh');
		expect(shell.className).toContain('flex-col');
		expect(bar.className).toContain('shrink-0');
		expect(main.className).toContain('app-scroll');
		expect(main.className).toContain('overflow-y-auto');
		expect(main.className).toContain('min-h-0');

		// The bar and the content are siblings, never nested, so the bar cannot scroll away.
		expect(bar.contains(main)).toBe(false);
		expect(shell.contains(main)).toBe(true);
		expect(bar.compareDocumentPosition(main) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

		// The phone tab bar floats over the bottom edge, so the scroller keeps both padding and
		// scroll-padding clearance (including safe-area-inset-bottom) so focused controls stay visible.
		expect(main.className).toContain('scroll-pb-[calc(4.5rem+env(safe-area-inset-bottom))]');
		expect(main.firstElementChild?.className).toContain('pb-28');
	});

	it('turns the top bar into a left rail from lg with the destinations, search and account controls', () => {
		renderApp('/');
		const bar = screen.getByRole('banner');
		expect(bar.className).toContain('lg:w-[236px]');
		expect(bar.className).toContain('lg:bg-sidebar');
		const rail = within(bar);
		const nav = rail.getByRole('navigation', { name: 'Main' });
		expect(within(nav).getByRole('link', { name: 'My learning' }).className).toContain('bg-foreground/[0.08] font-semibold text-foreground');
		expect(within(nav).getByRole('link', { name: 'Progress' }).className).not.toContain('font-semibold');
		expect(rail.getByRole('combobox', { name: 'Search notes, courses and lessons' })).toBeTruthy();
		expect(rail.getByRole('button', { name: 'Account: admin' })).toBeTruthy();
		expect(rail.getByRole('link', { name: 'LearnVault home' })).toBeTruthy();
	});

	it('shows the platform search shortcut in the rail', () => {
		renderApp('/');
		expect(screen.getByText('Ctrl K', { selector: 'kbd' })).toBeTruthy();
		cleanup();
		vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)');
		renderApp('/');
		expect(screen.getByText('⌘K', { selector: 'kbd' })).toBeTruthy();
	});

	it('lists courses in progress in the rail and hides the group when none is started', () => {
		renderApp('/', { catalog, recent: [{ mediaId: 'r1', positionSeconds: 95, completed: false, updatedAt: 2, recorded: true }] });
		const rail = within(screen.getByRole('banner'));
		expect(rail.getByRole('heading', { level: 2, name: 'In progress' })).toBeTruthy();
		expect(rail.getByRole('link', { name: 'Rust from zero' }).getAttribute('href')).toContain('rust');
	});

	it('hides the in-progress group when no course has been started', () => {
		renderApp('/', { catalog, recent: [] });
		expect(screen.queryByRole('heading', { name: 'In progress' })).toBeNull();
	});

	it('locks the document only while the shell is on screen', () => {
		expect(document.documentElement.dataset.appShell).toBeUndefined();
		renderApp('/');
		expect(document.documentElement.dataset.appShell).toBeDefined();
		cleanup();
		// The sign-in page and the route error page render without the shell: their document must scroll.
		expect(document.documentElement.dataset.appShell).toBeUndefined();
	});

	it('scopes the scroll lock in the stylesheet to the shell', () => {
		const css = readFileSync(resolve(dirname(fileURLToPath(import.meta.url)), '../styles/app.css'), 'utf8');
		expect(css).toMatch(/html\[data-app-shell\], html\[data-app-shell\] body \{ height: 100%; overflow: hidden; \}/);
		// No rule may lock every page: that made the sign-in button unreachable on a short viewport.
		expect(css).not.toMatch(/^html, body \{[^}]*overflow: hidden/m);
	});
});
