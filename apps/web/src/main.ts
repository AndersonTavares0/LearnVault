import { mount } from 'svelte';
import App from './App.svelte';
import './graphite/base.css';
import './graphite/reference.css';
import './graphite/shell.css';
import './graphite/refinement.css';
import { applyTheme, readThemeMode, resolveTheme } from './theme';
import { applyPalette, readPalettePreferences, resolvePalette } from './palettes';

const initialTheme = resolveTheme(readThemeMode(safeStorage()), window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
applyTheme(document.documentElement, initialTheme);
applyPalette(document.documentElement, resolvePalette(readPalettePreferences(safeStorage())), initialTheme);

function safeStorage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}
mount(App, { target: document.getElementById('app')! });
