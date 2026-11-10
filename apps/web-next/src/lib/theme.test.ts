import { describe, expect, it, vi } from 'vitest';
import { applyStyle, persistStyle, readStyle, applyPalette, applyTheme, persistPalette, persistThemeMode, readPalette, readThemeMode, resolveTheme, PALETTE_STORAGE_KEY, THEME_STORAGE_KEY, type ThemeMode } from './theme';

describe('theme preferences', () => {
	it('resolves explicit modes and follows the current system preference', () => {
		expect(resolveTheme('system', false)).toBe('light');
		expect(resolveTheme('system', true)).toBe('dark');
		expect(resolveTheme('light', true)).toBe('light');
		expect(resolveTheme('dark', false)).toBe('dark');
	});

	it('reads valid saved modes and safely falls back on absent, invalid, or blocked storage', () => {
		const getItem = vi.fn((key: string) => key === THEME_STORAGE_KEY ? 'dark' : null);
		expect(readThemeMode({ getItem })).toBe('dark');
		expect(readThemeMode({ getItem: () => 'sepia' })).toBe('system');
		expect(readThemeMode({ getItem: () => { throw new Error('denied'); } })).toBe('system');
		expect(readThemeMode(null)).toBe('system');
	});

	it('persists the preference but keeps theme changes usable when persistence fails', () => {
		const setItem = vi.fn();
		expect(persistThemeMode({ setItem }, 'light')).toBe(true);
		expect(setItem).toHaveBeenCalledWith(THEME_STORAGE_KEY, 'light');
		expect(persistThemeMode({ setItem: () => { throw new Error('denied'); } }, 'dark')).toBe(false);
		expect(persistThemeMode(null, 'system')).toBe(false);
	});

	it('applies the resolved theme through the document data attribute', () => {
		const root = { dataset: {} } as HTMLElement;
		applyTheme(root, 'dark');
		expect(root.dataset.theme).toBe('dark');
	});

	it('supports only the three preference modes', () => {
		for (const mode of ['system', 'light', 'dark'] as ThemeMode[]) expect(readThemeMode({ getItem: () => mode })).toBe(mode);
	});
});

describe('palette preferences', () => {
	it('defaults to emerald and ignores unknown or unreadable values', () => {
		expect(readPalette(null)).toBe('emerald');
		expect(readPalette({ getItem: () => 'purple' })).toBe('emerald');
		expect(readPalette({ getItem: () => { throw new Error('denied'); } })).toBe('emerald');
		expect(readPalette({ getItem: () => 'indigo' })).toBe('indigo');
	});

	it('persists and applies the palette', () => {
		const setItem = vi.fn();
		expect(persistPalette({ setItem }, 'teal')).toBe(true);
		expect(setItem).toHaveBeenCalledWith(PALETTE_STORAGE_KEY, 'teal');
		expect(persistPalette(null, 'teal')).toBe(false);
		const root = { dataset: {} } as HTMLElement;
		applyPalette(root, 'teal');
		expect(root.dataset.palette).toBe('teal');
	});

	it('stores the identity style, defaults to paper and applies it to the root', () => {
		const root = document.createElement('html');
		expect(readStyle(null)).toBe('paper');
		expect(readStyle({ getItem: () => 'neon' })).toBe('paper');
		expect(readStyle({ getItem: () => 'vault' })).toBe('vault');
		expect(persistStyle({ setItem: () => { throw new Error('denied'); } }, 'quiet')).toBe(false);
		applyStyle(root, 'quiet');
		expect(root.dataset.style).toBe('quiet');
	});
});
