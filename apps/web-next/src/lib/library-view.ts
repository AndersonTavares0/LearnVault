export const LIBRARY_VIEW_STORAGE_KEY = 'learnvault-library-view';

export const LIBRARY_VIEWS = ['covers', 'list', 'compact', 'shelf'] as const;

export type LibraryView = (typeof LIBRARY_VIEWS)[number];

export function parseLibraryView(value: unknown): LibraryView | null {
	if (typeof value !== 'string') return null;
	return (LIBRARY_VIEWS as readonly string[]).includes(value) ? (value as LibraryView) : null;
}

export function readLibraryView(storage: Pick<Storage, 'getItem'> | null | undefined): LibraryView {
	try {
		return parseLibraryView(storage?.getItem(LIBRARY_VIEW_STORAGE_KEY)) ?? 'covers';
	} catch {
		return 'covers';
	}
}

export function persistLibraryView(
	storage: Pick<Storage, 'setItem'> | null | undefined,
	view: LibraryView
): boolean {
	if (!storage) return false;
	try {
		storage.setItem(LIBRARY_VIEW_STORAGE_KEY, view);
		return true;
	} catch {
		return false;
	}
}

export const LIBRARY_SORT_STORAGE_KEY = 'learnvault-library-sort';

export const LIBRARY_SORTS = ['recent', 'title', 'progress'] as const;

export type LibrarySort = (typeof LIBRARY_SORTS)[number];

export function parseLibrarySort(value: unknown): LibrarySort | null {
	if (typeof value !== 'string') return null;
	return (LIBRARY_SORTS as readonly string[]).includes(value) ? (value as LibrarySort) : null;
}

export function readLibrarySort(storage: Pick<Storage, 'getItem'> | null | undefined): LibrarySort {
	try {
		return parseLibrarySort(storage?.getItem(LIBRARY_SORT_STORAGE_KEY)) ?? 'recent';
	} catch {
		return 'recent';
	}
}

export function persistLibrarySort(
	storage: Pick<Storage, 'setItem'> | null | undefined,
	sort: LibrarySort
): boolean {
	if (!storage) return false;
	try {
		storage.setItem(LIBRARY_SORT_STORAGE_KEY, sort);
		return true;
	} catch {
		return false;
	}
}
