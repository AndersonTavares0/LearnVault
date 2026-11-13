import { mount } from 'svelte';
import App from './App.svelte';
import './styles.css';
import './ui-components.css';
import './redesign.css';
import { applyTheme, readThemeMode, resolveTheme } from './theme';
import { applyLocale, readLocale, setLocale } from './i18n.svelte';

const storage = safeStorage();
const initialTheme = resolveTheme(readThemeMode(storage), window.matchMedia?.('(prefers-color-scheme: dark)').matches ?? false);
applyTheme(document.documentElement, initialTheme);
const initialLocale = readLocale(storage);
setLocale(initialLocale, storage);
applyLocale(document.documentElement, initialLocale);
const themeColor = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
if (themeColor) themeColor.content = initialTheme === 'dark' ? '#10151b' : '#eef1f4';

function safeStorage(): Storage | null {
	try {
		return window.localStorage;
	} catch {
		return null;
	}
}
mount(App, { target: document.getElementById('app')! });
