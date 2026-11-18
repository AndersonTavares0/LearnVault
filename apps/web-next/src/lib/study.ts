import type { StudyProgress } from './transport';
import { tr } from './i18n';

export type SaveResult =
	| { kind: 'saved'; progress: StudyProgress }
	| { kind: 'stale' }
	| { kind: 'error'; message: string }
	| { kind: 'queued' };

/**
 * Creates a position saver that serializes requests and coalesces multiple
 * updates while a request is in flight.
 *
 * This avoids two common race conditions in the player:
 * 1. Two parallel `recordPosition` requests where the older response arrives
 *    last, overwriting the progress with a stale position.
 * 2. Several `timeupdate` events firing successive requests, creating unnecessary
 *    load on the server.
 *
 * Only one request is sent at a time. If new positions arrive during the flight,
 * only the last one is kept and sent as soon as the previous one finishes. An internal
 * numeric sequence discards stale responses in case a parallel request slips through.
 */
export function createPositionSaver(save: (mediaId: string, position: number) => Promise<StudyProgress>) {
	let saving = false;
	let queued: { mediaId: string; position: number } | null = null;
	let sequence = 0;
	let applied = 0;

	async function run(mediaId: string, position: number): Promise<SaveResult> {
		sequence += 1;
		const id = sequence;
		saving = true;
		try {
			const progress = await save(mediaId, position);
			if (id < applied) return { kind: 'stale' };
			applied = id;
			return { kind: 'saved', progress };
		} catch (cause) {
			if (id < applied) return { kind: 'stale' };
			return { kind: 'error', message: cause instanceof Error ? cause.message : tr('Error saving progress.', 'Erro ao salvar progresso.') };
		} finally {
			saving = false;
			// Always drain: a duplicate of the position just sent would otherwise stay
			// pending and later replay over a newer position.
			const next = queued;
			queued = null;
			if (next !== null) await run(next.mediaId, next.position);
		}
	}

	return {
		get saving() {
			return saving;
		},
		request(mediaId: string, position: number): Promise<SaveResult> {
			if (saving) {
				queued = { mediaId, position };
				return Promise.resolve({ kind: 'queued' });
			}
			return run(mediaId, position);
		},
		reset() {
			queued = null;
		}
	};
}
