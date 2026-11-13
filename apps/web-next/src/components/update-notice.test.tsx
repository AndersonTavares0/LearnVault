import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DesktopUpdatePrompt } from '@/components/update-notice';
import { transport, type UpdateStatus } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const available: UpdateStatus = { current: '0.8.0', enabled: true, available: true, latest: '0.9.0', releasedAt: null, url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.9.0', changelog: 'Smaller downloads and a calmer player.', checkedAt: null, error: null };

const desktopAvailable: UpdateStatus = { current: '0.8.0', enabled: true, available: true, latest: '0.9.0', releasedAt: null, url: null, changelog: 'Smaller downloads and a calmer player.', checkedAt: null, error: null };

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

describe('desktop restart prompt', () => {
	it('offers the download, then the restart, and exposes the release notes', async () => {
		const downloadUpdate = vi.fn().mockResolvedValue(true);
		const restartToUpdate = vi.fn().mockResolvedValue(true);
		render(<DesktopUpdatePrompt status={desktopAvailable} client={{ downloadUpdate, restartToUpdate }} />);

		expect(screen.getByRole('status').textContent).toContain('Version 0.9.0 is available');
		expect(screen.getByText('Smaller downloads and a calmer player.')).toBeTruthy();

		fireEvent.click(screen.getByRole('button', { name: 'Download update' }));
		expect(downloadUpdate).toHaveBeenCalledTimes(1);
		fireEvent.click(await screen.findByRole('button', { name: 'Restart to update' }));
		expect(restartToUpdate).toHaveBeenCalledTimes(1);
	});

	it('shows the download in progress without blocking anything', async () => {
		const downloadUpdate = vi.fn().mockReturnValue(new Promise<boolean>(() => {}));
		render(<DesktopUpdatePrompt status={desktopAvailable} client={{ downloadUpdate, restartToUpdate: vi.fn() }} />);

		fireEvent.click(screen.getByRole('button', { name: 'Download update' }));
		expect(await screen.findByRole('button', { name: 'Downloading…' })).toHaveProperty('disabled', true);
		expect(screen.getByRole('status').textContent).toContain('Downloading version 0.9.0');
	});

	it('reports a failure in place and lets the person try again', async () => {
		const downloadUpdate = vi.fn().mockRejectedValueOnce(new Error('offline')).mockResolvedValueOnce(true);
		render(<DesktopUpdatePrompt status={desktopAvailable} client={{ downloadUpdate, restartToUpdate: vi.fn() }} />);

		fireEvent.click(screen.getByRole('button', { name: 'Download update' }));
		expect(await screen.findByRole('alert')).toBeTruthy();
		expect(screen.getByRole('alert').textContent).toContain('Could not prepare the update. Try again.');

		fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
		expect(await screen.findByRole('button', { name: 'Restart to update' })).toBeTruthy();
		expect(downloadUpdate).toHaveBeenCalledTimes(2);
	});
});
