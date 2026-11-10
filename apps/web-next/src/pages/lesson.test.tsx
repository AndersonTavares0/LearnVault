import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { transport, type Catalog, type CatalogNode, type FileEntry, type StudyProgress } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const media = (name: string, availability = 'available'): FileEntry => ({ name, path: `c/${name}`, size: 1, mediaId: name, kind: 'media', availability });
const lesson = (title: string, files: FileEntry[]): CatalogNode => ({ title, path: `c/${title}`, role: 'lesson', files, looseFiles: [], children: [] });
const catalog: Catalog = {
	complete: true,
	looseFiles: [],
	nodes: [{
		title: 'Course', path: 'c', role: 'course', courseId: 'c', files: [], looseFiles: [],
		children: [lesson('One', [media('one.mp4')]), lesson('Two', [media('two.mp4')]), lesson('Gone', [media('gone.mp4', 'missing')])]
	}]
};
const progress = (mediaId: string, completed = false): StudyProgress => ({ mediaId, positionSeconds: 0, completed, updatedAt: 1, recorded: false });

beforeEach(() => {
	localStorage.clear();
	vi.spyOn(transport, 'mediaCapabilities').mockResolvedValue({ transcoding: false });
	vi.spyOn(transport, 'studyProgress').mockImplementation(async (id) => progress(id));
	vi.spyOn(transport, 'listNotes').mockResolvedValue([]);
	vi.spyOn(transport, 'listFlashcards').mockResolvedValue([]);
	vi.spyOn(transport, 'rewardSummary').mockRejectedValue(new Error('not needed'));
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('lesson page', () => {
	it('plays the lesson media and links to neighbouring lessons', async () => {
		renderApp('/course/c/c%2FOne', { catalog });
		expect(await screen.findByRole('heading', { level: 1, name: 'One' })).toBeTruthy();
		expect(screen.getByLabelText('Lesson player: One').getAttribute('src')).toBe('/api/media?course=c&path=c%2Fone.mp4');
		expect((screen.getByRole('button', { name: /Previous/ }) as HTMLButtonElement).disabled).toBe(true);
		expect((screen.getByRole('button', { name: /Next/ }) as HTMLButtonElement).disabled).toBe(false);
		expect(screen.getByRole('complementary', { name: 'Course content' })).toBeTruthy();
	});

	it('marks the lesson completed from the page', async () => {
		const setCompleted = vi.spyOn(transport, 'setCompleted').mockImplementation(async (id, completed) => progress(id, completed));
		renderApp('/course/c/c%2FOne', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Mark as completed' }));
		await waitFor(() => expect(setCompleted).toHaveBeenCalledWith('one.mp4', true));
		expect(await screen.findByRole('button', { name: 'Completed' })).toBeTruthy();
	});

	it('opens a note draft with the n shortcut', async () => {
		renderApp('/course/c/c%2FOne', { catalog });
		fireEvent.keyDown(await screen.findByRole('group', { name: 'Media player' }), { key: 'n' });
		expect(await screen.findByLabelText('Note at 0:00')).toBeTruthy();
	});

	it('counts down to the next lesson when auto-advance is on', async () => {
		vi.spyOn(transport, 'setCompleted').mockImplementation(async (id, completed) => progress(id, completed));
		const { router } = renderApp('/course/c/c%2FOne', { catalog });
		fireEvent.click(await screen.findByRole('checkbox', { name: 'Play the next lesson automatically' }));
		vi.useFakeTimers();
		fireEvent.ended(screen.getByLabelText('Lesson player: One'));
		expect(screen.getByText('Next lesson in 5 s: Two')).toBeTruthy();
		for (let second = 0; second < 5; second += 1) await act(async () => { vi.advanceTimersByTime(1000); });
		vi.useRealTimers();
		expect(router.state.location.pathname).toBe('/course/c/c%2FTwo');
	});

	it('explains missing media without losing the lesson', async () => {
		renderApp('/course/c/c%2FGone', { catalog });
		expect(await screen.findByText('Media unavailable')).toBeTruthy();
		expect(screen.getByRole('heading', { level: 1, name: 'Gone' })).toBeTruthy();
	});
});
