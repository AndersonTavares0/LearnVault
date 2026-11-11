import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { pomodoro } from '@/lib/pomodoro-store';
import { transport, type Catalog, type LocalUser } from '@/lib/transport';
import { admin, renderApp } from '@/test/render-app';

const student = { userId: 'u2', username: 'ana', role: 'user', isAdmin: false, mustChangeCredentials: false };
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [
	{ title: 'Rust', path: 'r', role: 'course', courseId: 'rust', files: [], looseFiles: [], children: [] },
	{ title: 'Go', path: 'g', role: 'course', courseId: 'go', files: [], looseFiles: [], children: [] }
] };
const users: LocalUser[] = [{ userId: 'u1', username: 'admin', isAdmin: true, createdAt: 1, allowedCourseIds: null }];

beforeEach(() => {
	vi.spyOn(transport, 'checkUpdate').mockResolvedValue({ current: '0.9.0', enabled: true, available: false, latest: '0.9.0', releasedAt: null, url: null, changelog: null, checkedAt: null, error: null });
	vi.spyOn(transport, 'updateHistory').mockResolvedValue([]);
	vi.spyOn(transport, 'listUsers').mockResolvedValue(users);
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('settings', () => {
	it('customizes the look at once and can put the default back', async () => {
		renderApp('/settings', { catalog });
		const accent = await screen.findByRole('group', { name: 'Accent color' });
		fireEvent.click(within(accent).getByRole('button', { name: 'Plum' }));
		expect(document.documentElement.style.getPropertyValue('--brand')).toBe('#7A3B7B');
		fireEvent.click(within(screen.getByRole('group', { name: 'Corners' })).getByRole('button', { name: 'Round' }));
		expect(document.documentElement.dataset.radius).toBe('round');
		fireEvent.click(within(screen.getByRole('group', { name: 'Course content in a lesson' })).getByRole('button', { name: 'On the left' }));
		expect(document.documentElement.dataset.sidebar).toBe('left');
		fireEvent.click(screen.getByRole('checkbox', { name: 'Streak' }));
		expect(JSON.parse(localStorage.getItem('learnvault-appearance')!)).toMatchObject({ accent: '#7A3B7B', radius: 'round', sidebar: 'left', showStreak: false });
		fireEvent.click(screen.getByRole('button', { name: 'Reset to the default look' }));
		expect(document.documentElement.style.getPropertyValue('--brand')).toBe('#5C4229');
		expect(document.documentElement.dataset.radius).toBe('soft');
	});

	it('changes the weekly goal', async () => {
		const goal = vi.spyOn(transport, 'setWeeklyGoal').mockResolvedValue(5);
		renderApp('/settings#study', { catalog, rewards: { xp: 0, completedLessons: 0, weeklyGoal: 3, missions: [], study: { currentStreak: 0, longestStreak: 0, today: '2026-11-09', todayActive: false, atRisk: false, weekActiveDays: 0, completedLessons: 0, days: [] } } });
		const group = await screen.findByRole('group', { name: 'Weekly goal, in study days' });
		expect(within(group).getByRole('button', { name: '3' }).getAttribute('aria-pressed')).toBe('true');
		fireEvent.click(within(group).getByRole('button', { name: '5' }));
		await waitFor(() => expect(goal).toHaveBeenCalledWith(5));
	});


	it('shows admin sections only to administrators', async () => {
		renderApp('/settings#profile', { catalog, user: student });
		expect(await screen.findByRole('heading', { name: 'Profile' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Backup' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Updates' })).toBeNull();
		expect(screen.queryByRole('heading', { name: 'Accounts and course access' })).toBeNull();
		expect(screen.queryByRole('heading', { name: 'Backup' })).toBeNull();
		expect(screen.queryByRole('heading', { name: 'Updates' })).toBeNull();
		cleanup();
		renderApp('/settings#users', { catalog, user: admin });
		expect(await screen.findByRole('heading', { name: 'Accounts and course access' })).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Backup' }));
		expect(screen.getByRole('link', { name: /Download backup/ }).getAttribute('href')).toBe('/api/admin/backup');
		fireEvent.click(screen.getByRole('button', { name: 'Updates' }));
		expect(await screen.findByText('You are using the latest version.')).toBeTruthy();
	});

	it('rejects a short new password before sending it', async () => {
		const update = vi.spyOn(transport, 'updateCredentials');
		renderApp('/settings#profile', { catalog, user: student });
		fireEvent.change(await screen.findByLabelText('New password', { selector: 'input' }), { target: { value: 'short' } });
		expect(screen.getByText(/Use at least 12 characters/).className).toContain('text-destructive');
		expect((screen.getByRole('button', { name: 'Update credentials' }) as HTMLButtonElement).disabled).toBe(true);
		expect(update).not.toHaveBeenCalled();
	});

	it('creates accounts with no course access by default', async () => {
		const create = vi.spyOn(transport, 'createUser').mockResolvedValue({ userId: 'u3', username: 'bia', isAdmin: false, createdAt: 2, allowedCourseIds: [] });
		renderApp('/settings#users', { catalog, user: admin });
		const form = (await screen.findByRole('heading', { name: 'New account' })).closest('form')!;
		expect((within(form).getByRole('checkbox', { name: /All courses/ }) as HTMLInputElement).checked).toBe(false);
		fireEvent.change(within(form).getByLabelText('Username'), { target: { value: 'bia' } });
		fireEvent.change(within(form).getByLabelText('Password (12+ characters)', { selector: 'input' }), { target: { value: 'a long passphrase' } });
		fireEvent.click(within(form).getByRole('checkbox', { name: 'Go' }));
		fireEvent.click(within(form).getByRole('button', { name: 'Create account' }));
		await waitFor(() => expect(create).toHaveBeenCalledWith('bia', 'a long passphrase', ['go']));
	});

	it('saves Pomodoro durations', async () => {
		renderApp('/settings#focus', { catalog, user: student });
		fireEvent.change(await screen.findByLabelText('Focus (minutes)'), { target: { value: '45' } });
		fireEvent.click(screen.getByRole('button', { name: 'Save timer settings' }));
		expect(pomodoro.get().settings.focusMinutes).toBe(45);
	});
});

describe('course covers', () => {
	it('offers cover actions in the course admin menu', async () => {
		renderApp('/course/rust', { catalog: { ...catalog, nodes: [{ ...catalog.nodes[0], coverVersion: 3 }] } });
		fireEvent.click(await screen.findByRole('button', { name: 'Manage course' }));
		expect(await screen.findByRole('menuitem', { name: 'Change cover' })).toBeTruthy();
		expect(screen.getByRole('menuitem', { name: 'Remove cover' })).toBeTruthy();
	});
});
