import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { resolveSession } from '@/lib/session';
import { HttpTransport, credentialsChangeMessage, transport, type Catalog, type User } from '@/lib/transport';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { keys } from '@/app/data';
import { routes } from '@/app/router';
import { renderApp } from '@/test/render-app';

/** Only the session is seeded, so a library query that fires would reach the transport spies. */
function renderBare(path: string, user: User) {
	const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
	client.setQueryData(keys.session, { user, error: '' });
	const router = createMemoryRouter(routes, { initialEntries: [path] });
	render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
	return { router };
}

const temporary: User = { userId: 'u1', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: true };
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{ title: 'Rust', path: 'r', role: 'course', courseId: 'rust', files: [], looseFiles: [], children: [] }] };

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	setLocale('en', null);
});

/** Every call the library, the update notice and the indexing indicator would make: all of them answer 403 for a temporary password. */
function spyOnProtected() {
	return [
		vi.spyOn(transport, 'catalog'),
		vi.spyOn(transport, 'checkUpdate'),
		vi.spyOn(transport, 'indexingStatus'),
		vi.spyOn(transport, 'studyState'),
		vi.spyOn(transport, 'listFavorites'),
		vi.spyOn(transport, 'rewardSummary')
	];
}

describe('forced credentials change', () => {
	it('sends every protected route to the account page with the banner and keeps the library queries quiet', async () => {
		const spies = spyOnProtected();
		for (const spy of spies) spy.mockRejectedValue(new Error(credentialsChangeMessage()));
		const { router } = renderBare('/', temporary);

		expect(await screen.findByRole('heading', { level: 1, name: 'Settings' })).toBeTruthy();
		await waitFor(() => expect(router.state.location.pathname).toBe('/settings'));
		expect(router.state.location.hash).toBe('#profile');
		expect(screen.getByRole('alert').textContent).toContain('Change your temporary password first.');
		expect(screen.getByLabelText('Current password')).toBeTruthy();
		expect(screen.queryByRole('navigation', { name: 'Main' })).toBeNull();

		await act(() => router.navigate('/progress'));
		await waitFor(() => expect(router.state.location.pathname).toBe('/settings'));
		await act(() => router.navigate('/course/rust'));
		await waitFor(() => expect(router.state.location.pathname).toBe('/settings'));
		for (const spy of spies) expect(spy).not.toHaveBeenCalled();
	});

	it('words the banner in Brazilian Portuguese', async () => {
		setLocale('pt-BR', null);
		renderApp('/settings', { user: temporary });
		expect((await screen.findByRole('alert')).textContent).toContain('Troque primeiro a senha temporária.');
		expect(document.body.textContent).not.toContain('credentials_change_required');
	});

	it('opens the library once the credentials are changed', async () => {
		const updated = { ...temporary, username: 'owner', mustChangeCredentials: false };
		vi.spyOn(transport, 'updateCredentials').mockResolvedValue(updated);
		vi.spyOn(transport, 'catalog').mockResolvedValue(catalog);
		const { router } = renderApp('/', { user: temporary });

		fireEvent.change(await screen.findByLabelText('Current password'), { target: { value: 'admin' } });
		fireEvent.change(screen.getByLabelText('New password'), { target: { value: 'a much longer passphrase' } });
		fireEvent.click(screen.getByRole('button', { name: 'Update credentials' }));

		expect(await screen.findByRole('heading', { level: 1, name: 'My learning' })).toBeTruthy();
		expect(router.state.location.pathname).toBe('/');
		expect(screen.queryByText('Change your temporary password first.')).toBeNull();
		expect(screen.getByRole('navigation', { name: 'Main' })).toBeTruthy();
	});

	it('never renders the raw error code', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 403 })));
		const message = await new HttpTransport().catalog().catch((error: Error) => error.message);
		vi.unstubAllGlobals();
		expect(message).toBe(credentialsChangeMessage());
		expect(message).not.toContain('credentials_change_required');
	});

	it('leaves the desktop single-user identity untouched', async () => {
		const readSession = vi.fn(async () => temporary);
		const session = await resolveSession(readSession, 'desktop');
		expect(readSession).not.toHaveBeenCalled();
		expect(session.user).toMatchObject({ username: 'Biblioteca local', mustChangeCredentials: false });
	});
});
