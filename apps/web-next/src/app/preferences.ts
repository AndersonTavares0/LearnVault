import { applyLocale, readLocale, setLocale } from '@/lib/i18n';
import { playerPreferencesKey, playerPreferencesScope } from '@/lib/player-preferences';
import { clearProgressOutbox } from '@/lib/progress-outbox';
import { loadAppearance } from '@/lib/appearance';

export function safeStorage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}

/** Applies the learner's appearance choices and the language to <html> before the first render, avoiding a flash. */
export function applyStoredPreferences(root: HTMLElement = document.documentElement): void {
	const storage = safeStorage();
	loadAppearance(storage, root);
	const locale = readLocale(storage);
	setLocale(locale, storage, root);
	applyLocale(root, locale);
	// Drop data from the removed forest feature (docs/design/redesign.md, "Gamification").
	for (const key of ['learnvault-forest-nursery', 'learnvault-forest-session-seeds', 'learnvault-theme', 'learnvault-palette', 'learnvault-style']) {
		try {
			storage?.removeItem(key);
		} catch {
			// Storage can be unavailable; the keys are harmless leftovers.
		}
	}
}

/**
 * Clears what one user left in this browser (docs/design/security.md, "Shared computers"):
 * the cached avatar version, player preferences and unsaved positions. The focus outbox is
 * kept for the next sign-in and handled by the Pomodoro port (#339), which warns before
 * dropping unsynced sessions.
 */
export function clearUserLocalData(userId: string, storage: Pick<Storage, 'removeItem'> | null = safeStorage()): void {
	const scope = playerPreferencesScope('http', userId);
	for (const key of [`learnvault_avatar_${userId}`, scope ? playerPreferencesKey(scope) : null]) {
		if (!key) continue;
		try {
			storage?.removeItem(key);
		} catch {
			// Unavailable storage holds nothing to clear.
		}
	}
	clearProgressOutbox();
}
