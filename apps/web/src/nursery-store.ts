import { writable, get } from 'svelte/store';
import {
	DEFAULT_NURSERY_STATE,
	readNurseryState,
	saveNurseryState,
	selectActiveSeed,
	unlockSpecies,
	type BotanicalSpeciesId,
	type NurseryState
} from './forest-catalog';

function getStorage(): Storage | null {
	if (typeof window === 'undefined') return null;
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}

export const nursery = writable<NurseryState>(readNurseryState(getStorage()));

nursery.subscribe((state) => {
	saveNurseryState(getStorage(), state);
});

export function awardDrops(amount: number): void {
	if (typeof amount !== 'number' || !Number.isFinite(amount) || amount <= 0) return;
	const clean = Math.floor(amount);
	if (clean <= 0) return;
	nursery.update((current) => ({
		...current,
		dropsBalance: current.dropsBalance + clean,
		totalDropsEarned: current.totalDropsEarned + clean
	}));
}

export function unlockBotanicalSpecies(speciesId: BotanicalSpeciesId): boolean {
	const current = get(nursery);
	const result = unlockSpecies(current, speciesId);
	if (result.success) {
		nursery.set(result.state);
		return true;
	}
	return false;
}

export function setActiveSeed(speciesId: BotanicalSpeciesId): boolean {
	const current = get(nursery);
	const next = selectActiveSeed(current, speciesId);
	if (next.activeSeed === speciesId) {
		nursery.set(next);
		return true;
	}
	return false;
}

export function resetNurseryForTests(customState?: Partial<NurseryState>): void {
	nursery.set({
		...DEFAULT_NURSERY_STATE,
		...customState
	});
}
