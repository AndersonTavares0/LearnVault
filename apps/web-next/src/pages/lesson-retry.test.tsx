import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { transport, type Catalog, type CatalogNode, type FileEntry } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const video: FileEntry = { name: 'one.mp4', path: 'c/one.mp4', size: 1, mediaId: 'one', kind: 'media', availability: 'available' };
const lesson: CatalogNode = { title: 'Video', path: 'c/Video', role: 'lesson', files: [video], looseFiles: [], children: [] };
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{ title: 'Course', path: 'c', role: 'course', courseId: 'c', files: [], looseFiles: [], children: [lesson] }] };

/** jsdom has no decoder, so the element's error is supplied for the test. */
function failWith(code: number) {
	Object.defineProperty(HTMLMediaElement.prototype, 'error', { configurable: true, get: () => ({ code }) });
	fireEvent.error(document.querySelector('video')!);
}

beforeEach(() => {
	localStorage.clear();
	vi.spyOn(transport, 'mediaCapabilities').mockResolvedValue({ transcoding: false });
	vi.spyOn(transport, 'studyProgress').mockResolvedValue({ mediaId: 'one', positionSeconds: 0, completed: false, updatedAt: 1, recorded: false });
	vi.spyOn(transport, 'listNotes').mockResolvedValue([]);
	vi.spyOn(transport, 'listBookmarks').mockResolvedValue([]);
	vi.spyOn(transport, 'listFlashcards').mockResolvedValue([]);
	vi.spyOn(transport, 'rewardSummary').mockRejectedValue(new Error('not needed'));
});
afterEach(() => {
	cleanup();
	delete (HTMLMediaElement.prototype as { error?: unknown }).error;
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

describe('lesson load errors', () => {
	it('offers a new attempt for a decode failure and mounts a fresh media element', async () => {
		renderApp('/course/c/c%2FVideo', { catalog });
		const before = await waitForVideo();
		failWith(3);
		expect(await screen.findByText('The media could not be decoded.')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Try again' }));
		expect(screen.queryByRole('alert')).toBeNull();
		const after = await waitForVideo();
		expect(after).not.toBe(before);
	});

	it('does not offer a new attempt for an unsupported format', async () => {
		renderApp('/course/c/c%2FVideo', { catalog });
		await waitForVideo();
		failWith(4);
		expect(await screen.findByText('This media format or codec is not supported here.')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Try again' })).toBeNull();
	});

	it('reports a saturated server as retryable instead of a codec or disk problem', async () => {
		const head = vi.fn().mockResolvedValue(new Response(null, { status: 503 }));
		vi.stubGlobal('fetch', head);
		renderApp('/course/c/c%2FVideo', { catalog });
		await waitForVideo();
		failWith(4);
		expect(await screen.findByText('The server is busy right now.')).toBeTruthy();
		expect(screen.getByText(/Too many streams are open/)).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
		expect(head).toHaveBeenCalledWith('/api/media?course=c&path=c%2Fone.mp4', expect.objectContaining({ method: 'HEAD' }));
	});

	it('still explains a missing file when the probe is not a busy server', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(null, { status: 404 })));
		renderApp('/course/c/c%2FVideo', { catalog });
		await waitForVideo();
		failWith(2);
		expect(await screen.findByText('Could not load this media.')).toBeTruthy();
		expect(screen.getByText(/may have moved/)).toBeTruthy();
	});
});

async function waitForVideo(): Promise<HTMLVideoElement> {
	await screen.findByRole('group', { name: 'Media player' });
	return document.querySelector('video') as HTMLVideoElement;
}
