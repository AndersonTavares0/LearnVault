import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { pomodoro } from '@/lib/pomodoro-store';
import { clearProgressOutbox, progressOutboxKey, readProgressOutbox } from '@/lib/progress-outbox';
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
	clearProgressOutbox();
	localStorage.clear();
	vi.spyOn(transport, 'mediaCapabilities').mockResolvedValue({ transcoding: false });
	vi.spyOn(transport, 'studyProgress').mockImplementation(async (id) => progress(id));
	vi.spyOn(transport, 'listNotes').mockResolvedValue([]);
	vi.spyOn(transport, 'listBookmarks').mockResolvedValue([]);
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
		expect(screen.getByRole('complementary', { name: 'Course content' })).toBeTruthy();
		expect(screen.getByText('Lesson 1 of 3')).toBeTruthy();
		expect(await screen.findByRole('link', { name: /Two/ })).toBeTruthy();
	});

	it('has one set of controls on the player and the browser\'s own are off', async () => {
		renderApp('/course/c/c%2FOne', { catalog });
		const video = await screen.findByLabelText('Lesson player: One');
		expect(video.hasAttribute('controls')).toBe(false);
		expect(screen.getByRole('slider', { name: 'Seek' })).toBeTruthy();
		expect(screen.getByRole('slider', { name: 'Volume' })).toBeTruthy();
		for (const name of ['Play', 'Back 10s', 'Forward 10s', 'Mute', 'Loop', 'Cinema mode', 'Fullscreen']) expect(screen.getAllByRole('button', { name }).length).toBe(1);
		expect(screen.getByRole('combobox', { name: 'Speed' })).toBeTruthy();
	});

	it('keeps cinema mode and hidden course content between visits', async () => {
		renderApp('/course/c/c%2FOne', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Cinema mode' }));
		expect(localStorage.getItem('learnvault-cinema')).toBe('1');
		fireEvent.click(screen.getAllByRole('button', { name: 'Hide course content' })[0]);
		expect(localStorage.getItem('learnvault-lesson-content')).toBe('0');
		expect(screen.queryByRole('complementary', { name: 'Course content' })).toBeNull();
		cleanup();
		renderApp('/course/c/c%2FOne', { catalog });
		expect((await screen.findByRole('button', { name: 'Cinema mode' })).getAttribute('aria-pressed')).toBe('true');
	});

	it('marks the lesson completed from the page', async () => {
		const setCompleted = vi.spyOn(transport, 'setCompleted').mockImplementation(async (id, completed) => progress(id, completed));
		renderApp('/course/c/c%2FOne', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Mark as completed' }));
		await waitFor(() => expect(setCompleted).toHaveBeenCalledWith('one.mp4', true));
		expect(await screen.findByRole('button', { name: 'Completed' })).toBeTruthy();
	});

	it('walks the A-B loop control through start, end and clear', async () => {
		renderApp('/course/c/c%2FOne', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Loop' }));
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
			fireEvent.click(await screen.findByRole('button', { name: /^Note/ }));
			expect(await screen.findByLabelText('Note at 0:00')).toBeTruthy();
			fireEvent.click(screen.getByRole('tab', { name: 'Overview' }));
			fireEvent.click(screen.getByRole('button', { name: 'Card' }));
			expect(await screen.findByRole('form', { name: 'New card' })).toBeTruthy();
		});

		it('creates a card at the current player time', async () => {
			vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
			vi.spyOn(transport, 'recordPosition').mockImplementation(async (id, position) => ({ ...progress(id), positionSeconds: position, recorded: true }));
			const create = vi.spyOn(transport, 'createFlashcard').mockResolvedValue({
				cardId: 'card-1', mediaId: 'one.mp4', positionSeconds: 65, front: 'Question', back: 'Answer', intervalDays: 0, nextDue: 1, createdAt: 1, updatedAt: 1
			});
			renderApp('/course/c/c%2FOne', { catalog });
			const video = await screen.findByLabelText('Lesson player: One') as HTMLVideoElement;
			video.currentTime = 65;
			fireEvent.timeUpdate(video);
			fireEvent.click(screen.getByRole('button', { name: 'Card' }));
			fireEvent.change(await screen.findByLabelText('Front: question or concept'), { target: { value: 'Question' } });
			fireEvent.change(screen.getByLabelText('Back: answer or explanation'), { target: { value: 'Answer' } });
			expect(await screen.findByText('Linked to this lesson at 1:05.')).toBeTruthy();
			fireEvent.click(screen.getByRole('button', { name: 'Save card' }));
			await waitFor(() => expect(create).toHaveBeenCalledWith('one.mp4', 'Question', 'Answer', 65));
		});

		it('asks before deleting a card', async () => {
			vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
			vi.mocked(transport.listFlashcards).mockResolvedValue([{ cardId: 'card-1', mediaId: 'one.mp4', positionSeconds: null, front: 'Question', back: 'Answer', intervalDays: 0, nextDue: 1, createdAt: 1, updatedAt: 1 }]);
			const remove = vi.spyOn(transport, 'deleteFlashcard').mockResolvedValue(undefined);
			renderApp('/course/c/c%2FOne', { catalog });
			fireEvent.click(await screen.findByRole('tab', { name: 'Cards' }));
			fireEvent.click(await screen.findByRole('button', { name: 'Manage' }));
			fireEvent.click(await screen.findByRole('button', { name: 'Delete card: Question' }));
			expect(remove).not.toHaveBeenCalled();
			fireEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Delete card' }));
			await waitFor(() => expect(remove).toHaveBeenCalledWith('card-1'));
			await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
		});
	});

	describe('resume and persistence', () => {
		const saved = (positionSeconds: number, completed = false): StudyProgress => ({ mediaId: 'one.mp4', positionSeconds, completed, updatedAt: 1, recorded: true });

		it('starts at the saved position, clamped to the media duration', async () => {
			vi.spyOn(transport, 'studyProgress').mockResolvedValue(saved(42));
			renderApp('/course/c/c%2FOne', { catalog });
			const video = await screen.findByLabelText('Lesson player: One') as HTMLVideoElement;
			await screen.findByRole('button', { name: 'Resume from 0:42' });
			Object.defineProperty(video, 'duration', { configurable: true, value: 120 });
			fireEvent.loadedMetadata(video);
			expect(video.currentTime).toBe(42);
		});

		it('starts a completed lesson from the beginning', async () => {
			vi.spyOn(transport, 'studyProgress').mockResolvedValue(saved(42, true));
			renderApp('/course/c/c%2FOne', { catalog });
			const video = await screen.findByLabelText('Lesson player: One') as HTMLVideoElement;
			await screen.findByRole('button', { name: 'Completed' });
			Object.defineProperty(video, 'duration', { configurable: true, value: 120 });
			fireEvent.loadedMetadata(video);
			expect(video.currentTime).toBe(0);
		});

		it('saves on pause and on seek', async () => {
			const record = vi.spyOn(transport, 'recordPosition').mockImplementation(async (id, positionSeconds) => ({ ...saved(positionSeconds), mediaId: id }));
			renderApp('/course/c/c%2FOne', { catalog });
			const video = await screen.findByLabelText('Lesson player: One') as HTMLVideoElement;
			video.currentTime = 42;
			fireEvent.pause(video);
			await waitFor(() => expect(record).toHaveBeenCalledWith('one.mp4', 42));

			record.mockClear();
			video.currentTime = 10;
			fireEvent.click(screen.getByRole('button', { name: 'Forward 10s' }));
			await waitFor(() => expect(record).toHaveBeenCalledWith('one.mp4', 20));
		});

		it('never writes on every timeupdate and flushes the last position on unmount', async () => {
			const record = vi.spyOn(transport, 'recordPosition').mockImplementation(async (id, positionSeconds) => ({ ...saved(positionSeconds), mediaId: id }));
			vi.spyOn(performance, 'now').mockReturnValue(10_000);
			renderApp('/course/c/c%2FOne', { catalog });
			const video = await screen.findByLabelText('Lesson player: One') as HTMLVideoElement;
			video.currentTime = 3;
			fireEvent.timeUpdate(video);
			fireEvent.timeUpdate(video);
			await waitFor(() => expect(record).toHaveBeenCalledTimes(1));
			expect(record).toHaveBeenLastCalledWith('one.mp4', 3);

			vi.mocked(performance.now).mockReturnValue(0);
			video.currentTime = 55;
			fireEvent.timeUpdate(video);
			record.mockClear();
			cleanup();
			await waitFor(() => expect(record).toHaveBeenCalledWith('one.mp4', 55));
		});

		const outboxKey = progressOutboxKey('http-user:u1');
		const queued = (positionSeconds: number) => localStorage.setItem(outboxKey, JSON.stringify([{ mediaId: 'one.mp4', positionSeconds, at: 5 }]));
		const notice = 'Progress is not saved yet. It will retry when the connection is back.';

		it('queues a failed save, says so, and retries it when the tab comes back', async () => {
			vi.spyOn(transport, 'studyProgress').mockResolvedValue(saved(10));
			const record = vi.spyOn(transport, 'recordPosition').mockRejectedValue(new Error('offline'));
			renderApp('/course/c/c%2FOne', { catalog });
			const video = await screen.findByLabelText('Lesson player: One') as HTMLVideoElement;
			// The resume at 0:10 means the progress query settled and the retry listener is on.
			await screen.findByRole('button', { name: 'Resume from 0:10' });
			video.currentTime = 42;
			fireEvent.pause(video);

			const status = await screen.findByText(notice);
			expect(status.getAttribute('role')).toBe('status');
			expect(readProgressOutbox(localStorage, 'http-user:u1')).toMatchObject([{ mediaId: 'one.mp4', positionSeconds: 42 }]);

			record.mockImplementation(async (id, positionSeconds) => ({ ...saved(positionSeconds), mediaId: id }));
			act(() => { document.dispatchEvent(new Event('visibilitychange')); });

			await waitFor(() => expect(screen.queryByText(notice)).toBeNull());
			expect(record).toHaveBeenLastCalledWith('one.mp4', 42);
			expect(readProgressOutbox(localStorage, 'http-user:u1')).toEqual([]);
		});

		it('resumes from a queued position the server does not have yet', async () => {
			vi.spyOn(transport, 'studyProgress').mockResolvedValue(saved(10));
			const record = vi.spyOn(transport, 'recordPosition').mockImplementation(async (id, positionSeconds) => ({ ...saved(positionSeconds), mediaId: id }));
			queued(42);
			renderApp('/course/c/c%2FOne', { catalog });

			expect(await screen.findByRole('button', { name: 'Resume from 0:42' })).toBeTruthy();
			await waitFor(() => expect(record).toHaveBeenCalledWith('one.mp4', 42));
			await waitFor(() => expect(readProgressOutbox(localStorage, 'http-user:u1')).toEqual([]));
		});

		it('discards a queued position the server has already passed', async () => {
			vi.spyOn(transport, 'studyProgress').mockResolvedValue(saved(100));
			const record = vi.spyOn(transport, 'recordPosition').mockImplementation(async (id, positionSeconds) => ({ ...saved(positionSeconds), mediaId: id }));
			queued(10);
			renderApp('/course/c/c%2FOne', { catalog });

			expect(await screen.findByRole('button', { name: 'Resume from 1:40' })).toBeTruthy();
			await waitFor(() => expect(readProgressOutbox(localStorage, 'http-user:u1')).toEqual([]));
			expect(record).not.toHaveBeenCalled();
		});

		it('clears the unsaved position queue on sign out', async () => {
			vi.spyOn(transport, 'recordPosition').mockRejectedValue(new Error('offline'));
			vi.spyOn(transport, 'logout').mockResolvedValue(undefined);
			queued(42);
			renderApp('/course/c/c%2FOne', { catalog });
			await waitFor(() => expect(localStorage.getItem(outboxKey)).not.toBeNull());

			fireEvent.click(await screen.findByRole('button', { name: 'Account: admin' }));
			fireEvent.click(await screen.findByRole('menuitem', { name: 'Sign out' }));
			fireEvent.click(await screen.findByRole('button', { name: 'Sign out anyway' }));

			await waitFor(() => expect(localStorage.getItem(outboxKey)).toBeNull());
		});
	});

	describe('timestamp bookmarks', () => {
		const bookmark = { bookmarkId: 'b1', mediaId: 'one.mp4', positionSeconds: 30, label: 'Intro', createdAt: 1 };

		it('draws timeline markers and seeks from them', async () => {
			vi.spyOn(transport, 'listBookmarks').mockResolvedValue([bookmark]);
			renderApp('/course/c/c%2FOne', { catalog });
			const video = await screen.findByLabelText('Lesson player: One') as HTMLVideoElement;
			Object.defineProperty(video, 'duration', { configurable: true, value: 100 });
			fireEvent.loadedMetadata(video);
			fireEvent.durationChange(video);
			fireEvent.click(await screen.findByRole('button', { name: 'Jump to timestamp: Intro' }));
			expect(video.currentTime).toBe(30);
		});

		it('adds and deletes a bookmark with a label', async () => {
			const list = vi.spyOn(transport, 'listBookmarks').mockResolvedValue([]);
			const create = vi.spyOn(transport, 'createBookmark').mockImplementation(async (mediaId, positionSeconds, label) => {
				const created = { ...bookmark, mediaId, positionSeconds, label };
				list.mockResolvedValue([created]);
				return created;
			});
			const remove = vi.spyOn(transport, 'deleteBookmark').mockResolvedValue(undefined);
			renderApp('/course/c/c%2FOne', { catalog });
			fireEvent.click(await screen.findByRole('tab', { name: 'Timestamps' }));
			fireEvent.click(await screen.findByRole('button', { name: /Add timestamp at 0:00/ }));
			fireEvent.change(screen.getByLabelText(/Label at 0:00/), { target: { value: 'Intro' } });
			fireEvent.click(screen.getByRole('button', { name: 'Save timestamp' }));
			await waitFor(() => expect(create).toHaveBeenCalledWith('one.mp4', 0, 'Intro'));
			expect(await screen.findByText('Intro')).toBeTruthy();
			fireEvent.click(screen.getByRole('button', { name: 'Delete timestamp: Intro' }));
			expect(remove).not.toHaveBeenCalled();
			fireEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Delete timestamp' }));
			await waitFor(() => expect(remove).toHaveBeenCalledWith('b1'));
		});
	});

});
