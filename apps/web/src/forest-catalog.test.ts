import { describe, expect, it } from 'vitest';
import {
	BOTANICAL_SPECIES,
	calculateDropsEarned,
	calculateGrowthPhase,
	canUnlockSpecies,
	DEFAULT_NURSERY_STATE,
	getGuardianPhrase,
	getSpeciesById,
	parseNurseryState,
	readNurseryState,
	selectActiveSeed,
	unlockSpecies,
	type BotanicalSpeciesId,
	type NurseryState
} from './forest-catalog';

describe('forest-catalog', () => {
	describe('BOTANICAL_SPECIES', () => {
		it('includes base species unlocked by default with 0 cost', () => {
			const sapling = BOTANICAL_SPECIES.find((s) => s.id === 'sapling');
			const oak = BOTANICAL_SPECIES.find((s) => s.id === 'oak');
			expect(sapling).toBeDefined();
			expect(sapling?.cost).toBe(0);
			expect(oak).toBeDefined();
			expect(oak?.cost).toBe(0);
		});

		it('includes rare unlockable species with progressive costs', () => {
			const pine = BOTANICAL_SPECIES.find((s) => s.id === 'pine');
			const sakura = BOTANICAL_SPECIES.find((s) => s.id === 'sakura');
			const willow = BOTANICAL_SPECIES.find((s) => s.id === 'willow');
			const ginkgo = BOTANICAL_SPECIES.find((s) => s.id === 'ginkgo');
			const ancient = BOTANICAL_SPECIES.find((s) => s.id === 'ancient');

			expect(pine?.cost).toBeGreaterThan(0);
			expect(sakura?.cost).toBeGreaterThan(pine!.cost);
			expect(willow?.cost).toBeGreaterThan(sakura!.cost);
			expect(ginkgo?.cost).toBeGreaterThan(willow!.cost);
			expect(ancient?.cost).toBeGreaterThan(ginkgo!.cost);
		});

		it('finds species by id or falls back to oak', () => {
			expect(getSpeciesById('sakura').id).toBe('sakura');
			expect(getSpeciesById('unknown_xyz' as BotanicalSpeciesId).id).toBe('oak');
		});
	});

	describe('calculateDropsEarned', () => {
		it('returns 0 drops for incomplete or withered sessions', () => {
			expect(calculateDropsEarned(1500, false)).toBe(0);
			expect(calculateDropsEarned(3600, false)).toBe(0);
		});

		it('calculates drops proportionally for completed focus sessions', () => {
			// 10 minutes = 600s -> ~5 drops
			expect(calculateDropsEarned(600, true)).toBe(5);
			// 25 minutes = 1500s -> 12 drops
			expect(calculateDropsEarned(1500, true)).toBe(12);
			// 45 minutes = 2700s -> 22 drops
			expect(calculateDropsEarned(2700, true)).toBe(22);
			// 60 minutes = 3600s -> 30 drops
			expect(calculateDropsEarned(3600, true)).toBe(30);
		});

		it('guarantees at least 1 drop for short completed focus sessions', () => {
			// 2 minutes
			expect(calculateDropsEarned(120, true)).toBe(1);
		});
	});

	describe('calculateGrowthPhase', () => {
		it('returns phase 0 for beginning of timer (0% to 25% elapsed)', () => {
			// remaining 1400 of 1500 -> 100s elapsed (6.6%)
			expect(calculateGrowthPhase(1400, 1500)).toBe(0);
			// remaining 1200 of 1500 -> 300s elapsed (20%)
			expect(calculateGrowthPhase(1200, 1500)).toBe(0);
		});

		it('returns phase 1 for early stem growth (25% to 50% elapsed)', () => {
			// remaining 1000 of 1500 -> 500s elapsed (33.3%)
			expect(calculateGrowthPhase(1000, 1500)).toBe(1);
			// remaining 800 of 1500 -> 700s elapsed (46.6%)
			expect(calculateGrowthPhase(800, 1500)).toBe(1);
		});

		it('returns phase 2 for branching crown (50% to 75% elapsed)', () => {
			// remaining 600 of 1500 -> 900s elapsed (60%)
			expect(calculateGrowthPhase(600, 1500)).toBe(2);
			// remaining 400 of 1500 -> 1100s elapsed (73.3%)
			expect(calculateGrowthPhase(400, 1500)).toBe(2);
		});

		it('returns phase 3 for blooming tree (75% to 100% elapsed)', () => {
			// remaining 200 of 1500 -> 1300s elapsed (86.6%)
			expect(calculateGrowthPhase(200, 1500)).toBe(3);
			// remaining 0 of 1500 -> 100%
			expect(calculateGrowthPhase(0, 1500)).toBe(3);
		});

		it('handles edge cases safely (zero total, negative remaining)', () => {
			expect(calculateGrowthPhase(0, 0)).toBe(3);
			expect(calculateGrowthPhase(-10, 1500)).toBe(3);
		});
	});

	describe('unlockSpecies & state management', () => {
		const initial: NurseryState = {
			dropsBalance: 100,
			totalDropsEarned: 150,
			unlockedSpecies: ['sapling', 'oak'],
			activeSeed: 'oak'
		};

		it('verifies if species can be unlocked based on balance and existing ownership', () => {
			expect(canUnlockSpecies(initial.dropsBalance, 'pine', initial.unlockedSpecies)).toBe(true); // cost 40 <= 100
			expect(canUnlockSpecies(initial.dropsBalance, 'sakura', initial.unlockedSpecies)).toBe(true); // cost 80 <= 100
			expect(canUnlockSpecies(initial.dropsBalance, 'willow', initial.unlockedSpecies)).toBe(false); // cost 120 > 100
			expect(canUnlockSpecies(initial.dropsBalance, 'oak', initial.unlockedSpecies)).toBe(false); // already unlocked
		});

		it('unlocks species deducting cost and updating list', () => {
			const result = unlockSpecies(initial, 'pine');
			expect(result.success).toBe(true);
			expect(result.state.dropsBalance).toBe(60); // 100 - 40
			expect(result.state.unlockedSpecies).toContain('pine');
		});

		it('rejects unlocking if balance is insufficient', () => {
			const result = unlockSpecies(initial, 'willow');
			expect(result.success).toBe(false);
			expect(result.state.dropsBalance).toBe(100);
			expect(result.state.unlockedSpecies).not.toContain('willow');
		});

		it('selects active seed only if unlocked', () => {
			const ok = selectActiveSeed(initial, 'sapling');
			expect(ok.activeSeed).toBe('sapling');

			// Not unlocked yet
			const denied = selectActiveSeed(initial, 'ginkgo');
			expect(denied.activeSeed).toBe('oak');
		});
	});

	describe('persistence and parsing', () => {
		it('parses valid nursery state', () => {
			const raw = {
				dropsBalance: 50,
				totalDropsEarned: 100,
				unlockedSpecies: ['sapling', 'oak', 'pine'],
				activeSeed: 'pine'
			};
			const parsed = parseNurseryState(raw);
			expect(parsed.dropsBalance).toBe(50);
			expect(parsed.unlockedSpecies).toEqual(['sapling', 'oak', 'pine']);
			expect(parsed.activeSeed).toBe('pine');
		});

		it('falls back safely on corrupted data', () => {
			expect(parseNurseryState(null)).toEqual(DEFAULT_NURSERY_STATE);
			expect(parseNurseryState({ dropsBalance: 'invalid' })).toEqual(DEFAULT_NURSERY_STATE);
		});

		it('reads from mock storage', () => {
			const mockStorage = {
				getItem: () =>
					JSON.stringify({
						dropsBalance: 25,
						totalDropsEarned: 25,
						unlockedSpecies: ['sapling', 'oak'],
						activeSeed: 'sapling'
					})
			};
			const state = readNurseryState(mockStorage);
			expect(state.dropsBalance).toBe(25);
			expect(state.activeSeed).toBe('sapling');
		});
	});

	describe('getGuardianPhrase', () => {
		it('returns inspiring focus phrases according to growth phase', () => {
			const p0 = getGuardianPhrase('oak', 0);
			const p1 = getGuardianPhrase('oak', 1);
			const p2 = getGuardianPhrase('oak', 2);
			const p3 = getGuardianPhrase('oak', 3);

			expect(typeof p0).toBe('string');
			expect(p0.length).toBeGreaterThan(5);
			expect(p1.length).toBeGreaterThan(5);
			expect(p2.length).toBeGreaterThan(5);
			expect(p3.length).toBeGreaterThan(5);
		});
	});
});
