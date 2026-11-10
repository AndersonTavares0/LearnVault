import { applyLocale, readLocale, setLocale } from '@/lib/i18n';
import { playerPreferencesKey, playerPreferencesScope } from '@/lib/player-preferences';
import { applyPalette, applyStyle, applyTheme, persistPalette, persistStyle, persistThemeMode, readPalette, readStyle, readThemeMode, resolveTheme, type Palette, type Style, type ThemeMode } from '@/lib/theme';

export function safeStorage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}

const systemDark = () => window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false;

/** Applies theme, palette and locale to <html> before the first render, avoiding a flash. */
export function applyStoredPreferences(root: HTMLElement = document.documentElement): void {
	const storage = safeStorage();
	applyThemeMode(readThemeMode(storage), root);
	applyPalette(root, readPalette(storage));
	applyStyle(root, readStyle(storage));
	const locale = readLocale(storage, navigator.language);
	setLocale(locale, storage, root);
	applyLocale(root, locale);
	// Drop data from the removed forest feature (docs/design/redesign.md, "Gamification").
	for (const key of ['learnvault-forest-nursery', 'learnvault-forest-session-seeds']) {
		try {
			storage?.removeItem(key);
		} catch {
			// Storage can be unavailable; the keys are harmless leftovers.
		}
	}
}

export function applyThemeMode(mode: ThemeMode, root: HTMLElement = document.documentElement): void {
	const theme = resolveTheme(mode, systemDark());
	applyTheme(root, theme);
	const meta = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
	if (meta) meta.content = getComputedStyle(root).getPropertyValue('--background').trim() || (theme === 'dark' ? '#0c0a09' : '#fafaf9');
}

export function saveThemeMode(mode: ThemeMode): void {
	persistThemeMode(safeStorage(), mode);
	applyThemeMode(mode);
}

export function savePalette(palette: Palette): void {
	persistPalette(safeStorage(), palette);
	applyPalette(document.documentElement, palette);
}

export function saveStyle(style: Style): void {
	persistStyle(safeStorage(), style);
	applyStyle(document.documentElement, style);
}

/**
 * Clears what one user left in this browser (docs/design/security.md, "Shared computers"):
 * the cached avatar version and player preferences. The focus outbox is handled with the
 * Pomodoro port (#339), which must warn before dropping unsynced sessions.
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
}
