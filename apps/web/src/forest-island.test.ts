import { describe, expect, it } from 'vitest';
import {
	calculateGridDimension,
	calculateIslandGeometry,
	gridToIso,
	ISLAND_MAX_DIMENSION,
	layoutTreesOnIsland,
	type IslandTileCoord
} from './forest-island';
import type { ForestTree } from './forest';

describe('forest-island', () => {
	it('calculates optimal island grid dimension based on tree count', () => {
		// Minimum 4x4 for visual beauty even with 0-5 trees
		expect(calculateGridDimension(0)).toBe(4);
		expect(calculateGridDimension(3)).toBe(4);
		expect(calculateGridDimension(12)).toBe(4); // 4x4 = 16 spots
		// Expands to 5x5 for 17-25 trees
		expect(calculateGridDimension(17)).toBe(5);
		expect(calculateGridDimension(24)).toBe(5);
		// Expands to 6x6 for 26-36 trees
		expect(calculateGridDimension(28)).toBe(6);
		// Grows as ceil(sqrt(n)) so no tree is hidden, capped at ISLAND_MAX_DIMENSION
		expect(calculateGridDimension(50)).toBe(8);
		expect(calculateGridDimension(100)).toBe(10);
		expect(calculateGridDimension(10_000)).toBe(ISLAND_MAX_DIMENSION);
	});

	it('places every tree up to the island capacity (no silent cutoff)', () => {
		const many: ForestTree[] = Array.from({ length: 120 }, (_, i) => ({
			sessionId: `s${i}`,
			stage: 'oak',
			stageName: 'Carvalho',
			minutes: 25,
			completed: true,
			timestamp: i,
			dateIso: '2026-10-31',
			label: `t${i}`
		}));
		const dim = calculateGridDimension(many.length);
		expect(layoutTreesOnIsland(many, dim)).toHaveLength(120);
	});

	it('converts row/col grid coordinates to isometric x/y pixels', () => {
		const tileW = 64;
		const tileH = 32;

		// Origin (0,0) is at top corner
		const c0 = gridToIso(0, 0, tileW, tileH);
		expect(c0.x).toBe(0);
		expect(c0.y).toBe(0);

		// (row 0, col 1) moves down-right
		const c1 = gridToIso(0, 1, tileW, tileH);
		expect(c1.x).toBe(tileW / 2); // +32
		expect(c1.y).toBe(tileH / 2); // +16

		// (row 1, col 0) moves down-left
		const c2 = gridToIso(1, 0, tileW, tileH);
		expect(c2.x).toBe(-tileW / 2); // -32
		expect(c2.y).toBe(tileH / 2); // +16

		// Depth order is strictly row + col
		expect(c0.depth).toBe(0);
		expect(c1.depth).toBe(1);
		expect(c2.depth).toBe(1);
	});

	it('distributes trees deterministically on the island grid sorted by depth', () => {
		const sampleTrees: ForestTree[] = [
			{
				sessionId: 't1',
				stage: 'oak',
				stageName: 'Carvalho',
				minutes: 25,
				completed: true,
				timestamp: 100,
				dateIso: '2026-11-01',
				label: 't1'
			},
			{
				sessionId: 't2',
				stage: 'pine',
				stageName: 'Pinheiro',
				minutes: 45,
				completed: true,
				timestamp: 200,
				dateIso: '2026-11-01',
				label: 't2'
			},
			{
				sessionId: 't3',
				stage: 'withered',
				stageName: 'Seca',
				minutes: 10,
				completed: false,
				timestamp: 300,
				dateIso: '2026-11-01',
				label: 't3'
			}
		];

		const layout = layoutTreesOnIsland(sampleTrees, 4);
		expect(layout).toHaveLength(3);

		// Each placed tree has valid tile coordinates
		for (const placed of layout) {
			expect(placed.row).toBeGreaterThanOrEqual(0);
			expect(placed.row).toBeLessThan(4);
			expect(placed.col).toBeGreaterThanOrEqual(0);
			expect(placed.col).toBeLessThan(4);
			expect(typeof placed.isoX).toBe('number');
			expect(typeof placed.isoY).toBe('number');
		}

		// Distinct positions
		const positions = new Set(layout.map((p) => `${p.row},${p.col}`));
		expect(positions.size).toBe(3);

		// Layout is sorted back-to-front by depth (row + col)
		for (let i = 0; i < layout.length - 1; i++) {
			expect(layout[i].depth).toBeLessThanOrEqual(layout[i + 1].depth);
		}
	});

	it('computes accurate island geometry and SVG paths for any dimension', () => {
		const geo4 = calculateIslandGeometry(4, 64, 32, 54);
		expect(geo4.dimension).toBe(4);
		expect(geo4.topVertex).toEqual({ x: 0, y: -16 });
		expect(geo4.leftVertex).toEqual({ x: -128, y: 48 });
		expect(geo4.rightVertex).toEqual({ x: 128, y: 48 });
		expect(geo4.bottomVertex).toEqual({ x: 0, y: 112 });

		// Verify SVG paths are closed polygons
		expect(geo4.leftCliffPath).toContain('M -128 48');
		expect(geo4.leftCliffPath).toContain('Z');
		expect(geo4.rightCliffPath).toContain('M 0 112');
		expect(geo4.rightCliffPath).toContain('Z');

		// Viewbox includes padding for tree crowns and floating cliff shadow
		expect(geo4.minX).toBeLessThan(geo4.leftVertex.x);
		expect(geo4.minY).toBeLessThan(geo4.topVertex.y - 80);
		expect(geo4.width).toBeGreaterThan(256);
		expect(geo4.height).toBeGreaterThan(150);
		expect(geo4.viewBox).toBe(`${geo4.minX} ${geo4.minY} ${geo4.width} ${geo4.height}`);

		// Dimension 5 expands proportionally
		const geo5 = calculateIslandGeometry(5, 64, 32, 54);
		expect(geo5.leftVertex.x).toBe(-160);
		expect(geo5.rightVertex.x).toBe(160);
		expect(geo5.bottomVertex.y).toBe(144);
		expect(geo5.width).toBeGreaterThan(geo4.width);
	});
});
