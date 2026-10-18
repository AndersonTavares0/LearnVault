import { describe, expect, it } from 'vitest';
import {
	createDefaultPlayerPreferences,
	lessonPreferenceKey,
	playerPreferencesKey,
	playerPreferencesScope,
	readPlayerPreferences,
	updateLessonPreference,
	writePlayerPreferences,
	type PlayerPreferenceStorage
} from './player-preferences';

class MemoryStorage implements PlayerPreferenceStorage {
	private readonly values = new Map<string, string>();

	getItem(key: string) { return this.values.get(key) ?? null; }
	setItem(key: string, value: string) { this.values.set(key, value); }
}

describe('player preferences', () => {
	it('starts with neutral speed and auto-next disabled', () => {
		expect(createDefaultPlayerPreferences()).toEqual({
			audioSpeed: 1,
			videoSpeed: 1,
			autoNext: false,
			lessons: {}
		});
	});

	it('uses a stable local scope for desktop and isolates HTTP users', () => {
		expect(playerPreferencesScope('desktop', 'ignored-user')).toBe('desktop-local');
		expect(playerPreferencesScope('http', 'user-a')).toBe('http-user:user-a');
		expect(playerPreferencesScope('http', '')).toBeNull();
		expect(playerPreferencesKey('http-user:user/a')).not.toBe(playerPreferencesKey('http-user:user-b'));
	});

	it('round-trips independent audio/video speeds and auto-next per identity', () => {
		const storage = new MemoryStorage();
		const preferences = {
			...createDefaultPlayerPreferences(),
			audioSpeed: 1.5 as const,
			videoSpeed: 0.75 as const,
			autoNext: true
		};

		expect(writePlayerPreferences(storage, 'http-user:user-a', preferences)).toBe(true);
		expect(readPlayerPreferences(storage, 'http-user:user-a')).toEqual(preferences);
		expect(readPlayerPreferences(storage, 'http-user:user-b')).toEqual(createDefaultPlayerPreferences());
	});

	it('stores lesson-specific media and subtitle choice, including subtitles off', () => {
		const storage = new MemoryStorage();
		const preferences = updateLessonPreference(createDefaultPlayerPreferences(), 'course-1', 'module/aula 1', {
			mediaId: 'media-2',
			subtitleId: null
		});

		expect(lessonPreferenceKey('course-1', 'module/aula 1')).toBe(JSON.stringify(['course-1', 'module/aula 1']));
		expect(preferences.lessons[lessonPreferenceKey('course-1', 'module/aula 1')]).toEqual({ mediaId: 'media-2', subtitleId: null });
		expect(preferences.lessons[lessonPreferenceKey('course-2', 'module/aula 1')]).toBeUndefined();
		expect(createDefaultPlayerPreferences().lessons).toEqual({});
		expect(writePlayerPreferences(storage, 'desktop-local', preferences)).toBe(true);
		expect(readPlayerPreferences(storage, 'desktop-local').lessons).toEqual(preferences.lessons);
	});

	it('falls back to defaults for corrupt or out-of-range stored values', () => {
		const storage = new MemoryStorage();
		storage.setItem(playerPreferencesKey('http-user:user-a'), JSON.stringify({
			audioSpeed: 3,
			videoSpeed: 'fast',
			autoNext: 'yes',
			lessons: { broken: { mediaId: 12, subtitleId: 42 } }
		}));

		expect(readPlayerPreferences(storage, 'http-user:user-a')).toEqual(createDefaultPlayerPreferences());
	});

	it('treats unavailable storage as an optional preference instead of breaking playback', () => {
		const unavailable: PlayerPreferenceStorage = {
			getItem() { throw new Error('storage disabled'); },
			setItem() { throw new Error('quota exceeded'); }
		};

		expect(readPlayerPreferences(unavailable, 'desktop-local')).toEqual(createDefaultPlayerPreferences());
		expect(writePlayerPreferences(unavailable, 'desktop-local', createDefaultPlayerPreferences())).toBe(false);
	});
});
