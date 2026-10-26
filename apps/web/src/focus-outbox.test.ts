import { describe, expect, it } from 'vitest';
import { focusOutboxKey, persistFocusOutbox, readFocusOutbox } from './focus-outbox';

const session = {
	id: 'focus-1',
	kind: 'focus' as const,
	plannedSeconds: 1_500,
	actualSeconds: 1_500,
	completed: true,
	startedAt: 100,
	endedAt: 1_600
};

describe('focus session outbox', () => {
	it('isolates pending sessions by transport account scope', () => {
		expect(focusOutboxKey('http:user-1')).not.toBe(focusOutboxKey('http:user-2'));
		expect(focusOutboxKey('desktop:local')).not.toBe(focusOutboxKey('http:user-1'));
	});

	it('restores valid pending sessions and discards malformed entries', () => {
		const storage = { getItem: () => JSON.stringify([session, { ...session, id: '' }, null]) };
		expect(readFocusOutbox(storage, 'http:user-1')).toEqual([session]);
	});

	it('returns an empty queue when storage is unavailable or malformed', () => {
		expect(readFocusOutbox(null, 'http:user-1')).toEqual([]);
		expect(readFocusOutbox({ getItem: () => '{' }, 'http:user-1')).toEqual([]);
	});

	it('persists retries under the matching account key and reports storage failure', () => {
		const data = new Map<string, string>();
		const storage = { setItem: (key: string, value: string) => data.set(key, value), getItem: (key: string) => data.get(key) ?? null };
		expect(persistFocusOutbox(storage, 'http:user-1', [session])).toBe(true);
		expect(readFocusOutbox(storage, 'http:user-1')).toEqual([session]);
		expect(readFocusOutbox(storage, 'http:user-2')).toEqual([]);
		expect(persistFocusOutbox({ setItem: () => { throw new Error('quota'); } }, 'http:user-1', [session])).toBe(false);
	});
});
