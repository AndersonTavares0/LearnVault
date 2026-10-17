import { describe, expect, it } from 'vitest';
import { countCatalogFiles, countCatalogNode, formatCount, formatCourseMeta, formatLessonMeta } from './catalog';
import type { CatalogNode, FileEntry } from './transport';

function file(name: string, kind: string): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind, availability: 'available' };
}

function node(role: string, title: string, children: CatalogNode[] = [], files: FileEntry[] = []): CatalogNode {
	return { title, path: title, role, files, looseFiles: [], children };
}

const course = node('course', 'Rust', [
	node('module', 'Módulo 1', [
		node('lesson', 'Aula 1', [], [file('aula.mp4', 'media'), file('aula.srt', 'subtitle')]),
		node('lesson', 'Aula 2', [], [file('slide.pdf', 'material')])
	]),
	node('module', 'Módulo 2', [node('lesson', 'Aula 3', [], [file('aula-3.mp4', 'media'), file('aula-3.mp3', 'media')])]),
	node('lesson', 'Aula solta', [], [file('solto.bin', 'outro')])
]);

describe('countCatalogNode', () => {
	it('counts modules, lessons, videos and materials of the whole subtree', () => {
		expect(countCatalogNode(course)).toEqual({ modules: 2, lessons: 4, videos: 3, materials: 2 });
	});

	it('counts the node itself when it is a lesson and ignores unknown kinds', () => {
		expect(countCatalogNode(node('lesson', 'Aula', [], [file('a.mp4', 'media'), file('x.bin', 'outro')]))).toEqual({ modules: 0, lessons: 1, videos: 1, materials: 0 });
	});

	it('counts loose files of the node and of its children', () => {
		const category = node('category', 'TI', [node('lesson', 'Aula')], [file('nota.pdf', 'material')]);
		category.children[0].looseFiles = [file('extra.srt', 'subtitle')];
		expect(countCatalogNode(category)).toEqual({ modules: 0, lessons: 1, videos: 0, materials: 2 });
	});

	it('returns zeros for an empty node', () => {
		expect(countCatalogNode(node('category', 'Vazia'))).toEqual({ modules: 0, lessons: 0, videos: 0, materials: 0 });
	});
});

describe('countCatalogFiles', () => {
	it('counts only the files of the node itself', () => {
		const parent = node('lesson', 'Aula', [], [file('a.mp4', 'media')]);
		parent.children = [node('lesson', 'Bônus', [], [file('b.mp4', 'media'), file('b.pdf', 'material')])];
		expect(countCatalogFiles(parent)).toEqual({ videos: 1, materials: 0 });
		expect(countCatalogFiles(parent.children[0])).toEqual({ videos: 1, materials: 1 });
	});
});

describe('formatCount', () => {
	it('uses the singular for exactly one and the plural otherwise', () => {
		expect(formatCount(1, 'aula', 'aulas')).toBe('1 aula');
		expect(formatCount(2, 'aula', 'aulas')).toBe('2 aulas');
		expect(formatCount(0, 'material', 'materiais')).toBe('0 materiais');
	});
});

describe('formatCourseMeta', () => {
	it('joins every non-zero count in a stable order', () => {
		expect(formatCourseMeta({ modules: 3, lessons: 12, videos: 20, materials: 4 })).toBe('3 módulos · 12 aulas · 4 materiais');
	});

	it('omits zero parts and uses the singular for one', () => {
		expect(formatCourseMeta({ modules: 1, lessons: 1, videos: 0, materials: 1 })).toBe('1 módulo · 1 aula · 1 material');
		expect(formatCourseMeta({ modules: 0, lessons: 25, videos: 25, materials: 180 })).toBe('25 aulas · 180 materiais');
	});

	it('returns an empty string when there is nothing to count', () => {
		expect(formatCourseMeta({ modules: 0, lessons: 0, videos: 0, materials: 0 })).toBe('');
	});
});

describe('formatLessonMeta', () => {
	it('shows videos and materials with correct plurals', () => {
		expect(formatLessonMeta({ videos: 2, materials: 3 })).toBe('2 vídeos · 3 materiais');
		expect(formatLessonMeta({ videos: 1, materials: 1 })).toBe('1 vídeo · 1 material');
	});

	it('omits zero parts and returns an empty string when silent', () => {
		expect(formatLessonMeta({ videos: 0, materials: 5 })).toBe('5 materiais');
		expect(formatLessonMeta({ videos: 4, materials: 0 })).toBe('4 vídeos');
		expect(formatLessonMeta({ videos: 0, materials: 0 })).toBe('');
	});
});
