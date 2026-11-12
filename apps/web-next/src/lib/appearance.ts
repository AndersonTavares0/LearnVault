import { createStore } from '@/lib/store';

/**
 * Everything a learner can change about how LearnVault looks and what it shows. One small object,
 * kept in this browser (nothing is sent anywhere), applied to <html> before the first render.
 * Light is the default; dark (warm black) and "follow the system" are choices.
 */
export const APPEARANCE_STORAGE_KEY = 'learnvault-appearance';

/** Each preset has the colour used on the light page and a lighter one for the dark page. */
export const ACCENT_PRESETS = [
	{ id: 'brown', hex: '#5C4229', dark: '#E3A857' },
	{ id: 'amber', hex: '#A5540A', dark: '#F0B35A' },
	{ id: 'forest', hex: '#2F6B3F', dark: '#6FBF86' },
	{ id: 'terracotta', hex: '#B2472D', dark: '#F08A6B' },
	{ id: 'navy', hex: '#27508F', dark: '#8FB0FF' },
	{ id: 'plum', hex: '#7A3B7B', dark: '#C58BD0' },
	{ id: 'teal', hex: '#12736E', dark: '#5CC8C0' },
	{ id: 'slate', hex: '#4A4F5C', dark: '#AEB6C8' }
] as const;

export const THEMES = ['light', 'dark', 'system'] as const;

export const REWARD_COLORS = ['amber', 'accent'] as const;
export const RADII = ['square', 'soft', 'round'] as const;
export const DENSITIES = ['comfortable', 'compact'] as const;
export const TEXT_SIZES = ['small', 'medium', 'large'] as const;
export const FONTS = ['sans', 'system', 'serif'] as const;
export const SIDEBARS = ['right', 'left'] as const;
export const HOME_LAYOUTS = ['grid', 'list'] as const;

export type Appearance = {
	theme: (typeof THEMES)[number];
	accent: string;
	reward: (typeof REWARD_COLORS)[number];
	radius: (typeof RADII)[number];
	density: (typeof DENSITIES)[number];
	textSize: (typeof TEXT_SIZES)[number];
	font: (typeof FONTS)[number];
	sidebar: (typeof SIDEBARS)[number];
	homeLayout: (typeof HOME_LAYOUTS)[number];
	showContinue: boolean;
	showGoal: boolean;
	showStreak: boolean;
	showXp: boolean;
	showBadges: boolean;
};

export const DEFAULT_APPEARANCE: Appearance = {
	theme: 'light',
	accent: ACCENT_PRESETS[0].hex,
	reward: 'amber',
	radius: 'soft',
	density: 'comfortable',
	textSize: 'medium',
	font: 'sans',
	sidebar: 'right',
	homeLayout: 'grid',
	showContinue: true,
	showGoal: true,
	showStreak: true,
	showXp: true,
	showBadges: true
};

const HEX = /^#[0-9a-fA-F]{6}$/;

const pick = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T => ((allowed as readonly unknown[]).includes(value) ? (value as T) : fallback);
const flag = (value: unknown, fallback: boolean) => (typeof value === 'boolean' ? value : fallback);

/** Keeps every field valid, so a hand-edited or older saved value can never break the page. */
export function parseAppearance(value: unknown): Appearance {
	const raw = (value && typeof value === 'object' ? value : {}) as Record<string, unknown>;
	const d = DEFAULT_APPEARANCE;
	return {
		theme: pick(raw.theme, THEMES, d.theme),
		accent: typeof raw.accent === 'string' && HEX.test(raw.accent) ? raw.accent.toUpperCase() : d.accent,
		reward: pick(raw.reward, REWARD_COLORS, d.reward),
		radius: pick(raw.radius, RADII, d.radius),
		density: pick(raw.density, DENSITIES, d.density),
		textSize: pick(raw.textSize, TEXT_SIZES, d.textSize),
		font: pick(raw.font, FONTS, d.font),
		sidebar: pick(raw.sidebar, SIDEBARS, d.sidebar),
		homeLayout: pick(raw.homeLayout, HOME_LAYOUTS, d.homeLayout),
		showContinue: flag(raw.showContinue, d.showContinue),
		showGoal: flag(raw.showGoal, d.showGoal),
		showStreak: flag(raw.showStreak, d.showStreak),
		showXp: flag(raw.showXp, d.showXp),
		showBadges: flag(raw.showBadges, d.showBadges)
	};
}

const channel = (value: number) => {
	const c = value / 255;
	return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};

const rgb = (hex: string): [number, number, number] => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16)) as [number, number, number];

