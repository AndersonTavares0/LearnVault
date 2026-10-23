import { describe, expect, it, vi } from 'vitest';
import { applyTheme, applyThemeColor, applyThemePalette, persistThemeMode, persistThemePalette, readThemeMode, readThemePalette, resolveTheme, THEME_PALETTE_STORAGE_KEY, THEME_STORAGE_KEY, type ThemeMode, type ThemePalette } from './theme';

describe('theme preferences', () => {
	it('defaults to Blue-slate and validates saved palettes', () => {
		const getItem = vi.fn((key: string) => key === THEME_PALETTE_STORAGE_KEY ? 'deep-petrol' : null);
		expect(readThemePalette(null)).toBe('blue-slate');
		expect(readThemePalette({ getItem })).toBe('deep-petrol');
		expect(readThemePalette({ getItem: () => 'unknown' })).toBe('blue-slate');
		expect(readThemePalette({ getItem: () => { throw new Error('denied'); } })).toBe('blue-slate');
	});

	it('persists and applies palette changes safely', () => {
		const setItem = vi.fn();
		const root = { dataset: {} } as HTMLElement;
		expect(persistThemePalette({ setItem }, 'balanced-sage')).toBe(true);
		expect(setItem).toHaveBeenCalledWith(THEME_PALETTE_STORAGE_KEY, 'balanced-sage');
		expect(persistThemePalette({ setItem: () => { throw new Error('denied'); } }, 'blue-slate')).toBe(false);
		expect(persistThemePalette(null, 'blue-slate')).toBe(false);
		applyThemePalette(root, 'deep-petrol');
		expect(root.dataset.palette).toBe('deep-petrol');
	});

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

	it('keeps browser chrome color in sync with the active palette and mode', () => {
		const meta = { content: '' };
		applyThemeColor(meta, 'blue-slate', 'dark');
		expect(meta.content).toBe('#10161b');
		applyThemeColor(meta, 'deep-petrol', 'light');
		expect(meta.content).toBe('#edf3f1');
		applyThemeColor(meta, 'balanced-sage', 'dark');
		expect(meta.content).toBe('#222823');
		applyThemeColor(null, 'blue-slate', 'light');
	});

	it('supports only the three preference modes', () => {
		for (const mode of ['system', 'light', 'dark'] as ThemeMode[]) expect(readThemeMode({ getItem: () => mode })).toBe(mode);
	});
});
