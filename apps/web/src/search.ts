import type { CatalogNode } from './transport';

/**
 * Normalizes text for accent- and case-insensitive comparison.
 *
 * Decomposes accented characters (NFD), strips the combining marks, and
 * lowercases so "Programação" and "programacao" compare equal.
 */
export function normalizeForSearch(value: string): string {
	return value.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase();
}

/**
 * Filters an already-loaded catalog tree by node title.
 *
 * - An empty or whitespace-only query returns the original nodes unchanged, so
 *   the default order is always the server-provided order.
 * - A node is kept when its own title matches or when any descendant matches;
 *   the ancestors of a match are preserved so the hierarchy stays navigable.
 * - A node whose own title matches keeps its full subtree, while a node kept
 *   only because a descendant matched is returned with its filtered children.
 * - Returns `[]` when nothing matches. Never mutates the input tree.
 *
 * This is a client-side filter over the data already returned by
 * `/api/catalog`; it is not server-side full-text search.
 */
export function filterCatalog(nodes: CatalogNode[], query: string): CatalogNode[] {
	const needle = normalizeForSearch(query.trim());
	if (!needle) return nodes;
	const filter = (list: CatalogNode[]): CatalogNode[] => {
		const kept: CatalogNode[] = [];
		for (const node of list) {
			if (normalizeForSearch(node.title).includes(needle)) {
				kept.push(node);
				continue;
			}
			const children = filter(node.children);
			if (children.length) kept.push({ ...node, children });
		}
		return kept;
	};
	return filter(nodes);
}
