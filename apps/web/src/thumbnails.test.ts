import { describe, expect, it } from 'vitest';
import { selectThumbnailSource } from './thumbnails';
import type { CatalogNode } from './transport';

function lesson(title: string, files: CatalogNode['files']): CatalogNode {
	return { title, path: title, role: 'lesson', files, looseFiles: [], children: [] };
}

function file(name: string, overrides: Partial<CatalogNode['files'][number]> = {}): CatalogNode['files'][number] {
	return { name, path: name, size: 100, mediaId: name, mtimeSeconds: 1, kind: 'media', availability: 'available', ...overrides };
}

function course(children: CatalogNode[]): CatalogNode {
	return { title: 'Course', path: 'course-id', role: 'course', courseId: 'course-id', files: [], looseFiles: [], children };
}

describe('automatic thumbnail source selection', () => {
	it('uses the first eligible video in canonical depth-first lesson order', () => {
		const tree = course([
			{ title: 'Module', path: 'module', role: 'module', files: [], looseFiles: [], children: [
				lesson('Lesson 1', [file('notes.pdf'), file('audio.mp3'), file('intro.mp4')]),
				lesson('Lesson 2', [file('next.webm')])
			] }
		]);

		expect(selectThumbnailSource(tree)?.mediaId).toBe('intro.mp4');
	});

	it('skips unavailable, unverified, unsupported, or revisionless videos', () => {
		const tree = course([
			lesson('Lesson 1', [
				file('missing.mp4', { availability: 'missing' }),
				file('uncertain.mp4', { unverified: true }),
				file('old.mp4', { mtimeSeconds: undefined }),
				file('movie.mkv')
			]),
			lesson('Lesson 2', [file('valid.mp4')])
		]);

		expect(selectThumbnailSource(tree)?.mediaId).toBe('valid.mp4');
	});

	it('returns no source when the course has no eligible lesson video', () => {
		expect(selectThumbnailSource(course([lesson('Lesson', [file('sound.ogg')])]))).toBeNull();
	});
});
