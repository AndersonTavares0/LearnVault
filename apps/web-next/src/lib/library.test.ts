import { describe, expect, it } from 'vitest';
import { buildLibrary } from './library';
import type { Catalog, CatalogNode, FileEntry, StudyProgress } from './transport';

const media = (id: string): FileEntry => ({ name: `${id}.mp4`, path: `${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' });
const lesson = (id: string): CatalogNode => ({ title: `Lesson ${id}`, path: `lessons/${id}`, role: 'lesson', files: [media(id)], looseFiles: [], children: [] });
const course = (id: string, lessons: string[], sourceFolder?: string): CatalogNode => ({ title: `Course ${id}`, path: id, role: 'course', courseId: id, sourceFolder, files: [], looseFiles: [], children: lessons.map(lesson) });
const progress = (mediaId: string, completed: boolean, positionSeconds = 30): StudyProgress => ({ mediaId, completed, positionSeconds, updatedAt: 1, recorded: true });

const catalog: Catalog = { complete: true, looseFiles: [], nodes: [course('rust', ['r1', 'r2'], 'Dev'), course('go', ['g1'], 'Dev'), course('art', ['a1', 'a2'], 'Design')] };

describe('buildLibrary', () => {
	it('derives progress, status and the next lesson per course', () => {
		const library = buildLibrary({ catalog, recent: [progress('r1', true), progress('a1', false)], favoriteCourseIds: new Set(['go']) });
		const rust = library.entries.find((entry) => entry.courseId === 'rust')!;
		expect(rust).toMatchObject({ status: 'in_progress', done: 1, total: 2 });
		expect(library.entries.find((entry) => entry.courseId === 'art')!.next?.path).toBe('lessons/a1');
		expect(library.entries.find((entry) => entry.courseId === 'go')!).toMatchObject({ status: 'not_started', favorite: true, next: { path: 'lessons/g1' } });
		expect(library.counts).toEqual({ all: 3, in_progress: 2, not_started: 1, completed: 0, favorites: 1 });
		expect(library.categories).toEqual(['Design', 'Dev']);
	});

	it('filters by status, favourites, category and accent-insensitive search', () => {
		const recent = [progress('g1', true)];
		const ids = (input: Partial<Parameters<typeof buildLibrary>[0]>) => buildLibrary({ catalog, recent, favoriteCourseIds: new Set(['art']), ...input }).visible.map((entry) => entry.courseId);
		expect(ids({ filter: 'completed' })).toEqual(['go']);
		expect(ids({ filter: 'favorites' })).toEqual(['art']);
		expect(ids({ category: 'Dev' })).toEqual(['rust', 'go']);
		expect(ids({ query: 'cóurse ru' })).toEqual(['rust']);
	});

	it('lists one resume card per course, newest first, skipping completed lessons', () => {
		const library = buildLibrary({ catalog, recent: [progress('r2', false), progress('r1', false), progress('g1', true), progress('a2', false)], favoriteCourseIds: new Set() });
		expect(library.resume.map((entry) => [entry.location.courseId, entry.progress.mediaId])).toEqual([['rust', 'r2'], ['art', 'a2']]);
	});

	it('records the newest study time per course', () => {
		const library = buildLibrary({
			catalog,
			recent: [
				{ ...progress('r1', false), updatedAt: 10 },
				{ ...progress('r2', false), updatedAt: 20 },
				{ ...progress('a1', true), updatedAt: 30 }
			],
			favoriteCourseIds: new Set()
		});
		expect(library.entries.find((entry) => entry.courseId === 'rust')!.lastStudiedAt).toBe(20);
		expect(library.entries.find((entry) => entry.courseId === 'art')!.lastStudiedAt).toBe(30);
		expect(library.entries.find((entry) => entry.courseId === 'go')!.lastStudiedAt).toBe(0);
	});

	it('handles a missing catalog', () => {
		expect(buildLibrary({ catalog: null, recent: [], favoriteCourseIds: new Set() }).entries).toEqual([]);
	});
});
