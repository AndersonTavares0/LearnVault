import { describe, expect, it } from 'vitest';
import {
	createDefaultUiPreferences,
	hasStoredUiPreferences,
	readUiPreferences,
	uiPreferencesKey,
	uiPreferencesScope,
	writeUiPreferences,
	type UiPreferenceStorage
} from './ui-preferences';

class MemoryStorage implements UiPreferenceStorage {
	private readonly values = new Map<string, string>();

	getItem(key: string) { return this.values.get(key) ?? null; }
	setItem(key: string, value: string) { this.values.set(key, value); }
}

describe('ui preferences', () => {
	it('starts with the sidebar and the panel expanded', () => {
		expect(createDefaultUiPreferences()).toEqual({ sidebarCollapsed: false, panelCollapsed: false });
	});

	it('uses a stable local scope for desktop and isolates HTTP users', () => {
		expect(uiPreferencesScope('desktop', 'ignored-user')).toBe('desktop-local');
		expect(uiPreferencesScope('http', 'user-a')).toBe('http-user:user-a');
		expect(uiPreferencesScope('http', '  ')).toBeNull();
		expect(uiPreferencesScope('http', null)).toBeNull();
		expect(uiPreferencesKey('http-user:user/a')).not.toBe(uiPreferencesKey('http-user:user-b'));
	});

	it('round-trips the collapsed sidebar and panel per identity', () => {
		const storage = new MemoryStorage();
		const preferences = { sidebarCollapsed: true, panelCollapsed: true };

		expect(writeUiPreferences(storage, 'http-user:user-a', preferences)).toBe(true);
		expect(readUiPreferences(storage, 'http-user:user-a')).toEqual(preferences);
		expect(readUiPreferences(storage, 'http-user:user-b')).toEqual(createDefaultUiPreferences());
		expect(uiPreferencesScope('http', '')).toBeNull();
	});

	it('reports whether the person already chose a layout', () => {
		const storage = new MemoryStorage();
		expect(hasStoredUiPreferences(storage, 'desktop-local')).toBe(false);
		writeUiPreferences(storage, 'desktop-local', createDefaultUiPreferences());
		expect(hasStoredUiPreferences(storage, 'desktop-local')).toBe(true);
		expect(hasStoredUiPreferences(storage, null)).toBe(false);
	});

	it('falls back to defaults for corrupt or out-of-range stored values', () => {
		const storage = new MemoryStorage();
		storage.setItem(uiPreferencesKey('http-user:user-a'), JSON.stringify({ sidebarCollapsed: 'yes', panelCollapsed: 1 }));
		expect(readUiPreferences(storage, 'http-user:user-a')).toEqual(createDefaultUiPreferences());

		storage.setItem(uiPreferencesKey('http-user:user-a'), '{ not json');
		expect(readUiPreferences(storage, 'http-user:user-a')).toEqual(createDefaultUiPreferences());
	});

	it('treats unavailable storage as an optional preference instead of breaking the layout', () => {
		const unavailable: UiPreferenceStorage = {
			getItem() { throw new Error('storage disabled'); },
			setItem() { throw new Error('quota exceeded'); }
		};

		expect(readUiPreferences(unavailable, 'desktop-local')).toEqual(createDefaultUiPreferences());
		expect(hasStoredUiPreferences(unavailable, 'desktop-local')).toBe(false);
		expect(writeUiPreferences(unavailable, 'desktop-local', createDefaultUiPreferences())).toBe(false);
		expect(readUiPreferences(null, 'desktop-local')).toEqual(createDefaultUiPreferences());
		expect(writeUiPreferences(null, 'desktop-local', createDefaultUiPreferences())).toBe(false);
	});
});
