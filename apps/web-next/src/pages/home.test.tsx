import { cleanup, fireEvent, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
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
	study: { currentStreak: 2, longestStreak: 4, today: '2026-11-09', todayActive: true, atRisk: false, weekActiveDays: 1, completedLessons: 1, days: [] }
};

afterEach(() => {
	cleanup();
	setLocale('en');
});

describe('My learning', () => {
	it('puts the last unfinished lesson first and shows course progress', async () => {
		renderApp('/', { catalog, rewards, recent: [{ mediaId: 'r2', positionSeconds: 95, completed: false, updatedAt: 2, recorded: true }, { mediaId: 'r1', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		const resume = await screen.findByRole('region', { name: 'Continue studying' });
		expect(within(resume).getByRole('heading', { name: 'Borrowing' })).toBeTruthy();
		expect(within(resume).getByRole('link', { name: 'Resume lesson' }).getAttribute('href')).toBe('/course/rust/m%2Fr2?t=95');
		expect(screen.getByText(/2 day streak, 1 of 3 days this week/)).toBeTruthy();
		expect(screen.getByRole('progressbar', { name: /^50% complete/ })).toBeTruthy();
	});

	it('filters by progress and offers to clear when nothing matches', async () => {
		renderApp('/', { catalog, recent: [{ mediaId: 'a1', positionSeconds: 0, completed: true, updatedAt: 1, recorded: true }] });
		const courses = await screen.findByRole('region', { name: 'All courses' });
		fireEvent.click(within(courses).getByRole('button', { name: /^Completed/ }));
		expect(within(courses).getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent)).toEqual(['Drawing basics']);

		cleanup();
		renderApp('/?q=nothing-matches', { catalog });
		expect(await screen.findByText('No results found')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Clear filters and search' }));
		expect(await screen.findByRole('heading', { name: 'Rust from zero' })).toBeTruthy();
	});

	it('shows admin actions only to admins and explains an empty library', async () => {
		renderApp('/', { user: { userId: 'u2', username: 'ana', role: 'user', isAdmin: false, mustChangeCredentials: false } });
		expect(await screen.findByText('No courses yet')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Import course' })).toBeNull();
		cleanup();
		renderApp('/');
		expect(await screen.findByRole('button', { name: 'Import course' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Refresh courses' })).toBeTruthy();
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
});