/** WCAG relative luminance of a #RRGGBB colour. */
export function luminance(hex: string): number {
	const [r, g, b] = rgb(hex);
	return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function contrast(a: string, b: string): number {
	const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (light + 0.05) / (dark + 0.05);
}

/**
 * Any colour the learner picks has to carry white text at 4.5:1 (WCAG 1.4.3), because buttons,
 * the progress bar and links all use it. A colour that is too light is darkened until it does.
 */
export function readableAccent(hex: string): string {
	let [r, g, b] = rgb(hex);
	const text = (x: number, y: number, z: number) => `#${[x, y, z].map((v) => Math.round(v).toString(16).padStart(2, '0')).join('')}`.toUpperCase();
	for (let step = 0; step < 40 && contrast('#FFFFFF', text(r, g, b)) < 4.5; step += 1) [r, g, b] = [r * 0.94, g * 0.94, b * 0.94];
	return text(r, g, b);
}

const store = createStore<Appearance>(DEFAULT_APPEARANCE);

export const appearanceStore = {
	get: store.get,
	subscribe: store.subscribe
};

const DARK_PANEL = '#1C1510';
const DARK_PAGE = '#0B0A09';

/** "Follow the system" resolves to what the browser says right now. */
export function resolveTheme(theme: Appearance['theme']): 'light' | 'dark' {
	if (theme !== 'system') return theme;
	return typeof window !== 'undefined' && window.matchMedia?.('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

/**
 * On the dark page the accent has to stand out from a warm black, so a preset uses its lighter
 * twin and any other colour is lightened until it reaches 4.5:1 on the raised panel colour.
 */
export function darkAccent(hex: string): string {
	const preset = ACCENT_PRESETS.find((item) => item.hex === hex.toUpperCase());
	if (preset) return preset.dark;
	let [r, g, b] = rgb(hex);
	const text = (x: number, y: number, z: number) => `#${[x, y, z].map((v) => Math.round(v).toString(16).padStart(2, '0')).join('')}`.toUpperCase();
	for (let step = 0; step < 40 && contrast(DARK_PANEL, text(r, g, b)) < 4.5; step += 1) [r, g, b] = [r + (255 - r) * 0.1, g + (255 - g) * 0.1, b + (255 - b) * 0.1];
	return text(r, g, b);
}

/** Writes the choices to <html>: colours as custom properties, the rest as data attributes the CSS reads. */
export function applyAppearance(appearance: Appearance, root: HTMLElement = document.documentElement): void {
	const dark = resolveTheme(appearance.theme) === 'dark';
	root.dataset.theme = dark ? 'dark' : 'light';
	root.style.colorScheme = dark ? 'dark' : 'light';
	if (root === document.documentElement) document.querySelector('meta[name="theme-color"]')?.setAttribute('content', dark ? DARK_PAGE : '#FBF7F1');
	const accent = dark ? darkAccent(appearance.accent) : readableAccent(appearance.accent);
	root.style.setProperty('--brand', accent);
	root.style.setProperty('--brand-fg', dark ? '#1C1209' : '#FFFFFF');
	const own = appearance.reward === 'accent';
	root.style.setProperty('--reward', own ? 'var(--brand)' : dark ? '#C08A4E' : '#A96F2E');
	root.style.setProperty('--reward-fill', own ? 'var(--brand)' : dark ? '#C08A4E' : '#A96F2E');
	root.style.setProperty('--reward-fill-fg', own ? 'var(--brand-fg)' : dark ? '#201C18' : '#FFFFFF');
	root.style.setProperty('--reward-tint', own ? 'var(--brand-tint)' : dark ? '#3A2A18' : '#F3E3CB');
	for (const key of ['radius', 'density', 'textSize', 'font', 'sidebar', 'homeLayout'] as const) root.dataset[key] = appearance[key];
}

export function readAppearance(storage: Pick<Storage, 'getItem'> | null | undefined): Appearance {
	try {
		const text = storage?.getItem(APPEARANCE_STORAGE_KEY);
		return text ? parseAppearance(JSON.parse(text)) : DEFAULT_APPEARANCE;
	} catch {
		return DEFAULT_APPEARANCE;
	}
}

export function persistAppearance(storage: Pick<Storage, 'setItem'> | null | undefined, appearance: Appearance): boolean {
	if (!storage) return false;
	try {
		storage.setItem(APPEARANCE_STORAGE_KEY, JSON.stringify(appearance));
		return true;
	} catch {
		return false;
	}
}

let watching = false;

/** Loads the saved choices, applies them and makes them the app's current value. */
export function loadAppearance(storage: Storage | null, root: HTMLElement = document.documentElement): Appearance {
	const appearance = readAppearance(storage);
	store.set(appearance);
	applyAppearance(appearance, root);
	// "Follow the system" tracks the browser's setting while the app is open.
	if (!watching && typeof window !== 'undefined' && window.matchMedia) {
		watching = true;
		window.matchMedia('(prefers-color-scheme: dark)').addEventListener?.('change', () => { if (store.get().theme === 'system') applyAppearance(store.get(), root); });
	}
	return appearance;
}

/** Changes some choices: saved, applied and published to every component that reads them. */
export function updateAppearance(patch: Partial<Appearance>, storage: Storage | null, root: HTMLElement = document.documentElement): Appearance {
	const next = parseAppearance({ ...store.get(), ...patch });
	store.set(next);
	persistAppearance(storage, next);
	applyAppearance(next, root);
	return next;
}

export function resetAppearance(storage: Storage | null, root: HTMLElement = document.documentElement): Appearance {
	store.set(DEFAULT_APPEARANCE);
	try {
		storage?.removeItem(APPEARANCE_STORAGE_KEY);
	} catch {
		// Nothing was saved if storage is unavailable.
	}
	applyAppearance(DEFAULT_APPEARANCE, root);
	return DEFAULT_APPEARANCE;
}
