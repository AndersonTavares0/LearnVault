import { describe, expect, it } from 'vitest';
import { displayTitleFor, emptyLibraryPreferences, libraryPreferencesKey, readLibraryPreferences, writeLibraryPreferences } from './library-preferences';

describe('library preferences', () => {
	it('turns common directory names into readable course titles', () => {
		expect(displayTitleFor('Curso1')).toBe('Curso 1');
		expect(displayTitleFor('ExcelCompleto_2026')).toBe('Excel Completo 2026');
		expect(displayTitleFor('2024 - React')).toBe('React');
	});

	it('keeps saved titles and view preferences scoped to the signed-in user', () => {
		const values = new Map<string, string>();
		const storage = { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value) };
		const preferences = { ...emptyLibraryPreferences(), displayTitles: { 'course-1': 'Design Systems' }, view: 'list' as const };
		expect(writeLibraryPreferences(storage, 'user-a', preferences)).toBe(true);
		expect(readLibraryPreferences(storage, 'user-a')).toEqual(preferences);
		expect(readLibraryPreferences(storage, 'user-b')).toEqual(emptyLibraryPreferences());
		expect(libraryPreferencesKey('user-a')).not.toBe(libraryPreferencesKey('user-b'));
	});
});
