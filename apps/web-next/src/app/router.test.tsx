import { cleanup, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { renderApp } from '@/test/render-app';

afterEach(() => {
	cleanup();
	setLocale('en');
});

describe('routes', () => {
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
		expect(await screen.findByRole('heading', { name: 'Sign in to LearnVault' })).toBeTruthy();
		expect(router.state.location.pathname).toBe('/login');
		expect(router.state.location.state).toEqual({ from: '/notes' });
	});

	it('maps legacy statistics and focus links to progress', async () => {
		for (const path of ['/statistics', '/focus']) {
			const { router } = renderApp(path);
			expect(await screen.findByRole('heading', { name: 'Progress' })).toBeTruthy();
			expect(router.state.location.pathname).toBe('/progress');
			cleanup();
		}
	});

	it('maps a legacy player link to the course route and keeps the timestamp', async () => {
		const { router } = renderApp('/player/course-1/module%2Flesson.mp4?t=42');
		expect(await screen.findByRole('heading', { name: 'Course' })).toBeTruthy();
		expect(router.state.location.pathname).toBe('/course/course-1/module%2Flesson.mp4');
		expect(router.state.location.search).toBe('?t=42');
	});

	it('follows the selected language', async () => {
		setLocale('pt-BR');
		renderApp('/progress');
		expect(await screen.findByRole('heading', { name: 'Progresso' })).toBeTruthy();
	});
});
