import { describe, expect, it } from 'vitest';
import { canReorderCatalogSiblings, movedCatalogPaths } from './catalog-order';
import type { CatalogNode } from './transport';

function node(role: string, path: string): CatalogNode {
	return { title: path, path, role, files: [], looseFiles: [], children: [] };
}

describe('catalog ordering controls', () => {
	it('allows only multi-item sibling lists of modules or lessons', () => {
		expect(canReorderCatalogSiblings([node('module', 'm1'), node('module', 'm2')])).toBe(true);
		expect(canReorderCatalogSiblings([node('lesson', 'l1'), node('lesson', 'l2')])).toBe(true);
		expect(canReorderCatalogSiblings([node('course', 'c1'), node('course', 'c2')])).toBe(false);
		expect(canReorderCatalogSiblings([node('lesson', 'l1'), node('category', 'c1')])).toBe(false);
		expect(canReorderCatalogSiblings([node('lesson', 'l1')])).toBe(false);
	});

	it('moves one sibling by one position without mutating the source list', () => {
		const siblings = [node('lesson', 'a'), node('lesson', 'b'), node('lesson', 'c')];
		expect(movedCatalogPaths(siblings, 1, -1)).toEqual(['b', 'a', 'c']);
		expect(movedCatalogPaths(siblings, 1, 1)).toEqual(['a', 'c', 'b']);
		expect(siblings.map((item) => item.path)).toEqual(['a', 'b', 'c']);
	});

	it('does not move past either list boundary or accept invalid positions', () => {
		const siblings = [node('module', 'a'), node('module', 'b')];
		expect(movedCatalogPaths(siblings, 0, -1)).toBeNull();
		expect(movedCatalogPaths(siblings, 1, 1)).toBeNull();
		expect(movedCatalogPaths(siblings, -1, 1)).toBeNull();
		expect(movedCatalogPaths(siblings, 2, -1)).toBeNull();
		expect(movedCatalogPaths(siblings, 0, 2)).toBeNull();
	});
});
