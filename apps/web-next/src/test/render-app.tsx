import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { keys } from '@/app/data';
import { routes } from '@/app/router';
import type { Catalog, RewardSummary, StudyFavorites, StudyProgress, User } from '@/lib/transport';

export const admin: User = { userId: 'u1', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: false };

/** Renders the real routes with query data seeded, so no test touches the network. */
export function renderApp(path: string, seed: { user?: User | null; catalog?: Catalog; recent?: StudyProgress[]; favorites?: StudyFavorites; rewards?: RewardSummary } = {}) {
	const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
	client.setQueryData(keys.session, { user: seed.user === undefined ? admin : seed.user, error: '' });
	client.setQueryData(keys.catalog, seed.catalog ?? { complete: true, looseFiles: [], nodes: [] });
	client.setQueryData(keys.recent, seed.recent ?? []);
	client.setQueryData(keys.favorites, seed.favorites ?? { courseIds: [], mediaIds: [] });
	if (seed.rewards) client.setQueryData(keys.rewards, seed.rewards);
	const router = createMemoryRouter(routes, { initialEntries: [path] });
	render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>);
	return { router, client };
}
