export const THEME_STORAGE_KEY = 'learnvault-theme';
export const THEME_MODES = ['system', 'light', 'dark'] as const;
export type ThemeMode = (typeof THEME_MODES)[number];
export type ResolvedTheme = Exclude<ThemeMode, 'system'>;

export function parseThemeMode(value: unknown): ThemeMode | null {
	if (typeof value !== 'string' || !(THEME_MODES as readonly string[]).includes(value)) return null;
	return value as ThemeMode;
}

export function readThemeMode(storage: Pick<Storage, 'getItem'> | null | undefined): ThemeMode {
	try {
		return parseThemeMode(storage?.getItem(THEME_STORAGE_KEY)) ?? 'system';
	} catch {
		return 'system';
	}
}

export function persistThemeMode(storage: Pick<Storage, 'setItem'> | null | undefined, mode: ThemeMode): boolean {
	if (!storage) return false;
	try {
		storage.setItem(THEME_STORAGE_KEY, mode);
		return true;
	} catch {
		return false;
	}
}

export function resolveTheme(mode: ThemeMode, systemDark: boolean): ResolvedTheme {
	if (mode === 'system') return systemDark ? 'dark' : 'light';
	return mode;
}

export function applyTheme(root: HTMLElement, theme: ResolvedTheme): void {
	root.dataset.theme = theme;
}
