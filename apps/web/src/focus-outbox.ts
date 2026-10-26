import type { FocusSessionInput } from './transport';

const PREFIX = 'learnvault-focus-outbox:';

export function focusOutboxKey(scope: string): string {
	return `${PREFIX}${encodeURIComponent(scope)}`;
}

function isFocusSession(value: unknown): value is FocusSessionInput {
	if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
	const session = value as Record<string, unknown>;
	return typeof session.id === 'string' && session.id.length > 0 &&
		(session.kind === 'focus' || session.kind === 'short' || session.kind === 'long') &&
		Number.isSafeInteger(session.plannedSeconds) && (session.plannedSeconds as number) > 0 &&
		Number.isSafeInteger(session.actualSeconds) && (session.actualSeconds as number) >= 0 &&
		(session.actualSeconds as number) <= (session.plannedSeconds as number) &&
		typeof session.completed === 'boolean' &&
		Number.isSafeInteger(session.startedAt) && (session.startedAt as number) >= 0 &&
		Number.isSafeInteger(session.endedAt) && (session.endedAt as number) >= (session.startedAt as number) &&
		(session.endedAt as number) - (session.startedAt as number) <= 7_200;
}

export function readFocusOutbox(storage: Pick<Storage, 'getItem'> | null | undefined, scope: string): FocusSessionInput[] {
	try {
		const raw = storage?.getItem(focusOutboxKey(scope));
		if (!raw) return [];
		const value: unknown = JSON.parse(raw);
		return Array.isArray(value) ? value.filter(isFocusSession) : [];
	} catch {
		return [];
	}
}

export function persistFocusOutbox(
	storage: Pick<Storage, 'setItem'> | null | undefined,
	scope: string,
	sessions: FocusSessionInput[]
): boolean {
	if (!storage) return false;
	try {
		storage.setItem(focusOutboxKey(scope), JSON.stringify(sessions));
		return true;
	} catch {
		return false;
	}
}
