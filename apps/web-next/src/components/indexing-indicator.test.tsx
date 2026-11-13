import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { transport, type IndexingSnapshot } from '@/lib/transport';
import { renderApp, admin } from '@/test/render-app';

const running: IndexingSnapshot = {
	status: 'running',
	startedAt: 1_760_000_000,
	finishedAt: null,
	progress: { filesSeen: 12, added: 3, changed: 1, removed: 0, moved: 1 },
	coursesTotal: 2,
	coursesCompleted: 1,
	lastError: null
};

const failed: IndexingSnapshot = { ...running, status: 'error', finishedAt: 1_760_000_100, lastError: 'course_unavailable' };
const idle: IndexingSnapshot = { ...running, status: 'idle', startedAt: null, finishedAt: null, lastError: null };

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('indexing indicator', () => {
	it('shows live progress and lets an administrator cancel the run', async () => {
		vi.spyOn(transport, 'indexingStatus').mockResolvedValue(running);
		const cancel = vi.spyOn(transport, 'cancelIndexing').mockResolvedValue(true);

		renderApp('/', { user: admin });
		const region = await screen.findByRole('region', { name: 'Library indexing' });
		expect(region.textContent).toContain('Indexing your library');
		expect(region.textContent).toContain('12 files seen');
		expect(region.textContent).toContain('3 added');
		expect(region.textContent).toContain('0 removed');

		fireEvent.click(screen.getByRole('button', { name: 'Cancel indexing' }));
		await waitFor(() => expect(cancel).toHaveBeenCalled());
	});

	it('reports a failed run to members without offering the admin action', async () => {
		vi.spyOn(transport, 'indexingStatus').mockResolvedValue(failed);
		const cancel = vi.spyOn(transport, 'cancelIndexing').mockResolvedValue(true);

		renderApp('/', { user: { userId: 'u2', username: 'sam', role: 'member', isAdmin: false, mustChangeCredentials: false } });
		const region = await screen.findByRole('region', { name: 'Library indexing' });
		expect(region.textContent).toContain('Some courses could not be indexed');
		expect(screen.queryByRole('button', { name: 'Cancel indexing' })).toBeNull();
		expect(cancel).not.toHaveBeenCalled();
	});

	it('stays quiet when no job is running or failed', async () => {
		vi.spyOn(transport, 'indexingStatus').mockResolvedValue(idle);

		renderApp('/', { user: admin });
		await screen.findByRole('heading', { level: 1 });
		await waitFor(() => expect(transport.indexingStatus).toHaveBeenCalled());
		expect(screen.queryByRole('region', { name: 'Library indexing' })).toBeNull();
	});
});
