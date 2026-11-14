import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { appearanceStore, updateAppearance } from '@/lib/appearance';
import { setLocale } from '@/lib/i18n';
import type { Catalog, CatalogNode, FileEntry, RewardSummary } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const media = (id: string): FileEntry => ({ name: `${id}.mp4`, path: `${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' });
const lesson = (id: string, title: string): CatalogNode => ({ title, path: `m/${id}`, role: 'lesson', files: [media(id)], looseFiles: [], children: [] });
const course = (id: string, title: string, lessons: CatalogNode[]): CatalogNode => ({ title, path: id, role: 'course', courseId: id, files: [], looseFiles: [], children: lessons });
const catalog: Catalog = {
	complete: true,
	looseFiles: [],
	nodes: [course('rust', 'Rust from zero', [lesson('r1', 'Ownership'), lesson('r2', 'Borrowing')]), course('art', 'Drawing basics', [lesson('a1', 'Lines')])]
};
const rewards: RewardSummary = {
	xp: 10, completedLessons: 1, weeklyGoal: 3, missions: [],
	study: { currentStreak: 2, longestStreak: 4, today: '2026-11-09', todayActive: true, atRisk: false, weekActiveDays: 1, completedLessons: 1, activeDaysYear: [], days: [] }
};

afterEach(() => {
	cleanup();
	setLocale('en');
});

describe('My learning', () => {
	it('puts the last unfinished lesson first and shows course progress', async () => {
		renderApp('/', { catalog, rewards, recent: [{ mediaId: 'r2', positionSeconds: 95, completed: false, updatedAt: 2, recorded: true }, { mediaId: 'r1', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		const resume = await screen.findByRole('region', { name: 'Continue learning' });
		expect(within(resume).getByRole('heading', { name: 'Borrowing' })).toBeTruthy();
		expect(within(resume).getByRole('link', { name: 'Resume' }).getAttribute('href')).toBe('/course/rust/m%2Fr2?t=95');
		expect(screen.getByRole('list', { name: 'Your habit' }).textContent).toMatch(/Streak.*2|2.*Streak/);
		expect(screen.getAllByRole('progressbar', { name: /^50% complete/ }).length).toBeGreaterThan(0);
	});

	it('filters by progress and offers to clear when nothing matches', async () => {
		renderApp('/', { catalog, recent: [{ mediaId: 'a1', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		const courses = await screen.findByRole('region', { name: 'Your library' });
		fireEvent.click(screen.getByRole('button', { name: /^Completed/ }));
		expect(within(courses).getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent)).toEqual(['Drawing basics']);

		cleanup();
		renderApp('/?q=nothing-matches', { catalog });
		expect(await screen.findByText('No results found')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Clear filters and search' }));
		expect(await screen.findByRole('heading', { name: 'Rust from zero' })).toBeTruthy();
	});

	it('shows admin actions only to admins and explains an empty library', async () => {
		renderApp('/', { user: { userId: 'u2', username: 'ana', role: 'user', isAdmin: false, mustChangeCredentials: false } });
		expect(await screen.findByText('Bring in the courses you already have')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Import course' })).toBeNull();
		cleanup();
		renderApp('/');
		expect(await screen.findByRole('button', { name: 'Import course' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Refresh courses' })).toBeTruthy();
	});

	it('shows course management on library cards for admins without opening the course', async () => {
		renderApp('/', { catalog });
		const courses = await screen.findByRole('region', { name: 'Your library' });
		expect(within(courses).getAllByRole('button', { name: /^Manage / })).toHaveLength(2);
	});

	it('keeps the course visible when an admin opens and cancels library management', async () => {
		renderApp('/', { catalog });
		const courses = await screen.findByRole('region', { name: 'Your library' });
		fireEvent.click(within(courses).getByRole('button', { name: 'Manage Rust from zero' }));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Remove course' }));
		const dialog = await screen.findByRole('alertdialog', { name: /Remove “Rust from zero” from the library/ });
		expect(within(dialog).getByText(/original files on disk are not deleted/)).toBeTruthy();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
		expect(screen.getByRole('heading', { name: 'Rust from zero' })).toBeTruthy();
	});

	it('does not expose library management to non-admins with courses', async () => {
		renderApp('/', { catalog, user: { userId: 'u2', username: 'ana', role: 'user', isAdmin: false, mustChangeCredentials: false } });
		const courses = await screen.findByRole('region', { name: 'Your library' });
		expect(within(courses).queryByRole('button', { name: /^Manage / })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Import course' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Refresh courses' })).toBeNull();
	});

	it('honors the badges visibility setting on My learning', async () => {
		const previous = appearanceStore.get();
		try {
			updateAppearance({ showBadges: true }, null);
			renderApp('/', { catalog, rewards });
			expect(await screen.findByText('Badges')).toBeTruthy();
			updateAppearance({ showBadges: false }, null);
			await waitFor(() => expect(screen.queryByText('Badges')).toBeNull());
		} finally {
			updateAppearance(previous, null);
		}
	});

	it('opens the import dialog, says files stay read-only and closes on cancel', async () => {
		renderApp('/');
		fireEvent.click(await screen.findByRole('button', { name: 'Import course' }));
		const dialog = await screen.findByRole('dialog', { name: 'Import a course' });
		expect(within(dialog).getByText(/never moved, renamed or changed/)).toBeTruthy();
		expect(within(dialog).getByLabelText('Course folder on the server')).toBeTruthy();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
		expect(screen.queryByRole('dialog')).toBeNull();
	});
});

describe('Sign in', () => {
	it('labels fields for password managers and toggles password visibility', async () => {
		renderApp('/login', { user: null });
		const username = await screen.findByLabelText('Username');
		const password = screen.getByLabelText('Password', { selector: 'input' });
		expect(username.getAttribute('autocomplete')).toBe('username');
		expect(password.getAttribute('autocomplete')).toBe('current-password');
		expect(password.getAttribute('type')).toBe('password');
		fireEvent.click(screen.getByRole('button', { name: 'Show password' }));
		expect(password.getAttribute('type')).toBe('text');
	});

	it('lists starred lessons as bookmarks, or says there are none', async () => {
		renderApp('/', { catalog, rewards, favorites: { courseIds: [], mediaIds: ['r2'] } });
		const bookmarks = await screen.findByRole('region', { name: 'Bookmarks' });
		expect(within(bookmarks).getByRole('link', { name: /Borrowing/ }).getAttribute('href')).toBe('/course/rust/m%2Fr2');
		cleanup();
		renderApp('/', { catalog, rewards });
		expect(await screen.findByText(/Nothing saved yet/)).toBeTruthy();
	});
});
