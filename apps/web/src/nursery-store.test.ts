import { beforeEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import {
	awardDrops,
	nursery,
	resetNurseryForTests,
	setActiveSeed,
	unlockBotanicalSpecies
} from './nursery-store';

describe('nursery-store', () => {
	beforeEach(() => {
		resetNurseryForTests({
			dropsBalance: 50,
			totalDropsEarned: 50,
			unlockedSpecies: ['sapling', 'oak'],
			activeSeed: 'oak'
		});
	});

	it('initializes with given or default state', () => {
		const state = get(nursery);
		expect(state.dropsBalance).toBe(50);
		expect(state.unlockedSpecies).toContain('oak');
		expect(state.activeSeed).toBe('oak');
	});

	it('awards drops and updates balance and total', () => {
		awardDrops(25);
		const state = get(nursery);
		expect(state.dropsBalance).toBe(75);
		expect(state.totalDropsEarned).toBe(75);
	});

	it('unlocks species when balance is sufficient', () => {
		// Pine costs 40 drops, balance is 50
		const ok = unlockBotanicalSpecies('pine');
		expect(ok).toBe(true);

		const state = get(nursery);
		expect(state.dropsBalance).toBe(10); // 50 - 40
		expect(state.unlockedSpecies).toContain('pine');
		expect(state.activeSeed).toBe('pine'); // auto-selects newly unlocked
	});

	it('fails to unlock species when balance is insufficient', () => {
		// Sakura costs 80 drops, balance is 50
		const ok = unlockBotanicalSpecies('sakura');
		expect(ok).toBe(false);

		const state = get(nursery);
		expect(state.dropsBalance).toBe(50);
		expect(state.unlockedSpecies).not.toContain('sakura');
	});

	it('sets active seed if unlocked', () => {
		const ok = setActiveSeed('sapling');
		expect(ok).toBe(true);
		expect(get(nursery).activeSeed).toBe('sapling');

		// Not unlocked
		const denied = setActiveSeed('ancient');
		expect(denied).toBe(false);
		expect(get(nursery).activeSeed).toBe('sapling');
	});
});
