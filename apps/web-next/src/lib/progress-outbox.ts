import { createStore } from './store';

const PREFIX = 'learnvault-progress-outbox:';

/** A position the server does not have yet; `at` orders attempts, not playback. */
export type PendingPosition = { mediaId: string; positionSeconds: number; at: number };

export function progressOutboxKey(scope: string): string {
	return `${PREFIX}${encodeURIComponent(scope)}`;
}

function isPendingPosition(value: unknown): value is PendingPosition {
	if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
	const entry = value as Record<string, unknown>;
	return typeof entry.mediaId === 'string' && entry.mediaId.length > 0 &&
		Number.isSafeInteger(entry.positionSeconds) && (entry.positionSeconds as number) >= 0 &&
		Number.isSafeInteger(entry.at) && (entry.at as number) >= 0;
}

export function readProgressOutbox(storage: Pick<Storage, 'getItem'> | null | undefined, scope: string): PendingPosition[] {
	try {
		const raw = storage?.getItem(progressOutboxKey(scope));
		if (!raw) return [];
		const value: unknown = JSON.parse(raw);
		return Array.isArray(value) ? value.filter(isPendingPosition) : [];
	} catch {
		return [];
	}
}

export function persistProgressOutbox(
	storage: Pick<Storage, 'setItem'> | null | undefined,
	scope: string,
	entries: PendingPosition[]
): boolean {
	if (!storage) return false;
	try {
		storage.setItem(progressOutboxKey(scope), JSON.stringify(entries));
		return true;
	} catch {
		return false;
	}
}

function storage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}

/** Unsaved positions of the signed-in account, so a shared browser never mixes learners. */
export const progressOutbox = createStore<PendingPosition[]>([]);
let scope: string | null = null;
let lastAttempt = 0;

/** Monotonic stamp for save attempts, so an older failure cannot replace a newer entry. */
export function nextAttemptAt(): number {
	lastAttempt = Math.max(Date.now(), lastAttempt + 1);
	return lastAttempt;
}

export function setProgressOutboxScope(next: string | null): void {
	if (scope === next) return;
	scope = next;
	const entries = next ? readProgressOutbox(storage(), next) : [];
	// Account for entries from a previous visit so a new attempt is always the newest.
	lastAttempt = Math.max(lastAttempt, ...entries.map((entry) => entry.at));
	progressOutbox.set(entries);
}

/** Keeps only the newest attempt per lesson until a save for it succeeds. */
export function queuePosition(entry: PendingPosition): void {
	progressOutbox.update((current) => {
		const existing = current.find((item) => item.mediaId === entry.mediaId);
		if (existing && existing.at >= entry.at) return current;
		const next = [entry, ...current.filter((item) => item.mediaId !== entry.mediaId)];
		if (scope) persistProgressOutbox(storage(), scope, next);
		return next;
	});
}

/** Drops the queued position once the server accepted a save at or after `at`. */
export function clearPosition(mediaId: string, at: number): void {
	progressOutbox.update((current) => {
		const entry = current.find((item) => item.mediaId === mediaId);
		if (!entry || entry.at > at) return current;
		const next = current.filter((item) => item.mediaId !== mediaId);
		if (scope) persistProgressOutbox(storage(), scope, next);
		return next;
	});
}

/** Sign-out discards unsaved positions: the next learner must not inherit them. */
export function clearProgressOutbox(): void {
	if (scope) {
		try {
			storage()?.removeItem(progressOutboxKey(scope));
		} catch {
			// Unavailable storage holds nothing to clear.
		}
	}
	scope = null;
	progressOutbox.set([]);
}
