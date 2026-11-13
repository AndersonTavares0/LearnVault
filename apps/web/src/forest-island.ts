import type { ForestTree } from './forest';

export type IslandTileCoord = {
	x: number;
	y: number;
	depth: number;
};

export type PlacedIslandTree = {
	tree: ForestTree;
	row: number;
	col: number;
	isoX: number;
	isoY: number;
	depth: number;
};

export type IslandGeometry = {
	dimension: number;
	tileWidth: number;
	tileHeight: number;
	cliffHeight: number;
	minX: number;
	minY: number;
	width: number;
	height: number;
	viewBox: string;
	leftVertex: { x: number; y: number };
	rightVertex: { x: number; y: number };
	topVertex: { x: number; y: number };
	bottomVertex: { x: number; y: number };
	leftCliffPath: string;
	rightCliffPath: string;
	leftCliffShadePath: string;
	rightCliffShadePath: string;
};

/** Lado máximo da ilha (16x16 = 256 árvores); acima disso o excedente é sinalizado na UI. */
export const ISLAND_MAX_DIMENSION = 16;
export const ISLAND_MIN_DIMENSION = 7;

export function calculateGridDimension(treeCount: number): number {
	const count = Number.isFinite(treeCount) ? Math.max(0, treeCount) : 0;
	const side = Math.ceil(Math.sqrt(count));
	return Math.min(ISLAND_MAX_DIMENSION, Math.max(ISLAND_MIN_DIMENSION, side));
}

export function calculateIslandGeometry(
	dimension: number,
	tileWidth = 56,
	tileHeight = 28,
	cliffHeight = 36
): IslandGeometry {
	const dim = Math.max(3, dimension);
	const halfW = tileWidth / 2;
	const halfH = tileHeight / 2;

	const topVertex = { x: 0, y: -halfH };
	const leftVertex = { x: -dim * halfW, y: (dim - 1) * halfH };
	const rightVertex = { x: dim * halfW, y: (dim - 1) * halfH };
	const bottomVertex = { x: 0, y: dim * tileHeight - halfH };

	const leftCliffPath = `M ${leftVertex.x} ${leftVertex.y} L ${bottomVertex.x} ${bottomVertex.y} L ${bottomVertex.x} ${bottomVertex.y + cliffHeight} L ${leftVertex.x} ${leftVertex.y + cliffHeight} Z`;
	const rightCliffPath = `M ${bottomVertex.x} ${bottomVertex.y} L ${rightVertex.x} ${rightVertex.y} L ${rightVertex.x} ${rightVertex.y + cliffHeight} L ${bottomVertex.x} ${bottomVertex.y + cliffHeight} Z`;

	// Sub-facetas de sombra para profundidade rochosa 2.5D
	const midLeftX = (leftVertex.x + bottomVertex.x) / 2;
	const midLeftY = (leftVertex.y + bottomVertex.y) / 2;
	const leftCliffShadePath = `M ${midLeftX} ${midLeftY} L ${bottomVertex.x} ${bottomVertex.y} L ${bottomVertex.x} ${bottomVertex.y + cliffHeight} L ${midLeftX} ${midLeftY + cliffHeight} Z`;

	const midRightX = (bottomVertex.x + rightVertex.x) / 2;
	const midRightY = (bottomVertex.y + rightVertex.y) / 2;
	const rightCliffShadePath = `M ${bottomVertex.x} ${bottomVertex.y} L ${midRightX} ${midRightY} L ${midRightX} ${midRightY + cliffHeight} L ${bottomVertex.x} ${bottomVertex.y + cliffHeight} Z`;

	const padX = 20;
	const padTop = 38; // espaço proporcional para as copas das árvores na linha 0
	const padBottom = 16; // espaço para sombra projetada no ar

	const minX = leftVertex.x - padX;
	const maxX = rightVertex.x + padX;
	const minY = topVertex.y - padTop;
	const maxY = bottomVertex.y + cliffHeight + padBottom;

	const width = Math.ceil(maxX - minX);
	const height = Math.ceil(maxY - minY);
	const viewBox = `${minX} ${minY} ${width} ${height}`;

	return {
		dimension: dim,
		tileWidth,
		tileHeight,
		cliffHeight,
		minX,
		minY,
		width,
		height,
		viewBox,
		leftVertex,
		rightVertex,
		topVertex,
		bottomVertex,
		leftCliffPath,
		rightCliffPath,
		leftCliffShadePath,
		rightCliffShadePath
	};
}

export function gridToIso(row: number, col: number, tileWidth = 64, tileHeight = 32): IslandTileCoord {
	const x = (col - row) * (tileWidth / 2);
	const y = (col + row) * (tileHeight / 2);
	const depth = row + col;
	return { x, y, depth };
}

/**
 * Ordena células do grid pela proximidade ao centro para que
 * a floresta cresça do centro para as bordas de forma exuberante.
 */
function generateCenterBiasedSpots(dimension: number): Array<{ row: number; col: number }> {
	const center = (dimension - 1) / 2;
	const spots: Array<{ row: number; col: number; dist: number }> = [];

	for (let r = 0; r < dimension; r++) {
		for (let c = 0; c < dimension; c++) {
			const dist = Math.hypot(r - center, c - center);
			spots.push({ row: r, col: c, dist });
		}
	}

	// Ordena do centro para fora
	spots.sort((a, b) => a.dist - b.dist);
	return spots.map(({ row, col }) => ({ row, col }));
}

export function layoutTreesOnIsland(
	trees: ForestTree[],
	dimension: number,
	tileWidth = 64,
	tileHeight = 32
): PlacedIslandTree[] {
	const spots = generateCenterBiasedSpots(dimension);
	const placed: PlacedIslandTree[] = [];
	const maxTrees = Math.min(trees.length, spots.length);

	for (let i = 0; i < maxTrees; i++) {
		const tree = trees[i];
		const spot = spots[i];
		const iso = gridToIso(spot.row, spot.col, tileWidth, tileHeight);
		placed.push({
			tree,
			row: spot.row,
			col: spot.col,
			isoX: iso.x,
			isoY: iso.y,
			depth: iso.depth
		});
	}

	// Ordena pelo algoritmo do pintor: menor depth (fundo) primeiro, maior depth (frente) depois
	placed.sort((a, b) => {
		if (a.depth !== b.depth) return a.depth - b.depth;
		return a.row - b.row;
	});

	return placed;
}
