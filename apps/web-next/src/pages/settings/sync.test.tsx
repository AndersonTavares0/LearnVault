import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { transport, type SyncResult, type SyncState } from '@/lib/transport';
import { SyncSettings } from './sync';

const state = (patch: Partial<SyncState> = {}): SyncState => ({ deviceId: 'device-1', pullCursor: 0, serverUrl: '', signedIn: false, pending: 0, lastError: null, lastResult: null, ...patch });
const SYNC_METHODS = ['syncState', 'syncConfigure', 'syncSignIn', 'syncSignOut', 'syncNow'] as const;

function renderSync() {
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	render(<QueryClientProvider client={client}><SyncSettings /></QueryClientProvider>);
}

beforeEach(() => {
	transport.syncState = vi.fn().mockResolvedValue(state());
	transport.syncConfigure = vi.fn().mockResolvedValue(state({ serverUrl: 'https://lv.example' }));
	transport.syncSignIn = vi.fn().mockResolvedValue(state({ serverUrl: 'https://lv.example', signedIn: true }));
	transport.syncSignOut = vi.fn().mockResolvedValue(state({ serverUrl: 'https://lv.example' }));
	transport.syncNow = vi.fn().mockResolvedValue({ pushed: 0, pulled: 0, pending: 0, error: null });
});
afterEach(() => {
	cleanup();
	for (const method of SYNC_METHODS) delete transport[method];
});

describe('sync settings', () => {
	it('refuses an address the Rust rule rejects and then opens the sign-in form', async () => {
		renderSync();
		const address = await screen.findByLabelText('Server address');
		for (const bad of ['lv.example', 'https://lv.example/course', 'https://user@lv.example', 'https://lv.example?x=1']) {
			fireEvent.change(address, { target: { value: bad } });
			expect(screen.getByRole('alert').textContent).toContain('Use http:// or https://');
			expect((screen.getByRole('button', { name: 'Save address' }) as HTMLButtonElement).disabled).toBe(true);
		}
		expect(transport.syncConfigure).not.toHaveBeenCalled();

		fireEvent.change(address, { target: { value: 'https://lv.example' } });
		fireEvent.click(screen.getByRole('button', { name: 'Save address' }));
		await waitFor(() => expect(transport.syncConfigure).toHaveBeenCalledWith('https://lv.example'));
		expect(await screen.findByRole('heading', { name: 'Sign in to sync' })).toBeTruthy();
	});

	it('signs in, never renders the password and shows the queue', async () => {
		transport.syncState = vi.fn().mockResolvedValue(state({ serverUrl: 'https://lv.example' }));
		transport.syncSignIn = vi.fn().mockResolvedValue(state({ serverUrl: 'https://lv.example', signedIn: true, pending: 2 }));
		renderSync();

		fireEvent.change(await screen.findByLabelText('Username'), { target: { value: 'ana' } });
		fireEvent.change(screen.getByLabelText('Password', { selector: 'input' }), { target: { value: 'segredo-secreto' } });
		fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

		await waitFor(() => expect(transport.syncSignIn).toHaveBeenCalledWith('ana', 'segredo-secreto'));
		expect(await screen.findByText('Signed in to https://lv.example.')).toBeTruthy();
		expect(screen.getByText('2 change(s) waiting to upload.')).toBeTruthy();
		expect(screen.getByText(/Signing out keeps the queue on this device/)).toBeTruthy();
		expect(screen.queryByLabelText('Password', { selector: 'input' })).toBeNull();
		expect(document.body.innerHTML).not.toContain('segredo-secreto');
	});

	it('clears the password after a failed sign-in and keeps the error visible', async () => {
		transport.syncState = vi.fn().mockResolvedValue(state({ serverUrl: 'https://lv.example' }));
		transport.syncSignIn = vi.fn().mockRejectedValue(new Error('Usuário ou senha incorretos.'));
		renderSync();

		fireEvent.change(await screen.findByLabelText('Username'), { target: { value: 'ana' } });
		fireEvent.change(screen.getByLabelText('Password', { selector: 'input' }), { target: { value: 'senha-que-nao-fica' } });
		fireEvent.click(screen.getByRole('button', { name: 'Sign in' }));

		await waitFor(() => expect((screen.getByLabelText('Password', { selector: 'input' }) as HTMLInputElement).value).toBe(''));
		expect(screen.getByRole('alert').textContent).toContain('Usuário ou senha incorretos.');
		expect(document.body.innerHTML).not.toContain('senha-que-nao-fica');
	});

	it('disables Sync now while a run is in flight and shows the outcome', async () => {
		let finish!: (result: SyncResult) => void;
		transport.syncNow = vi.fn(() => new Promise<SyncResult>((resolve) => { finish = resolve; }));
		transport.syncState = vi.fn()
			.mockResolvedValueOnce(state({ serverUrl: 'https://lv.example', signedIn: true, pending: 2 }))
			.mockResolvedValue(state({ serverUrl: 'https://lv.example', signedIn: true, pending: 0, lastResult: { pushed: 2, pulled: 1, pending: 0, error: null } }));
		renderSync();

		fireEvent.click(await screen.findByRole('button', { name: 'Sync now' }));
		expect(await screen.findByText('Syncing your changes…')).toBeTruthy();
		expect((screen.getByRole('button', { name: 'Sync now' }) as HTMLButtonElement).disabled).toBe(true);

		finish({ pushed: 2, pulled: 1, pending: 0, error: null });
		expect(await screen.findByText('Last sync: 2 sent, 1 received, 0 pending.')).toBeTruthy();
		expect(screen.getByText('Everything is synced.')).toBeTruthy();
		expect((screen.getByRole('button', { name: 'Sync now' }) as HTMLButtonElement).disabled).toBe(false);
	});

	it('shows a failed run as an alert without dropping the queue', async () => {
		transport.syncState = vi.fn().mockResolvedValue(state({ serverUrl: 'https://lv.example', signedIn: true, pending: 5, lastError: 'session_expired', lastResult: { pushed: 0, pulled: 0, pending: 5, error: 'session_expired' } }));
		renderSync();

		expect(await screen.findByText('Session expired. Sign in again.')).toBeTruthy();
		expect(screen.getByText('5 change(s) waiting to upload.')).toBeTruthy();
		expect(screen.getByRole('alert').textContent).toContain('Session expired');
	});
});
