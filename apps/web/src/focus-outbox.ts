import type { FocusSessionInput } from './transport';

const PREFIX = 'learnvault-focus-outbox:';
const REJECTED_SUFFIX = ':rejected';

export type RejectedFocusSession = { session: FocusSessionInput; reason: string };

function rejectedKey(scope: string): string {
	return `${focusOutboxKey(scope)}${REJECTED_SUFFIX}`;
}

export function readRejectedFocusSessions(storage: Pick<Storage, 'getItem'> | null | undefined, scope: string): RejectedFocusSession[] {
	try {
		const raw = storage?.getItem(rejectedKey(scope));
		if (!raw) return [];
		const value: unknown = JSON.parse(raw);
		if (!Array.isArray(value)) return [];
		return value.flatMap((entry) => {
			if (!entry || typeof entry !== 'object' || Array.isArray(entry)) return [];
			const rejected = entry as Record<string, unknown>;
			return isFocusSession(rejected.session) && typeof rejected.reason === 'string'
				? [{ session: rejected.session, reason: rejected.reason }]
				: [];
		});
	} catch {
		return [];
	}
}

export function persistRejectedFocusSessions(storage: Pick<Storage, 'setItem'> | null | undefined, scope: string, sessions: RejectedFocusSession[]): boolean {
	if (!storage) return false;
	try {
		storage.setItem(rejectedKey(scope), JSON.stringify(sessions));
		return true;
	} catch {
		return false;
	}
}

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
		Number.isSafeInteger(session.endedAt) && (session.endedAt as number) >= (session.startedAt as number);
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
