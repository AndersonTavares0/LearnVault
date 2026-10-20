import { mount } from 'svelte';
import App from './App.svelte';
import './styles.css';
import './reference.css';
import { applyTheme, readThemeMode, resolveTheme } from './theme';

const initialTheme = resolveTheme(readThemeMode(safeStorage()), window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
applyTheme(document.documentElement, initialTheme);
const themeColor = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
	if (themeColor) themeColor.content = initialTheme === 'dark' ? '#14232b' : '#f2f5f7';

function safeStorage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}
mount(App, { target: document.getElementById('app')! });
