import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { transport, type UpdateStatus } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const available: UpdateStatus = { current: '0.8.0', enabled: true, available: true, latest: '0.9.0', releasedAt: null, url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.9.0', changelog: null, checkedAt: null, error: null };

beforeEach(() => { localStorage.clear(); });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });

describe('update notice', () => {
	it('tells the administrator about a newer version, links to what is new, and stays quiet once dismissed', async () => {
		vi.spyOn(transport, 'checkUpdate').mockResolvedValue(available);
		renderApp('/', { user: { userId: 'u', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: false } });
		const notice = await screen.findByRole('region', { name: 'Update available' });
		expect(notice.textContent).toContain('Version 0.9.0 is available');
		expect(screen.getByRole('link', { name: 'What’s new' }).getAttribute('href')).toBe(available.url);
		expect(screen.getByRole('link', { name: 'How to update' }).getAttribute('href')).toBe('/settings#updates');
		fireEvent.click(screen.getByRole('button', { name: 'Dismiss until the next version' }));
		expect(screen.queryByRole('region', { name: 'Update available' })).toBeNull();
		expect(localStorage.getItem('learnvault-update-dismissed')).toBe('0.9.0');
	});

	it('is not shown to people who are not administrators, or when up to date', async () => {
		const check = vi.spyOn(transport, 'checkUpdate').mockResolvedValue(available);
		renderApp('/', { user: { userId: 'u', username: 'sam', role: 'member', isAdmin: false, mustChangeCredentials: false } });
		await screen.findByRole('heading', { level: 1 });
		expect(check).not.toHaveBeenCalled();
		expect(screen.queryByRole('region', { name: 'Update available' })).toBeNull();
		cleanup();
		vi.spyOn(transport, 'checkUpdate').mockResolvedValue({ ...available, available: false, latest: '0.8.0' });
		renderApp('/', { user: { userId: 'u', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: false } });
		await screen.findByRole('heading', { level: 1 });
		await waitFor(() => expect(transport.checkUpdate).toHaveBeenCalled());
		expect(screen.queryByRole('region', { name: 'Update available' })).toBeNull();
	});
});
