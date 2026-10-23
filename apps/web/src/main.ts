import { mount } from 'svelte';
import App from './App.svelte';
import './styles.css';
import { applyTheme, applyThemeColor, applyThemePalette, readThemeMode, readThemePalette, resolveTheme } from './theme';

const initialTheme = resolveTheme(readThemeMode(safeStorage()), window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
const initialPalette = readThemePalette(safeStorage());
applyTheme(document.documentElement, initialTheme);
applyThemePalette(document.documentElement, initialPalette);
applyThemeColor(document.querySelector<HTMLMetaElement>('meta[name="theme-color"]'), initialPalette, initialTheme);

function safeStorage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}
mount(App, { target: document.getElementById('app')! });
