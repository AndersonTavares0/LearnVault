import { render, screen } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { afterEach, describe, expect, it } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { routes } from './router';

function open(path: string) {
	const router = createMemoryRouter(routes, { initialEntries: [path] });
	render(<RouterProvider router={router} />);
	return router;
}

afterEach(() => setLocale('en'));

describe('routes', () => {
	it('renders the shell with a skip link and main landmark', async () => {
		open('/');
		expect(await screen.findByRole('heading', { name: 'My learning' })).toBeTruthy();
		expect(screen.getByRole('link', { name: 'Skip to content' })).toBeTruthy();
		expect(screen.getByRole('main')).toBeTruthy();
		expect(screen.getByRole('navigation', { name: 'Main' })).toBeTruthy();
	});

	it('maps legacy statistics and focus links to progress', async () => {
		for (const path of ['/statistics', '/focus']) {
			const router = open(path);
			expect(await screen.findByRole('heading', { name: 'Progress' })).toBeTruthy();
			expect(router.state.location.pathname).toBe('/progress');
			document.body.innerHTML = '';
		}
	});

	it('maps a legacy player link to the course route and keeps the timestamp', async () => {
		const router = open('/player/course-1/module%2Flesson.mp4?t=42');
		expect(await screen.findByRole('heading', { name: 'Course' })).toBeTruthy();
		expect(router.state.location.pathname).toBe('/course/course-1/module%2Flesson.mp4');
		expect(router.state.location.search).toBe('?t=42');
	});

	it('follows the selected language', async () => {
		setLocale('pt-BR');
		open('/progress');
		expect(await screen.findByRole('heading', { name: 'Progresso' })).toBeTruthy();
	});
});
