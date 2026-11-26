import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appearanceStore, updateAppearance } from '@/lib/appearance';
import { setLocale } from '@/lib/i18n';
import { transport, type Catalog, type CatalogNode, type FileEntry, type RemoteSource, type RewardSummary } from '@/lib/transport';
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
	vi.restoreAllMocks();
	cleanup();
	setLocale('en');
});

describe('My learning', () => {
	it('refreshes lesson counts when the first indexing snapshot is already finished', async () => {
		vi.spyOn(transport, 'indexingStatus').mockResolvedValue({
			status: 'done', startedAt: 1, finishedAt: 2, coursesTotal: 1, coursesCompleted: 1, lastError: null,
			progress: { filesSeen: 1, added: 1, changed: 0, removed: 0, moved: 0 }
		});
		vi.spyOn(transport, 'catalog').mockResolvedValue({ ...catalog, nodes: [course('rust', 'Rust from zero', [lesson('r1', 'Ownership')])] });
		renderApp('/', { catalog: { ...catalog, nodes: [course('rust', 'Rust from zero', [])] } });

		expect(screen.getByText('No lessons found')).toBeTruthy();
		expect(await screen.findByText('1 lesson')).toBeTruthy();
		expect(screen.queryByText('No lessons found')).toBeNull();
	});

	it('reveals a large library in batches and focuses the first newly shown course', async () => {
		const nodes = Array.from({ length: 500 }, (_, index) => {
			const id = String(index).padStart(3, '0');
			return course(id, `Course ${id}`, []);
		});
		renderApp('/', { catalog: { ...catalog, nodes } });

		const courses = await screen.findByRole('region', { name: 'Your library' });
		expect(within(courses).getAllByRole('heading', { level: 3 })).toHaveLength(60);
		fireEvent.click(within(courses).getByRole('button', { name: 'Show more courses' }));
		expect(within(courses).getAllByRole('heading', { level: 3 })).toHaveLength(120);
		expect(document.activeElement).toBe(within(courses).getByRole('link', { name: 'Course 060' }));

		const search = screen.getByRole('searchbox', { name: 'Search courses' });
		fireEvent.change(search, { target: { value: 'Course 499' } });
		expect(within(courses).getByRole('link', { name: 'Course 499' })).toBeTruthy();
		expect(within(courses).queryByRole('button', { name: 'Show more courses' })).toBeNull();
		fireEvent.change(search, { target: { value: '' } });
		expect(within(courses).getAllByRole('heading', { level: 3 })).toHaveLength(60);
	});

	it('puts the last unfinished lesson first and shows course progress', async () => {
		renderApp('/', { catalog, rewards, recent: [{ mediaId: 'r2', positionSeconds: 95, completed: false, updatedAt: 2, recorded: true }, { mediaId: 'r1', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		const resume = await screen.findByRole('region', { name: 'Continue learning' });
		expect(within(resume).getByRole('heading', { name: 'Borrowing' })).toBeTruthy();
		expect(within(resume).getByText('Lesson 2 of 2')).toBeTruthy();
		expect(within(resume).getByText('You stopped at 1:35.')).toBeTruthy();
		expect(within(resume).getByRole('link', { name: 'Resume at 1:35' }).getAttribute('href')).toBe('/course/rust/m%2Fr2?t=95');
		expect(within(resume).getByRole('link', { name: 'Open course' }).getAttribute('href')).toBe('/course/rust');
		expect(screen.getByRole('list', { name: 'Your habit' }).textContent).toMatch(/Streak.*2|2.*Streak/);
		expect(screen.getAllByRole('progressbar', { name: '1 of 2 lessons done' }).length).toBeGreaterThan(0);
	});

	it('hands Up next to the next unfinished lesson once the last one is completed, and hides it when the course is done', async () => {
		renderApp('/', { catalog, rewards, recent: [{ mediaId: 'r1', positionSeconds: 0, completed: true, updatedAt: 2, recorded: true }] });
		const resume = await screen.findByRole('region', { name: 'Continue learning' });
		expect(within(resume).getByRole('heading', { name: 'Borrowing' })).toBeTruthy();
		expect(within(resume).getByText('Lesson 2 of 2')).toBeTruthy();
		expect(within(resume).queryByText(/You stopped at/)).toBeNull();
		expect(within(resume).getByRole('link', { name: 'Start lesson' }).getAttribute('href')).toBe('/course/rust/m%2Fr2');
		cleanup();

		renderApp('/', { catalog, rewards, recent: [{ mediaId: 'r1', positionSeconds: 0, completed: true, updatedAt: 2, recorded: true }, { mediaId: 'r2', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		await screen.findByRole('region', { name: 'Your library' });
		expect(screen.queryByRole('region', { name: 'Continue learning' })).toBeNull();
	});

	it('filters by progress and offers to clear when nothing matches', async () => {
		renderApp('/', { catalog, recent: [{ mediaId: 'a1', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		const courses = await screen.findByRole('region', { name: 'Your library' });
		fireEvent.click(screen.getByRole('button', { name: /^Completed/ }));
		expect(within(courses).getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent)).toEqual(['Drawing basics']);

		cleanup();
		renderApp('/?filter=favorites', { catalog });
		expect(await screen.findByText('No favorite courses yet')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Show all courses' }));
		expect(await screen.findByRole('heading', { name: 'Rust from zero' })).toBeTruthy();

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

	it('lays out the habit strip full-width below resume in list view', async () => {
		localStorage.setItem('learnvault-library-view', 'list');
		renderApp('/', { catalog, rewards, favorites: { courseIds: [], mediaIds: ['r2'] } });
		const habit = await screen.findByLabelText('Your habit');
		expect(habit.tagName).toBe('DL');
		expect(habit.className).toContain('lg:grid-cols-5');
		expect(within(habit).getByText('Streak')).toBeTruthy();
		expect(within(habit).getByText('Level 1')).toBeTruthy();
		expect(within(habit).getByText('Goal')).toBeTruthy();
		expect(within(habit).getByText('Badges')).toBeTruthy();
		expect(within(habit).getByText('Bookmarks')).toBeTruthy();
		localStorage.removeItem('learnvault-library-view');
	});

	describe('Refresh courses', () => {
		const remote: RemoteSource = { id: 'src-1', name: 'NAS', url: 'smb://nas.example.test/cursos', username: null, hasPassword: false, credentialError: false };

		// A successful refresh invalidates the catalog and the recent study, which are fetched again.
		beforeEach(() => {
			vi.spyOn(transport, 'catalog').mockResolvedValue(catalog);
			vi.spyOn(transport, 'recentStudy').mockResolvedValue({ items: [] });
		});

		it('also reads the remote sources and says so', async () => {
			const rescan = vi.spyOn(transport, 'rescan').mockResolvedValue();
			vi.spyOn(transport, 'listSources').mockResolvedValue([remote]);
			const scan = vi.spyOn(transport, 'scanSource').mockResolvedValue({ complete: true, inserted: 2 });
			renderApp('/', { catalog });
			fireEvent.click(await screen.findByRole('button', { name: 'Refresh courses' }));

			await waitFor(() => expect(scan).toHaveBeenCalledWith('src-1'));
			expect(rescan).toHaveBeenCalledTimes(1);
			expect(await screen.findByText('Catalog updated, including 1 remote source.')).toBeTruthy();
		});

		it('names a source that could not be read instead of claiming success', async () => {
			vi.spyOn(transport, 'rescan').mockResolvedValue();
			vi.spyOn(transport, 'listSources').mockResolvedValue([remote]);
			vi.spyOn(transport, 'scanSource').mockRejectedValue(new Error('The server could not be reached.'));
			renderApp('/', { catalog });
			fireEvent.click(await screen.findByRole('button', { name: 'Refresh courses' }));
			expect(await screen.findByText('Catalog updated, but this source could not be read: “NAS”. The server could not be reached. Unavailable courses and your study progress are kept.')).toBeTruthy();
		});

		it('keeps the specific message when a rescan is already running', async () => {
			vi.spyOn(transport, 'listSources').mockResolvedValue([]);
			vi.spyOn(transport, 'rescan').mockRejectedValue(Object.assign(new Error('A rescan is already in progress.'), { name: 'RescanBusyError' }));
			renderApp('/', { catalog });
			fireEvent.click(await screen.findByRole('button', { name: 'Refresh courses' }));
			expect(await screen.findByText('A rescan is already in progress.')).toBeTruthy();
		});

		it('hides an unexpected failure behind a generic message', async () => {
			vi.spyOn(transport, 'listSources').mockResolvedValue([]);
			vi.spyOn(transport, 'rescan').mockRejectedValue(new TypeError('Failed to fetch'));
			renderApp('/', { catalog });
			fireEvent.click(await screen.findByRole('button', { name: 'Refresh courses' }));
			expect(await screen.findByText('The library action failed. Try again.')).toBeTruthy();
			expect(screen.queryByText('Failed to fetch')).toBeNull();
		});
	});
});
