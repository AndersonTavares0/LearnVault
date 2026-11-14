import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { transport, type Catalog, type CatalogNode, type RewardSummary } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const lesson = (id: string): CatalogNode => ({ title: id, path: `c/${id}`, role: 'lesson', files: [{ name: `${id}.mp4`, path: `c/${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' }], looseFiles: [], children: [] });
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{ title: 'Rust', path: 'c', role: 'course', courseId: 'rust', files: [], looseFiles: [], children: [lesson('a'), lesson('b')] }] };
const days = ['2026-11-09', '2026-11-10', '2026-11-11', '2026-11-12', '2026-11-13', '2026-11-14', '2026-11-15'].map((day, index) => ({ day, active: index === 0 }));
const rewards: RewardSummary = {
	xp: 150, completedLessons: 2, weeklyGoal: 3,
	missions: [{ id: 'lesson', progress: 1, target: 1, claimed: false }, { id: 'focus-two', progress: 0, target: 2, claimed: false }],
	study: { currentStreak: 1, longestStreak: 5, today: '2026-11-09', todayActive: true, atRisk: false, weekActiveDays: 1, completedLessons: 2, activeDaysYear: [], days }
};
const done = (mediaId: string) => ({ mediaId, completed: true, positionSeconds: 0, updatedAt: 1_794_200_000, recorded: true });

beforeEach(() => {
	vi.spyOn(transport, 'focusSummary').mockResolvedValue({ todayMinutes: 25, weekMinutes: 50, weekSessions: 2, totalMinutes: 50, totalSessions: 2, completionRate: 1, bestDay: null, days: [] });
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('progress', () => {
	it('shows the week, a finished course with its badge and the level', async () => {
		renderApp('/progress', { catalog, rewards, recent: [done('a'), done('b')] });
		expect(await screen.findByRole('heading', { name: '1 of 3 days this week' })).toBeTruthy();
		expect(screen.getByText(/2 more days to reach your goal\./)).toBeTruthy();
		expect(within(await screen.findByRole('table', { name: 'Focus minutes by day', hidden: true })).getAllByRole('row')).toHaveLength(8);
		expect(screen.getByText('First course finished').parentElement?.textContent).toContain('Earned');
		expect(screen.getByText(/Level 2 · /)).toBeTruthy();
	});

	it('offers to claim only a finished mission', async () => {
		const claim = vi.spyOn(transport, 'claimDailyMission').mockResolvedValue(true);
		renderApp('/progress', { catalog, rewards });
		const button = await screen.findByRole('button', { name: 'Claim 30 XP' });
		expect(screen.getAllByRole('button', { name: /^Claim/ })).toHaveLength(1);
		fireEvent.click(button);
		await waitFor(() => expect(claim).toHaveBeenCalledWith('lesson'));
	});

	it('labels the all-time total separately from its weekly breakdown', async () => {
		vi.spyOn(transport, 'focusSummary').mockResolvedValue({ todayMinutes: 25, weekMinutes: 50, weekSessions: 2, totalMinutes: 200, totalSessions: 8, completionRate: 1, bestDay: null, days: [] });
		renderApp('/progress', { catalog, rewards });
		fireEvent.click(await screen.findByRole('button', { name: 'All time' }));
		expect(screen.getByText('3h 20m', { exact: true })).toBeTruthy();
		expect(screen.getByText('Weekly breakdown')).toBeTruthy();
	});

	it('reports a focus summary failure instead of showing zero activity', async () => {
		vi.spyOn(transport, 'focusSummary').mockRejectedValue(new Error('focus unavailable'));
		renderApp('/progress', { catalog, rewards });
		expect((await screen.findByRole('alert')).textContent).toContain('Focus statistics could not be loaded.');
		expect(screen.queryByText('No focus time yet. Start the timer in a lesson and it will show up here.')).toBeNull();
	});
});
