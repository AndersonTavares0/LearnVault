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

export const PALETTE_STORAGE_KEY = 'learnvault-palette';
// The first entry is the default (docs/design/redesign.md, "Color").
export const PALETTES = ['emerald', 'teal', 'indigo'] as const;
export type Palette = (typeof PALETTES)[number];

export function readPalette(storage: Pick<Storage, 'getItem'> | null | undefined): Palette {
	try {
		const value = storage?.getItem(PALETTE_STORAGE_KEY);
		return (PALETTES as readonly unknown[]).includes(value) ? (value as Palette) : PALETTES[0];
	} catch {
		return PALETTES[0];
	}
}

export function persistPalette(storage: Pick<Storage, 'setItem'> | null | undefined, palette: Palette): boolean {
	try {
		storage?.setItem(PALETTE_STORAGE_KEY, palette);
		return Boolean(storage);
	} catch {
		return false;
	}
}

export function applyPalette(root: HTMLElement, palette: Palette): void {
	root.dataset.palette = palette;
}

export const STYLE_STORAGE_KEY = 'learnvault-style';
// Identity direction (docs/design/redesign.md): the first entry is the default.
export const STYLES = ['paper', 'quiet', 'vault'] as const;
export type Style = (typeof STYLES)[number];

export function readStyle(storage: Pick<Storage, 'getItem'> | null | undefined): Style {
	try {
		const value = storage?.getItem(STYLE_STORAGE_KEY);
		return (STYLES as readonly unknown[]).includes(value) ? (value as Style) : STYLES[0];
	} catch {
		return STYLES[0];
	}
}

export function persistStyle(storage: Pick<Storage, 'setItem'> | null | undefined, style: Style): boolean {
	try {
		storage?.setItem(STYLE_STORAGE_KEY, style);
		return Boolean(storage);
	} catch {
		return false;
	}
}

export function applyStyle(root: HTMLElement, style: Style): void {
	root.dataset.style = style;
}
