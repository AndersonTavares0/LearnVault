import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { focusOutboxKey, persistFocusOutbox, readFocusOutbox } from './focus-outbox';

const mocks = vi.hoisted(() => ({
	recordFocusSession: vi.fn(),
	focusSummary: vi.fn(),
	rewardSummary: vi.fn()
}));

vi.mock('./transport', () => ({ transport: mocks }));

import { focusSync, setFocusOutboxScope } from './pomodoro-store';

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
});
