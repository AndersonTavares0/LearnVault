import { describe, expect, it } from 'vitest';
import { clearPosition, persistProgressOutbox, progressOutboxKey, queuePosition, readProgressOutbox, setProgressOutboxScope } from './progress-outbox';

const entry = { mediaId: 'm1', positionSeconds: 42, at: 10 };

function memoryStorage() {
	const data = new Map<string, string>();
	return {
		data,
		getItem: (key: string) => data.get(key) ?? null,
		setItem: (key: string, value: string) => { data.set(key, value); },
		removeItem: (key: string) => { data.delete(key); }
	};
}

describe('position outbox', () => {
	it('isolates unsaved positions by account scope', () => {
		expect(progressOutboxKey('http-user:u1')).not.toBe(progressOutboxKey('http-user:u2'));
		const storage = memoryStorage();
		expect(persistProgressOutbox(storage, 'http-user:u1', [entry])).toBe(true);
		expect(readProgressOutbox(storage, 'http-user:u1')).toEqual([entry]);
		expect(readProgressOutbox(storage, 'http-user:u2')).toEqual([]);
	});

	it('discards malformed entries and survives unavailable storage', () => {
		expect(readProgressOutbox({ getItem: () => JSON.stringify([entry, { ...entry, mediaId: '' }, { ...entry, at: -1 }, null]) }, 'http-user:u1')).toEqual([entry]);
		expect(readProgressOutbox(null, 'http-user:u1')).toEqual([]);
		expect(readProgressOutbox({ getItem: () => '{' }, 'http-user:u1')).toEqual([]);
	});

	it('never lets an older attempt overwrite a newer queued position', () => {
		localStorage.clear();
		setProgressOutboxScope('http-user:u1');
		queuePosition({ mediaId: 'm1', positionSeconds: 80, at: 20 });
		queuePosition({ mediaId: 'm1', positionSeconds: 10, at: 19 });
		queuePosition({ mediaId: 'm2', positionSeconds: 5, at: 1 });
		expect(readProgressOutbox(localStorage, 'http-user:u1')).toEqual(expect.arrayContaining([
			{ mediaId: 'm1', positionSeconds: 80, at: 20 },
			{ mediaId: 'm2', positionSeconds: 5, at: 1 }
		]));
		clearPosition('m1', 19);
		expect(readProgressOutbox(localStorage, 'http-user:u1')).toHaveLength(2);
		clearPosition('m1', 20);
		expect(readProgressOutbox(localStorage, 'http-user:u1')).toEqual([{ mediaId: 'm2', positionSeconds: 5, at: 1 }]);
		setProgressOutboxScope(null);
	});
});
