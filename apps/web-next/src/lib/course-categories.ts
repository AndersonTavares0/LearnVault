import { createStore } from '@/lib/store';
import type { CatalogNode } from '@/lib/transport';

export const COURSE_CATEGORIES_STORAGE_KEY = 'learnvault-course-categories';

export type CourseCategoryMap = Record<string, string>;

const browserStorage = (): Storage | null => {
	try {
		return typeof window !== 'undefined' ? window.localStorage : null;
	} catch {
		return null;
	}
};

export function parseCourseCategories(value: unknown): CourseCategoryMap {
	if (!value || typeof value !== 'object' || Array.isArray(value)) return {};
	const next: CourseCategoryMap = {};
	for (const [id, raw] of Object.entries(value as Record<string, unknown>)) {
		if (typeof raw !== 'string') continue;
		const clean = raw.trim();
		if (id && clean) next[id] = clean;
	}
	return next;
}

export function readCourseCategories(storage: Pick<Storage, 'getItem'> | null | undefined): CourseCategoryMap {
	try {
		const raw = storage?.getItem(COURSE_CATEGORIES_STORAGE_KEY);
		return raw ? parseCourseCategories(JSON.parse(raw)) : {};
	} catch {
		return {};
	}
}

function persistCourseCategories(storage: Pick<Storage, 'setItem' | 'removeItem'> | null | undefined, map: CourseCategoryMap): void {
	if (!storage) return;
	try {
		if (Object.keys(map).length === 0) storage.removeItem(COURSE_CATEGORIES_STORAGE_KEY);
		else storage.setItem(COURSE_CATEGORIES_STORAGE_KEY, JSON.stringify(map));
	} catch {
		// Storage is optional.
	}
}

const store = createStore<CourseCategoryMap>(readCourseCategories(browserStorage()));

export const courseCategoriesStore = {
	get: store.get,
	subscribe: store.subscribe
};

export function setCourseCategory(
	courseId: string,
	category: string | null,
	storage: Pick<Storage, 'setItem' | 'removeItem'> | null = browserStorage()
): CourseCategoryMap {
	const current = { ...store.get() };
	const clean = category?.trim() ?? '';
	if (clean) current[courseId] = clean;
	else delete current[courseId];
	store.set(current);
	persistCourseCategories(storage, current);
	return current;
}

export function clearCourseCategories(storage: Pick<Storage, 'removeItem'> | null = browserStorage()): void {
	store.set({});
	try {
		storage?.removeItem(COURSE_CATEGORIES_STORAGE_KEY);
	} catch {
		// Storage is optional.
	}
}

/** Resolves a course's active category: learner-assigned category overrides imported sourceFolder. */
export function resolveCourseCategory(course: Pick<CatalogNode, 'courseId' | 'path' | 'sourceFolder'>, custom: CourseCategoryMap = {}): string | null {
	const id = course.courseId || course.path;
	const assigned = custom[id]?.trim();
	if (assigned) return assigned;
	const folder = course.sourceFolder?.trim();
	return folder || null;
}
