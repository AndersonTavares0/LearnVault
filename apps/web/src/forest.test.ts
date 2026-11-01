import { describe, expect, it } from 'vitest';
import {
	buildForest,
	calculateForestStats,
	deriveEcosystem,
	deriveTreeStage,
	filterForest,
	formatTreeLabel,
	treeStageName
} from './forest';
import type { FocusSession } from './pomodoro';

describe('forest logic', () => {
	it('derives tree stages according to focus duration and completion', () => {
		expect(deriveTreeStage(600, false)).toBe('withered');
		expect(deriveTreeStage(1500, false)).toBe('withered');

		expect(deriveTreeStage(900, true)).toBe('sapling'); // 15 min
		expect(deriveTreeStage(1200, true)).toBe('sapling'); // 20 min
		expect(deriveTreeStage(1500, true)).toBe('oak'); // 25 min
		expect(deriveTreeStage(2400, true)).toBe('oak'); // 40 min
		expect(deriveTreeStage(3000, true)).toBe('pine'); // 50 min
		expect(deriveTreeStage(3600, true)).toBe('pine'); // 60 min
		expect(deriveTreeStage(4500, true)).toBe('ancient'); // 75 min
	});

	it('formats tree names and accessible labels', () => {
		expect(treeStageName('sapling')).toBe('Brotinho');
		expect(treeStageName('oak')).toBe('Carvalho Frondoso');
		expect(treeStageName('pine')).toBe('Pinheiro Nobre');
		expect(treeStageName('sakura')).toBe('Cerejeira em Flor');
		expect(treeStageName('willow')).toBe('Salgueiro Chorão');
		expect(treeStageName('ginkgo')).toBe('Ginkgo Dourado');
		expect(treeStageName('ancient')).toBe('Árvore Ancestral');
		expect(treeStageName('withered')).toBe('Árvore Seca');

		const label = formatTreeLabel({
			sessionId: '1',
			stage: 'oak',
			stageName: 'Carvalho Frondoso',
			minutes: 25,
			completed: true,
			timestamp: 1761926400,
			dateIso: '2025-10-31'
		});
		expect(label).toContain('Carvalho Frondoso');
		expect(label).toContain('25 min de foco cultivados');

		const witheredLabel = formatTreeLabel({
			sessionId: '2',
			stage: 'withered',
			stageName: 'Árvore Seca',
			minutes: 10,
			completed: false,
			timestamp: 1761926400,
			dateIso: '2025-10-31'
		});
		expect(witheredLabel).toContain('Árvore Seca');
		expect(witheredLabel).toContain('interrompida');
	});

	it('builds forest only from focus sessions and filters out breaks', () => {
		const sessions: FocusSession[] = [
			{
				id: 's1',
				kind: 'focus',
				plannedSeconds: 1500,
				actualSeconds: 1500,
				completed: true,
				startedAt: 100,
				endedAt: 1600
			},
			{
				id: 'b1',
				kind: 'short',
				plannedSeconds: 300,
				actualSeconds: 300,
				completed: true,
				startedAt: 1600,
				endedAt: 1900
			},
			{
				id: 's2',
				kind: 'focus',
				plannedSeconds: 1500,
				actualSeconds: 400,
				completed: false,
				startedAt: 1900,
				endedAt: 2300
			}
		];

		const forest = buildForest(sessions);
		expect(forest).toHaveLength(2);
		expect(forest[0].sessionId).toBe('s1');
		expect(forest[0].stage).toBe('oak');
		expect(forest[1].sessionId).toBe('s2');
		expect(forest[1].stage).toBe('withered');

		// With custom species seed
		const forestWithSeed = buildForest(sessions, { s1: 'sakura', s2: 'ginkgo' });
		expect(forestWithSeed[0].stage).toBe('sakura');
		expect(forestWithSeed[0].stageName).toBe('Cerejeira em Flor');
		// Uncompleted sessions remain withered regardless of seed choice
		expect(forestWithSeed[1].stage).toBe('withered');
	});

	it('calculates correct forest statistics', () => {
		const trees = [
			{
				sessionId: '1',
				stage: 'oak' as const,
				stageName: 'Carvalho',
				minutes: 25,
				completed: true,
				timestamp: 100,
				dateIso: '2025-10-31',
				label: '1'
			},
			{
				sessionId: '2',
				stage: 'pine' as const,
				stageName: 'Pinheiro',
				minutes: 50,
				completed: true,
				timestamp: 200,
				dateIso: '2025-10-31',
				label: '2'
			},
			{
				sessionId: '3',
				stage: 'withered' as const,
				stageName: 'Seca',
				minutes: 10,
				completed: false,
				timestamp: 300,
				dateIso: '2025-10-31',
				label: '3'
			}
		];

		const stats = calculateForestStats(trees);
		expect(stats.totalTrees).toBe(3);
		expect(stats.healthyTrees).toBe(2);
		expect(stats.witheredTrees).toBe(1);
		expect(stats.successRate).toBe(67);
		expect(stats.totalMinutes).toBe(75); // 25 + 50 (ignora minutos de árvores murchas)
		expect(stats.ecosystem.name).toBe('Bosque Jovem');
		expect(stats.longestSessionMinutes).toBe(50);
		expect(stats.speciesCounts.oak).toBe(1);
		expect(stats.speciesCounts.pine).toBe(1);
		expect(stats.speciesCounts.withered).toBe(1);
	});

	it('derives ecosystem progression stages based on healthy trees count', () => {
		expect(deriveEcosystem(0)).toMatchObject({
			level: 0,
			name: 'Clareira Inicial',
			treesToNext: 1,
			nextName: 'Bosque Jovem'
		});
		expect(deriveEcosystem(2)).toMatchObject({
			level: 1,
			name: 'Bosque Jovem',
			treesToNext: 3,
			nextName: 'Bosque Frondoso'
		});
		expect(deriveEcosystem(8)).toMatchObject({
			level: 2,
			name: 'Bosque Frondoso',
			treesToNext: 4,
			nextName: 'Floresta Verdejante'
		});
		expect(deriveEcosystem(15)).toMatchObject({
			level: 3,
			name: 'Floresta Verdejante',
			treesToNext: 10,
			nextName: 'Reserva Natural'
		});
		expect(deriveEcosystem(30)).toMatchObject({
			level: 4,
			name: 'Reserva Natural',
			treesToNext: 20,
			nextName: 'Santuário Ancestral'
		});
		expect(deriveEcosystem(60)).toMatchObject({
			level: 5,
			name: 'Santuário Ancestral',
			treesToNext: 0,
			nextName: null
		});
	});

	it('filters trees by time range', () => {
		const now = new Date(2025, 9, 31, 12, 0, 0); // 31/10/2025
		const nowSec = Math.floor(now.getTime() / 1000);

		const trees = [
			{
				sessionId: 'today',
				stage: 'oak' as const,
				stageName: 'Carvalho',
				minutes: 25,
				completed: true,
				timestamp: nowSec - 3600,
				dateIso: '2025-10-31',
				label: 't'
			},
			{
				sessionId: '3daysAgo',
				stage: 'oak' as const,
				stageName: 'Carvalho',
				minutes: 25,
				completed: true,
				timestamp: nowSec - 3 * 86400,
				dateIso: '2025-10-28',
				label: 'w'
			},
			{
				sessionId: '40daysAgo',
				stage: 'oak' as const,
				stageName: 'Carvalho',
				minutes: 25,
				completed: true,
				timestamp: nowSec - 40 * 86400,
				dateIso: '2025-09-21',
				label: 'm'
			}
		];

		expect(filterForest(trees, 'today', now)).toHaveLength(1);
		expect(filterForest(trees, 'week', now)).toHaveLength(2);
		expect(filterForest(trees, 'month', now)).toHaveLength(2);
		expect(filterForest(trees, 'all', now)).toHaveLength(3);
	});
});
