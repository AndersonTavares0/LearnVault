import { describe, expect, it, vi } from 'vitest';
import { createPositionSaver } from './study';
import type { StudyProgress } from './transport';

function progress(mediaId: string, positionSeconds: number): StudyProgress {
	return { mediaId, positionSeconds, completed: false, updatedAt: null, recorded: true };
}

function deferredSave() {
	const resolvers: Array<(value: StudyProgress) => void> = [];
	const save = vi.fn().mockImplementation(() => new Promise<StudyProgress>((resolve) => { resolvers.push(resolve); }));
	return {
		save,
		resolve(value: StudyProgress) {
			const resolve = resolvers.shift();
			if (resolve) resolve(value);
		},
		pendingCount() {
			return resolvers.length;
		}
	};
}

describe('position saver', () => {
	it('saves a position and returns the recorded progress', async () => {
		const save = vi.fn().mockResolvedValue(progress('m1', 10));
		const saver = createPositionSaver(save);
		const result = await saver.request('m1', 10);
		expect(result).toEqual({ kind: 'saved', progress: progress('m1', 10) });
		expect(save).toHaveBeenCalledWith('m1', 10);
	});

	it('queues positions while a save is in flight and only keeps the latest', async () => {
		const { save, resolve } = deferredSave();
		const saver = createPositionSaver(save);

		const first = saver.request('m1', 10);
		expect(saver.saving).toBe(true);
		const second = saver.request('m1', 20);
		const third = saver.request('m1', 30);

		expect(await second).toEqual({ kind: 'queued' });
		expect(await third).toEqual({ kind: 'queued' });

		resolve(progress('m1', 10));
		await new Promise((r) => setTimeout(r, 0));
		resolve(progress('m1', 30));
		await first;

		expect(save).toHaveBeenCalledTimes(2);
		expect(save).toHaveBeenLastCalledWith('m1', 30);
	});

	it('serializes saves so only one request is in flight at a time', async () => {
		const save = vi.fn().mockResolvedValue(progress('m1', 0));
		const saver = createPositionSaver(save);
		await Promise.all([saver.request('m1', 1), saver.request('m1', 2), saver.request('m1', 3)]);
		// primeira + a última coalescida; as posições intermediárias são descartadas
		expect(save).toHaveBeenCalledTimes(2);
		expect(save).toHaveBeenLastCalledWith('m1', 3);
	});

	it('drops queued positions after reset, keeping only the in-flight request', async () => {
		const { save, resolve } = deferredSave();
		const saver = createPositionSaver(save);

		const first = saver.request('m1', 10);
		saver.request('m1', 20);
		saver.reset();
		resolve(progress('m1', 10));
		await first;

		expect(save).toHaveBeenCalledTimes(1);
		expect(save).toHaveBeenCalledWith('m1', 10);
	});

	it('surfaces save errors and remains usable for future requests', async () => {
		const save = vi.fn().mockRejectedValueOnce(new Error('network')).mockResolvedValue(progress('m1', 5));
		const saver = createPositionSaver(save);
		const first = await saver.request('m1', 10);
		expect(first).toEqual({ kind: 'error', message: 'network' });
		const second = await saver.request('m1', 5);
		expect(second).toEqual({ kind: 'saved', progress: progress('m1', 5) });
	});

	it('ignores stale results by sequence when an older response arrives late', async () => {
		// Simula uma resposta antiga que chega depois de uma nova já ter sido aplicada.
		const { save, resolve } = deferredSave();
		const saver = createPositionSaver(save);

		const first = saver.request('m1', 10);
		saver.request('m1', 20);
		resolve(progress('m1', 10));
		await new Promise((r) => setTimeout(r, 0));
		resolve(progress('m1', 20));
		await first;

		expect(save).toHaveBeenCalledTimes(2);
	});
});
