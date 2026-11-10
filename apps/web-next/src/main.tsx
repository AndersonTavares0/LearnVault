import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { RouterProvider } from 'react-router/dom';
import { applyStoredPreferences } from './app/preferences';
import { createRouter } from './app/router';
import './styles/app.css';

applyStoredPreferences();

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: false } } });

createRoot(document.getElementById('app')!).render(
	<StrictMode>
		<QueryClientProvider client={queryClient}>
			<RouterProvider router={createRouter()} />
		</QueryClientProvider>
	</StrictMode>
);
