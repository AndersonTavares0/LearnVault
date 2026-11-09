import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { transport, type Catalog, type CatalogNode, type RewardSummary } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const lesson = (id: string): CatalogNode => ({ title: id, path: `c/${id}`, role: 'lesson', files: [{ name: `${id}.mp4`, path: `c/${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' }], looseFiles: [], children: [] });
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{ title: 'Rust', path: 'c', role: 'course', courseId: 'rust', files: [], looseFiles: [], children: [lesson('a'), lesson('b')] }] };
const days = ['2026-11-09', '2026-11-10', '2026-11-11', '2026-11-12', '2026-11-13', '2026-11-14', '2026-11-15'].map((day, index) => ({ day, active: index === 0 }));
const rewards: RewardSummary = {
	xp: 150, completedLessons: 2, weeklyGoal: 3,
	missions: [{ id: 'lesson', progress: 1, target: 1, claimed: false }, { id: 'focus-two', progress: 0, target: 2, claimed: false }],
	study: { currentStreak: 1, longestStreak: 5, today: '2026-11-09', todayActive: true, atRisk: false, weekActiveDays: 1, completedLessons: 2, days }
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
	it('shows the habit, a finished course with its badge and the focus summary', async () => {
		renderApp('/progress', { catalog, rewards, recent: [done('a'), done('b')] });
		expect(await screen.findByText('1 day')).toBeTruthy();
		expect(screen.getByText('1 of 3 days')).toBeTruthy();
		expect(screen.getByRole('list', { name: 'This week' }).querySelectorAll('li')).toHaveLength(7);
		expect(screen.getByText('Finished')).toBeTruthy();
		expect(screen.getByText('First course finished').parentElement?.textContent).toContain('1/1');
		expect(await screen.findByText('25 min today · 50 min in 2 sessions this week')).toBeTruthy();
		expect(screen.getByRole('heading', { name: 'Level 2 · Archive Explorer' })).toBeTruthy();
	});

	it('claims a finished mission and changes the weekly goal', async () => {
		const claim = vi.spyOn(transport, 'claimDailyMission').mockResolvedValue(true);
		const goal = vi.spyOn(transport, 'setWeeklyGoal').mockResolvedValue(5);
		renderApp('/progress', { catalog, rewards });
		const buttons = await screen.findAllByRole('button', { name: 'Claim 30 XP' });
		expect((buttons[1] as HTMLButtonElement).disabled).toBe(true);
		fireEvent.click(buttons[0]);
		await waitFor(() => expect(claim).toHaveBeenCalledWith('lesson'));
		fireEvent.change(screen.getByLabelText('Days per week'), { target: { value: '5' } });
		await waitFor(() => expect(goal).toHaveBeenCalledWith(5));
	});
});
