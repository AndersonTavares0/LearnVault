export const THEME_STORAGE_KEY = 'learnvault-theme';
export const THEME_PALETTE_STORAGE_KEY = 'learnvault-palette';
export const THEME_MODES = ['system', 'light', 'dark'] as const;
export const THEME_PALETTES = ['blue-slate', 'deep-petrol', 'balanced-sage'] as const;
export type ThemeMode = (typeof THEME_MODES)[number];
export type ThemePalette = (typeof THEME_PALETTES)[number];
export type ResolvedTheme = Exclude<ThemeMode, 'system'>;

export function parseThemeMode(value: unknown): ThemeMode | null {
	if (typeof value !== 'string' || !(THEME_MODES as readonly string[]).includes(value)) return null;
	return value as ThemeMode;
}

export function parseThemePalette(value: unknown): ThemePalette | null {
	if (typeof value !== 'string' || !(THEME_PALETTES as readonly string[]).includes(value)) return null;
	return value as ThemePalette;
}

export function readThemePalette(storage: Pick<Storage, 'getItem'> | null | undefined): ThemePalette {
	try {
		return parseThemePalette(storage?.getItem(THEME_PALETTE_STORAGE_KEY)) ?? 'blue-slate';
	} catch {
		return 'blue-slate';
	}
}

export function persistThemePalette(storage: Pick<Storage, 'setItem'> | null | undefined, palette: ThemePalette): boolean {
	if (!storage) return false;
	try {
		storage.setItem(THEME_PALETTE_STORAGE_KEY, palette);
		return true;
	} catch {
		return false;
	}
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

export function applyThemePalette(root: HTMLElement, palette: ThemePalette): void {
	root.dataset.palette = palette;
}

export function applyThemeColor(meta: Pick<HTMLMetaElement, 'content'> | null, palette: ThemePalette, theme: ResolvedTheme): void {
	if (!meta) return;
	meta.content = {
		'blue-slate': { light: '#eef1f2', dark: '#10161b' },
		'deep-petrol': { light: '#edf3f1', dark: '#172326' },
		'balanced-sage': { light: '#f1f4ef', dark: '#222823' }
	}[palette][theme];
}
