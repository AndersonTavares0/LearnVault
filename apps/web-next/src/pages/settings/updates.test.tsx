import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { transport, type UpdateStatus } from '@/lib/transport';
import { UpdateSettings } from './updates';

const status: UpdateStatus = { current: '0.3.0', enabled: true, available: false, latest: null, releasedAt: null, url: null, changelog: null, checkedAt: '1', error: null };

function renderUpdates() {
	const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
	render(<QueryClientProvider client={client}><UpdateSettings /></QueryClientProvider>);
}

afterEach(() => {
	cleanup();
	setLocale('en', null);
	vi.restoreAllMocks();
});

describe('update settings', () => {
	it('localizes the credential error of a forbidden update check in pt-BR', async () => {
		setLocale('pt-BR', null);
		vi.spyOn(transport, 'updateHistory').mockResolvedValue([]);
		vi.spyOn(transport, 'checkUpdate').mockResolvedValueOnce(status).mockRejectedValue(new Error('credentials_change_required'));
		renderUpdates();

		fireEvent.click(await screen.findByRole('button', { name: 'Verificar atualização' }));

		const alert = await screen.findByRole('alert');
		expect(alert.textContent).toBe('Troque sua senha no servidor antes de verificar atualizações.');
		expect(document.body.innerHTML).not.toContain('credentials_change_required');
	});

	it('falls back to a generic localized message for an unknown update error code', async () => {
		vi.spyOn(transport, 'updateHistory').mockResolvedValue([]);
		vi.spyOn(transport, 'checkUpdate').mockResolvedValueOnce(status).mockRejectedValue(new Error('update_gateway_unavailable'));
		renderUpdates();

		fireEvent.click(await screen.findByRole('button', { name: 'Check for updates' }));

		const alert = await screen.findByRole('alert');
		expect(alert.textContent).toBe('Could not complete the update request. Try again.');
		expect(document.body.innerHTML).not.toContain('update_gateway_unavailable');
	});

	it('keeps a message that is already localized', async () => {
		setLocale('pt-BR', null);
		vi.spyOn(transport, 'updateHistory').mockResolvedValue([]);
		vi.spyOn(transport, 'checkUpdate').mockResolvedValueOnce(status).mockRejectedValue(new Error('Não foi possível verificar a atualização.'));
		renderUpdates();

		fireEvent.click(await screen.findByRole('button', { name: 'Verificar atualização' }));

		expect((await screen.findByRole('alert')).textContent).toBe('Não foi possível verificar a atualização.');
	});
});
