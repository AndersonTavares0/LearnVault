import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { persistFocusOutbox, readFocusOutbox, readRejectedFocusSessions } from './focus-outbox';
import type * as TransportModule from './transport';

const mocks = vi.hoisted(() => ({
	recordFocusSession: vi.fn(),
	focusSummary: vi.fn(),
	rewardSummary: vi.fn()
}));

vi.mock('./transport', async (importOriginal) => ({
	...(await importOriginal<typeof TransportModule>()),
	transport: mocks
}));

import {
	focusSync,
	pomodoro,
	pauseTimer,
	resetTimer,
	retryFocusSessions,
	setFocusOutboxScope,
	startTimer
} from './pomodoro-store';
import { FocusSessionConflict } from './transport';

const sessionA = { id: 'a', kind: 'focus' as const, plannedSeconds: 1500, actualSeconds: 1500, completed: true, startedAt: 100, endedAt: 1600 };
const sessionB = { ...sessionA, id: 'b' };

class MemoryStorage implements Storage {
	private values = new Map<string, string>();
	get length() { return this.values.size; }
	clear() { this.values.clear(); }
	getItem(key: string) { return this.values.get(key) ?? null; }
	key(index: number) { return [...this.values.keys()][index] ?? null; }
	removeItem(key: string) { this.values.delete(key); }
	setItem(key: string, value: string) { this.values.set(key, value); }
}

describe('Pomodoro sync scope changes', () => {
	let storage: MemoryStorage;
	let finishRequest: ((id: string) => void) | undefined;

	beforeEach(() => {
		vi.resetAllMocks();
		storage = new MemoryStorage();
		vi.stubGlobal('window', { localStorage: storage });
		mocks.focusSummary.mockResolvedValue({});
		mocks.rewardSummary.mockResolvedValue({});
		finishRequest = undefined;
	});
	afterEach(() => {
		setFocusOutboxScope(null);
		vi.unstubAllGlobals();
	});

	it('does not overlap accounts or mutate the new account when an old request completes', async () => {
		mocks.recordFocusSession.mockImplementation((session: { id: string }) => new Promise<void>((resolve) => {
			finishRequest = (id) => { if (id === session.id) resolve(); };
		}));
		persistFocusOutbox(storage, 'http:user-a', [sessionA]);
		setFocusOutboxScope('http:user-a');
		await vi.waitFor(() => expect(mocks.recordFocusSession).toHaveBeenCalledTimes(1));

		persistFocusOutbox(storage, 'http:user-b', [sessionB]);
		setFocusOutboxScope('http:user-b');
		expect(mocks.recordFocusSession).toHaveBeenCalledTimes(1);
		expect(get(focusSync).pending).toEqual([sessionB]);

		finishRequest?.('a');
		await vi.waitFor(() => expect(mocks.recordFocusSession).toHaveBeenCalledTimes(2));
		expect(mocks.recordFocusSession).toHaveBeenLastCalledWith(sessionB);
		expect(get(focusSync)).toMatchObject({ pending: [sessionB], savedAt: null });
		expect(readFocusOutbox(storage, 'http:user-a')).toEqual([sessionA]);
		expect(readFocusOutbox(storage, 'http:user-b')).toEqual([sessionB]);

		finishRequest?.('b');
		await vi.waitFor(() => expect(get(focusSync).pending).toEqual([]));
		expect(readFocusOutbox(storage, 'http:user-a')).toEqual([sessionA]);
		expect(readFocusOutbox(storage, 'http:user-b')).toEqual([]);
	});
	it('quarantines a conflict and continues saving the next session', async () => {
		mocks.recordFocusSession
			.mockRejectedValueOnce(new FocusSessionConflict())
			.mockResolvedValueOnce({});
		persistFocusOutbox(storage, 'http:user-a', [sessionA, sessionB]);
		setFocusOutboxScope('http:user-a');

		await vi.waitFor(() => expect(mocks.recordFocusSession).toHaveBeenCalledTimes(2));
		expect(mocks.recordFocusSession).toHaveBeenNthCalledWith(1, sessionA);
		expect(mocks.recordFocusSession).toHaveBeenNthCalledWith(2, sessionB);
		expect(get(focusSync)).toMatchObject({ pending: [], saving: false });
		expect(get(focusSync).rejected.map((entry) => entry.session)).toEqual([sessionA]);
		expect(readRejectedFocusSessions(storage, 'http:user-a').map((entry) => entry.session)).toEqual([sessionA]);
	});

	it('stops after a transient failure instead of retrying forever', async () => {
		mocks.recordFocusSession.mockRejectedValue(new Error('offline'));
		persistFocusOutbox(storage, 'http:user-a', [sessionA]);
		setFocusOutboxScope('http:user-a');
		await retryFocusSessions();

		expect(mocks.recordFocusSession).toHaveBeenCalledTimes(1);
		expect(get(focusSync)).toMatchObject({ pending: [sessionA], error: 'offline', saving: false });
	});
});

describe('Pomodoro timer controls', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.setSystemTime(new Date('2026-10-28T12:00:00.000Z'));
		resetTimer();
	});

	afterEach(() => {
		resetTimer();
		vi.useRealTimers();
	});

	it('shows start, pause, and resume immediately without advancing time early', () => {
		const initial = get(pomodoro);
		startTimer();
		const started = get(pomodoro);
		expect(started).toMatchObject({
			status: 'running',
			remainingSeconds: initial.remainingSeconds
		});
		expect(started.startedAt).toBe(Math.floor(Date.now() / 1000));

		vi.advanceTimersByTime(999);
		expect(get(pomodoro).remainingSeconds).toBe(initial.remainingSeconds);
		vi.advanceTimersByTime(1);
		expect(get(pomodoro).remainingSeconds).toBe(initial.remainingSeconds - 1);

		pauseTimer();
		expect(get(pomodoro)).toMatchObject({
			status: 'paused',
			remainingSeconds: initial.remainingSeconds - 1,
			startedAt: started.startedAt
		});
		vi.advanceTimersByTime(5_000);
		expect(get(pomodoro).remainingSeconds).toBe(initial.remainingSeconds - 1);

		startTimer();
		expect(get(pomodoro)).toMatchObject({
			status: 'running',
			remainingSeconds: initial.remainingSeconds - 1,
			startedAt: started.startedAt
		});
		vi.advanceTimersByTime(999);
		expect(get(pomodoro).remainingSeconds).toBe(initial.remainingSeconds - 1);
		vi.advanceTimersByTime(1);
		expect(get(pomodoro).remainingSeconds).toBe(initial.remainingSeconds - 2);
		expect(get(pomodoro).status).toBe('running');
	});
});
