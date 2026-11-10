import { clampSpeed, type PlaybackSpeed } from './player';
import type { TransportKind } from './transport';

const STORAGE_KEY_PREFIX = 'learnvault.player-preferences.v1:';
const MAX_STORED_BYTES = 1_048_576;
const MAX_LESSONS = 5_000;
const MAX_ID_LENGTH = 256;
const MAX_LESSON_KEY_LENGTH = 8_192;

export interface PlayerPreferenceStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

export type LessonPlayerPreference = {
	mediaId?: string;
	subtitleId?: string | null;
};

export type PlayerPreferences = {
	audioSpeed: PlaybackSpeed;
	videoSpeed: PlaybackSpeed;
	autoNext: boolean;
	lessons: Record<string, LessonPlayerPreference>;
};

export function createDefaultPlayerPreferences(): PlayerPreferences {
	return { audioSpeed: 1, videoSpeed: 1, autoNext: false, lessons: {} };
}

/** Preferências web são separadas por usuário; o desktop tem um perfil local fixo. */
export function playerPreferencesScope(kind: TransportKind, userId: string | null | undefined): string | null {
	if (kind === 'desktop') return 'desktop-local';
	const normalizedUserId = userId?.trim();
	return normalizedUserId ? `http-user:${normalizedUserId}` : null;
}

export function playerPreferencesKey(scope: string): string {
	return `${STORAGE_KEY_PREFIX}${encodeURIComponent(scope)}`;
}

/** A tupla JSON evita colisões entre IDs/caminhos que contenham separadores. */
export function lessonPreferenceKey(courseId: string, lessonPath: string): string {
	return JSON.stringify([courseId, lessonPath]);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function validIdentifier(value: unknown): value is string {
	return typeof value === 'string' && value.length > 0 && value.length <= MAX_ID_LENGTH;
}

function validLessonKey(value: string): boolean {
	if (value.length > MAX_LESSON_KEY_LENGTH) return false;
	try {
		const parsed: unknown = JSON.parse(value);
		return Array.isArray(parsed) && parsed.length === 2 &&
			typeof parsed[0] === 'string' && parsed[0].length > 0 && parsed[0].length <= MAX_ID_LENGTH &&
			typeof parsed[1] === 'string' && parsed[1].length > 0 && parsed[1].length <= MAX_LESSON_KEY_LENGTH;
	} catch {
		return false;
	}
}

function normalizeLessonPreference(value: unknown): LessonPlayerPreference | null {
	if (!isRecord(value)) return null;
	const normalized: LessonPlayerPreference = {};
	if (validIdentifier(value.mediaId)) normalized.mediaId = value.mediaId;
	if (value.subtitleId === null || validIdentifier(value.subtitleId)) normalized.subtitleId = value.subtitleId;
	return Object.keys(normalized).length ? normalized : null;
}

function normalizePreferences(value: unknown): PlayerPreferences {
	const defaults = createDefaultPlayerPreferences();
	if (!isRecord(value)) return defaults;
	const lessons: Record<string, LessonPlayerPreference> = {};
	if (isRecord(value.lessons)) {
		for (const [key, lesson] of Object.entries(value.lessons).slice(0, MAX_LESSONS)) {
			if (!validLessonKey(key)) continue;
			const normalized = normalizeLessonPreference(lesson);
			if (normalized) lessons[key] = normalized;
		}
	}
	return {
		audioSpeed: clampSpeed(typeof value.audioSpeed === 'number' ? value.audioSpeed : 1),
		videoSpeed: clampSpeed(typeof value.videoSpeed === 'number' ? value.videoSpeed : 1),
		autoNext: typeof value.autoNext === 'boolean' ? value.autoNext : false,
		lessons
	};
}

/** Storage pode estar bloqueado ou corrompido; preferências nunca impedem a reprodução. */
export function readPlayerPreferences(storage: PlayerPreferenceStorage | null, scope: string | null): PlayerPreferences {
	if (!storage || !scope) return createDefaultPlayerPreferences();
	try {
		const raw = storage.getItem(playerPreferencesKey(scope));
		if (!raw || raw.length > MAX_STORED_BYTES) return createDefaultPlayerPreferences();
		return normalizePreferences(JSON.parse(raw) as unknown);
	} catch {
		return createDefaultPlayerPreferences();
	}
}

export function writePlayerPreferences(
	storage: PlayerPreferenceStorage | null,
	scope: string | null,
	preferences: PlayerPreferences
): boolean {
	if (!storage || !scope) return false;
	try {
		const serialized = JSON.stringify(normalizePreferences(preferences));
		if (serialized.length > MAX_STORED_BYTES) return false;
		storage.setItem(playerPreferencesKey(scope), serialized);
		return true;
	} catch {
		return false;
	}
}

export function updateLessonPreference(
	preferences: PlayerPreferences,
	courseId: string,
	lessonPath: string,
	patch: LessonPlayerPreference
): PlayerPreferences {
	const key = lessonPreferenceKey(courseId, lessonPath);
	const current = preferences.lessons[key] ?? {};
	const next = normalizeLessonPreference({ ...current, ...patch });
	const lessons = { ...preferences.lessons };
	if (next) lessons[key] = next;
	else delete lessons[key];
	return { ...preferences, lessons };
}
