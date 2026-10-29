import { mount } from 'svelte';
import App from './App.svelte';
import './styles.css';
import './ui-components.css';
import './redesign.css';
import { applyTheme, readThemeMode, resolveTheme } from './theme';

const initialTheme = resolveTheme(readThemeMode(safeStorage()), window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
applyTheme(document.documentElement, initialTheme);
const themeColor = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
if (themeColor) themeColor.content = initialTheme === 'dark' ? '#151b1d' : '#eceeea';

function safeStorage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}
mount(App, { target: document.getElementById('app')! });
