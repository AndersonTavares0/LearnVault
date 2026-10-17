import { describe, expect, it } from 'vitest';
import { lessonGroup, lessonSequence, neighboringLessons } from './navigation';
import type { CatalogNode } from './transport';

function lesson(title: string, path: string): CatalogNode {
	return { title, path, role: 'lesson', files: [], looseFiles: [], children: [] };
}

function container(title: string, path: string, role: string, children: CatalogNode[]): CatalogNode {
	return { title, path, role, files: [], looseFiles: [], children };
}

const rust = container('Rust', 'curso-rust', 'course', [
	container('Módulo 1', 'mod-1', 'module', [lesson('Aula 1', 'aula-1'), lesson('Aula 2', 'aula-2')]),
	container('Módulo 2', 'mod-2', 'module', [lesson('Aula 3', 'aula-3')])
]);
const go = container('Go', 'curso-go', 'course', [lesson('Aula G1', 'aula-g1')]);
const catalog = [container('TI', 'cat-ti', 'category', [rust, go]), container('Vazia', 'cat-vazia', 'category', [])];

function titles(nodes: { title: string }[]): string[] {
	return nodes.map((node) => node.title);
}

describe('lesson navigation', () => {
	it('lists every lesson once, in the tree order of the catalog', () => {
		expect(titles(lessonSequence(catalog))).toEqual(['Aula 1', 'Aula 2', 'Aula 3', 'Aula G1']);
		expect(lessonSequence([])).toEqual([]);
	});

	it('ignores content containers that hold no lesson', () => {
		expect(titles(lessonSequence([container('Vazia', 'cat-vazia', 'category', [])]))).toEqual([]);
	});

	it('walks previous and next inside the canonical order', () => {
		expect(neighboringLessons(catalog, 'aula-1')).toEqual({ previous: null, next: { title: 'Aula 2', path: 'aula-2' } });
		expect(neighboringLessons(catalog, 'aula-2')).toEqual({ previous: { title: 'Aula 1', path: 'aula-1' }, next: { title: 'Aula 3', path: 'aula-3' } });
		expect(neighboringLessons(catalog, 'aula-g1')).toEqual({ previous: { title: 'Aula 3', path: 'aula-3' }, next: null });
	});

	it('returns no neighbor for an unknown or absent lesson path', () => {
		expect(neighboringLessons(catalog, 'ausente')).toEqual({ previous: null, next: null });
		expect(neighboringLessons(catalog, null)).toEqual({ previous: null, next: null });
		expect(neighboringLessons([], 'aula-1')).toEqual({ previous: null, next: null });
	});

	it('groups a lesson with the other lessons of its course, keeping the order', () => {
		expect(lessonGroup(catalog, 'aula-1')).toEqual({ title: 'Rust', lessons: [{ title: 'Aula 1', path: 'aula-1' }, { title: 'Aula 2', path: 'aula-2' }, { title: 'Aula 3', path: 'aula-3' }] });
		expect(lessonGroup(catalog, 'aula-3')).toEqual({ title: 'Rust', lessons: [{ title: 'Aula 1', path: 'aula-1' }, { title: 'Aula 2', path: 'aula-2' }, { title: 'Aula 3', path: 'aula-3' }] });
		expect(lessonGroup(catalog, 'aula-g1')).toEqual({ title: 'Go', lessons: [{ title: 'Aula G1', path: 'aula-g1' }] });
	});

	it('falls back to the module when there is no course, and to the whole catalog otherwise', () => {
		const looseModule = [container('Soltos', 'soltos', 'module', [lesson('Aula X', 'aula-x'), lesson('Aula Y', 'aula-y')])];
		const looseCategory = [container('TI', 'cat-ti', 'category', [lesson('Aula Z', 'aula-z'), lesson('Aula W', 'aula-w')])];
		expect(lessonGroup(looseModule, 'aula-y')).toEqual({ title: 'Soltos', lessons: [{ title: 'Aula X', path: 'aula-x' }, { title: 'Aula Y', path: 'aula-y' }] });
		expect(lessonGroup(looseCategory, 'aula-z')).toEqual({ title: null, lessons: [{ title: 'Aula Z', path: 'aula-z' }, { title: 'Aula W', path: 'aula-w' }] });
	});

	it('returns an empty group for an unknown or absent lesson path', () => {
		expect(lessonGroup(catalog, 'ausente')).toEqual({ title: null, lessons: [] });
		expect(lessonGroup(catalog, null)).toEqual({ title: null, lessons: [] });
		expect(lessonGroup([], 'aula-1')).toEqual({ title: null, lessons: [] });
	});
});
