import { describe, expect, it } from 'vitest';
import { filterCatalog, normalizeForSearch } from './search';
import type { CatalogNode } from './transport';

function node(title: string, role = 'category', children: CatalogNode[] = []): CatalogNode {
	return { title, path: title.toLowerCase().replace(/\s+/g, '-'), role, files: [], looseFiles: [], children };
}

const tree: CatalogNode[] = [
	node('Programação', 'category', [
		node('Aulas de JavaScript', 'course', [node('Introdução', 'lesson')]),
		node('Rust', 'course', [node('Ownership', 'lesson')])
	]),
	node('Design', 'category', [node('Tipografia', 'lesson'), node('Contraste', 'lesson')])
];

describe('normalizeForSearch', () => {
	it('strips diacritics and case', () => {
		expect(normalizeForSearch('Programação')).toBe('programacao');
		expect(normalizeForSearch('PROGRAMAÇÃO')).toBe('programacao');
	});
});

describe('filterCatalog', () => {
	it('returns the original nodes for an empty or blank query', () => {
		expect(filterCatalog(tree, '')).toBe(tree);
		expect(filterCatalog(tree, '   ')).toBe(tree);
	});

	it('matches case-insensitively', () => {
		const result = filterCatalog(tree, 'javascript');
		expect(result).toHaveLength(1);
		expect(result[0].title).toBe('Programação');
		expect(result[0].children.map((child) => child.title)).toEqual(['Aulas de JavaScript']);
	});

	it('matches without the diacritics typed in the catalog', () => {
		expect(filterCatalog(tree, 'programacao').map((n) => n.title)).toEqual(['Programação']);
		expect(filterCatalog(tree, 'introducao')[0].children[0].children[0].title).toBe('Introdução');
	});

	it('preserves matching ancestors and the original order', () => {
		const result = filterCatalog(tree, 'nt');
		expect(result.map((n) => n.title)).toEqual(['Programação', 'Design']);
		expect(result[0].children.map((c) => c.title)).toEqual(['Aulas de JavaScript']);
		expect(result[0].children[0].children.map((c) => c.title)).toEqual(['Introdução']);
		expect(result[1].children.map((c) => c.title)).toEqual(['Contraste']);
	});

	it('keeps the full subtree of a node whose own title matches', () => {
		const result = filterCatalog(tree, 'aulas');
		expect(result.map((n) => n.title)).toEqual(['Programação']);
		expect(result[0].children.map((c) => c.title)).toEqual(['Aulas de JavaScript']);
		expect(result[0].children[0].children.map((c) => c.title)).toEqual(['Introdução']);
	});

	it('returns no roots when nothing matches', () => {
		expect(filterCatalog(tree, 'inexistente')).toEqual([]);
	});

	it('finds nested results and rebuilds the hierarchy around them', () => {
		const result = filterCatalog(tree, 'introdução');
		expect(result.map((n) => n.title)).toEqual(['Programação']);
		expect(result[0].children.map((c) => c.title)).toEqual(['Aulas de JavaScript']);
		expect(result[0].children[0].children.map((c) => c.title)).toEqual(['Introdução']);
	});

	it('does not mutate the input tree', () => {
		const snapshot = JSON.stringify(tree);
		filterCatalog(tree, 'ownership');
		expect(JSON.stringify(tree)).toBe(snapshot);
	});
});
