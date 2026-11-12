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
	vi.spyOn(transport, 'listFlashcards').mockResolvedValue([]);
	vi.spyOn(transport, 'rewardSummary').mockRejectedValue(new Error('not needed'));
});
afterEach(() => {
	cleanup();
	delete (HTMLMediaElement.prototype as { error?: unknown }).error;
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
});

async function waitForVideo(): Promise<HTMLVideoElement> {
	await screen.findByRole('group', { name: 'Media player' });
	return document.querySelector('video') as HTMLVideoElement;
}
