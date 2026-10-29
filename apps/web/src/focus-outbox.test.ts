import { describe, expect, it } from 'vitest';
import { focusOutboxKey, persistFocusOutbox, persistRejectedFocusSessions, readFocusOutbox, readRejectedFocusSessions } from './focus-outbox';

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
	it('preserves legacy sessions with temporal mismatches for explicit server rejection handling', () => {
		const legacy = { ...session, actualSeconds: 1_500, endedAt: 101 };
		const storage = { getItem: () => JSON.stringify([legacy]) };
		expect(readFocusOutbox(storage, 'http:user-1')).toEqual([legacy]);
	});

	it('persists rejected sessions and their reasons by account scope', () => {
		const values = new Map<string, string>();
		const storage = { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value) };
		const rejected = [{ session, reason: 'Esta sessão já foi registrada com outros dados.' }];
		expect(persistRejectedFocusSessions(storage, 'http:user-1', rejected)).toBe(true);
		expect(readRejectedFocusSessions(storage, 'http:user-1')).toEqual(rejected);
		expect(readRejectedFocusSessions(storage, 'http:user-2')).toEqual([]);
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
