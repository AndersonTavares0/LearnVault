import { afterEach, describe, expect, it } from 'vitest';
import { clearCourseCategories, parseCourseCategories, readCourseCategories, resolveCourseCategory, setCourseCategory } from './course-categories';

afterEach(() => clearCourseCategories(null));

describe('course categories', () => {
	it('parses valid mappings and drops empty or non-string entries', () => {
		expect(parseCourseCategories({ docker: '  DevOps  ', rust: '', bad: 123 })).toEqual({ docker: 'DevOps' });
		expect(parseCourseCategories(['not-an-object'])).toEqual({});
	});

	it('reads corrupt storage safely as an empty map', () => {
		expect(readCourseCategories({ getItem: () => '{broken' })).toEqual({});
	});

	it('assigns, overrides sourceFolder, and clears a course category', () => {
		const saved = new Map<string, string>();
		const storage = {
			getItem: (k: string) => saved.get(k) ?? null,
			setItem: (k: string, v: string) => void saved.set(k, v),
			removeItem: (k: string) => void saved.delete(k)
		} as unknown as Storage;
		const course = { courseId: 'docker', path: 'docker', sourceFolder: 'Imported' };
		expect(resolveCourseCategory(course, {})).toBe('Imported');

		const assigned = setCourseCategory('docker', 'Infrastructure', storage);
		expect(resolveCourseCategory(course, assigned)).toBe('Infrastructure');

		const cleared = setCourseCategory('docker', '', storage);
		expect(resolveCourseCategory(course, cleared)).toBe('Imported');
	});
});
