import { act, cleanup, fireEvent, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { renderApp } from '@/test/render-app';

afterEach(() => {
	cleanup();
	setLocale('en');
	document.title = 'LearnVault';
});

describe('routes', () => {
	it('updates the tab title when navigating, going back or changing language', async () => {
		const { router } = renderApp('/');
		await screen.findByRole('heading', { name: 'My learning' });
		expect(document.title).toBe('My learning · LearnVault');

		const nav = screen.getByRole('navigation', { name: 'Main' });
		fireEvent.click(within(nav).getByRole('link', { name: 'Progress' }));
		await screen.findByRole('heading', { name: 'Your progress', level: 1 });
		expect(document.title).toBe('Your progress · LearnVault');

		await act(() => router.navigate(-1));
		expect(document.title).toBe('My learning · LearnVault');
		act(() => setLocale('pt-BR'));
		expect(document.title).toBe('Meu aprendizado · LearnVault');
	});

	it('renders the shell with a skip link, landmarks and both navigations', async () => {
		renderApp('/');
		expect(await screen.findByRole('heading', { name: 'My learning' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Skip to content' })).toBeTruthy();
		expect(screen.getByRole('main')).toBeTruthy();
		expect(screen.getByRole('navigation', { name: 'Main' })).toBeTruthy();
		expect(screen.getByRole('navigation', { name: 'Sections' })).toBeTruthy();
	});

	it('sends a server visitor without a session to sign-in and remembers the destination', async () => {
		const { router } = renderApp('/notes', { user: null });
		expect(await screen.findByRole('heading', { name: 'Sign in', level: 1 })).toBeTruthy();
		expect(router.state.location.pathname).toBe('/login');
		expect(router.state.location.state).toMatchObject({ from: '/notes' });
		expect(screen.queryByText(/Your session expired/)).toBeNull();
		expect(document.title).toBe('Sign in · LearnVault');
	});

	it('maps legacy statistics and focus links to progress', async () => {
		for (const path of ['/statistics', '/focus']) {
			const { router } = renderApp(path);
			expect(await screen.findByRole('heading', { name: 'Your progress' })).toBeTruthy();
			expect(router.state.location.pathname).toBe('/progress');
			cleanup();
		}
	});

	it('maps a legacy player link to the course route and keeps the timestamp', async () => {
		const { router } = renderApp('/player/course-1/module%2Flesson.mp4?t=42');
		expect(await screen.findByRole('heading', { name: 'Course not found' })).toBeTruthy();
		expect(router.state.location.pathname).toBe('/course/course-1/module%2Flesson.mp4');
		expect(router.state.location.search).toBe('?t=42');
		expect(document.title).toBe('Course not found · LearnVault');
	});

	it('follows the selected language', async () => {
		setLocale('pt-BR');
		renderApp('/progress');
		expect(await screen.findByRole('heading', { name: 'Seu progresso' })).toBeTruthy();
	});
});

describe('account menu', () => {
	it('opens without crashing and offers settings and sign-out', async () => {
		renderApp('/');
		fireEvent.click(await screen.findByRole('button', { name: 'Account: admin' }));
		expect(await screen.findByRole('menuitem', { name: 'Settings' })).toBeTruthy();
		expect(screen.getByRole('menuitem', { name: 'Sign out' })).toBeTruthy();
	});
});
