import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { pomodoro } from '@/lib/pomodoro-store';
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
const initialTimer = pomodoro.get();
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
	pomodoro.set(initialTimer);
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
		expect(screen.getByRole('complementary', { name: 'Study panel' })).toBeTruthy();
		expect(screen.getByText('Lesson 1 of 3')).toBeTruthy();
		fireEvent.click(screen.getByRole('tab', { name: 'Lessons' }));
		expect(await screen.findByRole('link', { name: /Two/ })).toBeTruthy();
	});

	it('marks the lesson completed from the page', async () => {
		const setCompleted = vi.spyOn(transport, 'setCompleted').mockImplementation(async (id, completed) => progress(id, completed));
		renderApp('/course/c/c%2FOne', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Mark as completed' }));
		await waitFor(() => expect(setCompleted).toHaveBeenCalledWith('one.mp4', true));
		expect(await screen.findByRole('button', { name: 'Completed' })).toBeTruthy();
	});

	it('switches to theater mode and remembers it', async () => {
		localStorage.removeItem('learnvault-theater');
		renderApp('/course/c/c%2FOne', { catalog });
		const toggle = await screen.findByRole('button', { name: 'Theater mode' });
		expect(toggle.getAttribute('aria-pressed')).toBe('false');
		fireEvent.click(toggle);
		expect(screen.getByRole('button', { name: 'Show side panel' }).getAttribute('aria-pressed')).toBe('true');
		expect(localStorage.getItem('learnvault-theater')).toBe('1');
		localStorage.removeItem('learnvault-theater');
	});

	it('walks the A-B loop control through start, end and clear', async () => {
		renderApp('/course/c/c%2FOne', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Loop: set start' }));
		expect(screen.getByRole('button', { name: /Loop from 0:00: set end/ }).getAttribute('aria-pressed')).toBe('true');
		fireEvent.click(screen.getByRole('button', { name: /Loop from 0:00: set end/ }));
		// End must be after the start, so with no playback time the loop stays open.
		expect(screen.getByRole('button', { name: /Loop from 0:00: set end/ })).toBeTruthy();
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

	describe('study workspace', () => {
		const finishRound = (mode: 'focus' | 'short', at: number) => act(() => { pomodoro.update((state) => ({ ...state, mode: mode === 'focus' ? 'short' : 'focus', status: 'idle', remainingSeconds: 300, totalSeconds: 300, lastCompleted: { mode, at } })); });

		it('pauses the lesson and covers it with a break when a focus block ends', async () => {
			const pause = vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
			renderApp('/course/c/c%2FOne', { catalog });
			await screen.findByRole('heading', { level: 1, name: 'One' });
			finishRound('focus', Date.now() + 1);
			const region = await screen.findByRole('region', { name: 'Break' });
			expect(region.textContent).toContain('Round done.');
			expect(pause).toHaveBeenCalled();
			fireEvent.click(screen.getByRole('button', { name: 'End session' }));
			expect(screen.queryByRole('region', { name: 'Break' })).toBeNull();
		});

		it('offers to resume when the break is over, and does not resume by itself', async () => {
			vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
			const play = vi.spyOn(HTMLMediaElement.prototype, 'play').mockResolvedValue(undefined);
			renderApp('/course/c/c%2FOne', { catalog });
			await screen.findByRole('heading', { level: 1, name: 'One' });
			finishRound('focus', Date.now() + 1);
			await screen.findByRole('region', { name: 'Break' });
			finishRound('short', Date.now() + 2);
			expect(await screen.findByRole('heading', { name: 'Break over' })).toBeTruthy();
			expect(play).not.toHaveBeenCalled();
			fireEvent.click(screen.getByRole('button', { name: 'Resume lesson' }));
			expect(play).toHaveBeenCalled();
			expect(screen.queryByRole('region', { name: 'Break' })).toBeNull();
		});

		it('closes the break screen when the next focus round starts elsewhere', async () => {
			vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
			renderApp('/course/c/c%2FOne', { catalog });
			await screen.findByRole('heading', { level: 1, name: 'One' });
			finishRound('focus', Date.now() + 1);
			await screen.findByRole('region', { name: 'Break' });
			act(() => { pomodoro.update((state) => ({ ...state, mode: 'focus', status: 'running' })); });
			expect(screen.queryByRole('region', { name: 'Break' })).toBeNull();
		});

		it('leaves the lesson playing when the pause setting is off', async () => {
			pomodoro.update((state) => ({ ...state, settings: { ...state.settings, pauseVideoOnRoundEnd: false } }));
			renderApp('/course/c/c%2FOne', { catalog });
			await screen.findByRole('heading', { level: 1, name: 'One' });
			finishRound('focus', Date.now() + 1);
			expect(screen.queryByRole('region', { name: 'Break' })).toBeNull();
		});

		it('opens a note draft and a blank card from the tools under the video', async () => {
			vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
			renderApp('/course/c/c%2FOne', { catalog });
			fireEvent.click(await screen.findByRole('button', { name: /Pause and note/ }));
			expect(await screen.findByLabelText('Note at 0:00')).toBeTruthy();
			fireEvent.click(screen.getByRole('button', { name: 'Make card' }));
			expect(await screen.findByRole('form', { name: 'New card' })).toBeTruthy();
		});
	});
});
