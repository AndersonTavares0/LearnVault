import { describe, expect, it } from 'vitest';
import { lessonSections, lessonSequence, neighboringLessons } from './navigation';
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

	it('groups every lesson by its nearest container, in canonical order', () => {
		const sections = lessonSections(catalog);
		expect(sections.map((section) => section.title)).toEqual(['Módulo 1', 'Módulo 2', 'Go']);
		expect(titles(sections[0].lessons)).toEqual(['Aula 1', 'Aula 2']);
		expect(titles(sections[1].lessons)).toEqual(['Aula 3']);
		expect(titles(sections[2].lessons)).toEqual(['Aula G1']);
	});

	it('falls back to the course when there is no module, and to the category otherwise', () => {
		const courseOnly = [container('TI', 'cat-ti', 'category', [container('Rust', 'curso-rust', 'course', [lesson('Aula 1', 'aula-1'), lesson('Aula 2', 'aula-2')])])];
		expect(lessonSections(courseOnly).map((section) => section.title)).toEqual(['Rust']);
		const flat = [container('Java', 'cat-java', 'category', [lesson('Aula X', 'aula-x'), lesson('Aula Y', 'aula-y')])];
		expect(lessonSections(flat).map((section) => section.title)).toEqual(['Java']);
	});

	it('keeps lessons without any container in a titleless section', () => {
		expect(lessonSections([lesson('Solta 1', 'solta-1'), lesson('Solta 2', 'solta-2')])).toEqual([
			{ title: null, lessons: [{ title: 'Solta 1', path: 'solta-1' }, { title: 'Solta 2', path: 'solta-2' }] }
		]);
	});

	it('ignores containers without lessons and empty catalogs', () => {
		expect(lessonSections([container('Vazia', 'cat-vazia', 'category', [])])).toEqual([]);
		expect(lessonSections([])).toEqual([]);
	});

	it('keeps two same-named containers in different paths as separate sections', () => {
		const repeated = [
			container('TI', 'cat-ti', 'category', [container('Rust', 'curso-rust', 'course', [lesson('Aula A', 'aula-a')])]),
			container('Outra', 'cat-outra', 'category', [container('Rust', 'curso-rust-2', 'course', [lesson('Aula B', 'aula-b')])])
		];
		expect(lessonSections(repeated).map((section) => section.title)).toEqual(['Rust', 'Rust']);
		expect(lessonSections(repeated).map((section) => section.lessons[0]?.path)).toEqual(['aula-a', 'aula-b']);
	});
});
