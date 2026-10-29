import { describe, expect, it } from 'vitest';
import {
	LIBRARY_VIEWS,
	LIBRARY_VIEW_STORAGE_KEY,
	parseLibraryView,
	persistLibraryView,
	readLibraryView
} from './library-view';

describe('parseLibraryView', () => {
	it('accepts every known view', () => {
		for (const view of LIBRARY_VIEWS) expect(parseLibraryView(view)).toBe(view);
	});

	it('rejects unknown, empty and non-string values', () => {
		expect(parseLibraryView('grid')).toBeNull();
		expect(parseLibraryView('')).toBeNull();
		expect(parseLibraryView(null)).toBeNull();
		expect(parseLibraryView(undefined)).toBeNull();
		expect(parseLibraryView(3)).toBeNull();
	});
});

describe('readLibraryView / persistLibraryView', () => {
	it('falls back to the course grid when storage is missing or invalid', () => {
		expect(readLibraryView(null)).toBe('covers');
		expect(readLibraryView({ getItem: () => 'invalid' })).toBe('covers');
		expect(readLibraryView({ getItem: () => null })).toBe('covers');
	});

	it('reads a persisted view', () => {
		expect(readLibraryView({ getItem: (key) => (key === LIBRARY_VIEW_STORAGE_KEY ? 'shelf' : null) })).toBe('shelf');
	});

	it('persists and reports write failures without throwing', () => {
		const writes: Array<[string, string]> = [];
		expect(persistLibraryView({ setItem: (key, value) => { writes.push([key, value]); } }, 'compact')).toBe(true);
		expect(writes).toEqual([[LIBRARY_VIEW_STORAGE_KEY, 'compact']]);
		expect(persistLibraryView(null, 'list')).toBe(false);
		expect(persistLibraryView({ setItem: () => { throw new Error('quota'); } }, 'list')).toBe(false);
	});
});
