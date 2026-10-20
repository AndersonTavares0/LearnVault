import type { CatalogNode } from './transport';

const REORDERABLE_ROLES = new Set(['module', 'lesson']);

export function canReorderCatalogSiblings(siblings: CatalogNode[]): boolean {
	return siblings.length > 1 && siblings.every((node) => REORDERABLE_ROLES.has(node.role));
}

export function movedCatalogPaths(siblings: CatalogNode[], index: number, offset: number): string[] | null {
	if (!canReorderCatalogSiblings(siblings) || !Number.isInteger(index) || index < 0 || index >= siblings.length || (offset !== -1 && offset !== 1)) return null;
	const destination = index + offset;
	if (destination < 0 || destination >= siblings.length) return null;
	const reordered = [...siblings];
	[reordered[index], reordered[destination]] = [reordered[destination], reordered[index]];
	return reordered.map((node) => node.path);
}
