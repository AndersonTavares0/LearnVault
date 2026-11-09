import { applyLocale, readLocale, setLocale } from '@/lib/i18n';
import { applyPalette, applyTheme, persistPalette, persistThemeMode, readPalette, readThemeMode, resolveTheme, type Palette, type ThemeMode } from '@/lib/theme';

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
