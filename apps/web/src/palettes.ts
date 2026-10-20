import type { ResolvedTheme } from './theme';

export const PALETTE_STORAGE_KEY = 'learnvault-palettes.v1';
export const DEFAULT_PALETTE_ID = 'stone-olive';

export const PALETTE_COLOR_FIELDS = [
	'background',
	'surface',
	'surfaceRaised',
	'input',
	'text',
	'muted',
	'border',
	'borderStrong',
	'accent',
	'onAccent',
	'navigation',
	'onNavigation',
	'danger',
	'warning'
] as const;

export type PaletteColors = {
	background: string;
	surface: string;
	surfaceRaised: string;
	input: string;
	text: string;
	muted: string;
	border: string;
	borderStrong: string;
	accent: string;
	onAccent: string;
	navigation: string;
	onNavigation: string;
	danger: string;
	warning: string;
};

export type Palette = {
	id: string;
	name: string;
	light: PaletteColors;
	dark: PaletteColors;
};

export type PalettePreferences = {
	version: 1;
	selectedPaletteId: string;
	customPalettes: Palette[];
};

const stoneOlive = {
	id: 'stone-olive',
	name: 'Pedra, areia e oliva',
	light: {
		background: '#F6F5F1',
		surface: '#FFFFFF',
		surfaceRaised: '#EFEEE8',
		input: '#FFFFFF',
		text: '#262A25',
		muted: '#666B61',
		border: '#DADCD3',
		borderStrong: '#7E8974',
		accent: '#526B42',
		onAccent: '#FFFFFF',
		navigation: '#262D23',
		onNavigation: '#F0F1EB',
		danger: '#9C392F',
		warning: '#805B19'
	},
	dark: {
		background: '#191B18',
		surface: '#242821',
		surfaceRaised: '#30372B',
		input: '#1D211B',
		text: '#F0F1EB',
		muted: '#B8BDB0',
		border: '#444E3E',
		borderStrong: '#7A8471',
		accent: '#B5CD8B',
		onAccent: '#192113',
		navigation: '#121510',
		onNavigation: '#F0F1EB',
		danger: '#FFB4A7',
		warning: '#E5C47A'
	}
} as const satisfies Palette;

const graphite = {
	id: 'graphite',
	name: 'Graphite clássico',
	light: {
		background: '#F2F5F7',
		surface: '#FFFFFF',
		surfaceRaised: '#F7F9FA',
		input: '#FFFFFF',
		text: '#1B2832',
		muted: '#60707A',
		border: '#D9E1E6',
		borderStrong: '#7D8B94',
		accent: '#276F73',
		onAccent: '#FFFFFF',
		navigation: '#14232B',
		onNavigation: '#EDF4F6',
		danger: '#A23F32',
		warning: '#8A501A'
	},
	dark: {
		background: '#101B21',
		surface: '#192A32',
		surfaceRaised: '#203640',
		input: '#14242B',
		text: '#EDF4F5',
		muted: '#B8C8CD',
		border: '#334B54',
		borderStrong: '#7B929B',
		accent: '#79C8BF',
		onAccent: '#102328',
		navigation: '#0D181E',
		onNavigation: '#EDF4F6',
		danger: '#F19A8C',
		warning: '#E3B16F'
	}
} as const satisfies Palette;

export const PALETTE_PRESETS: readonly Palette[] = Object.freeze([
	deepFreeze(stoneOlive),
	deepFreeze(graphite)
]);

const DEFAULT_PREFERENCES: PalettePreferences = {
	version: 1,
	selectedPaletteId: DEFAULT_PALETTE_ID,
	customPalettes: []
};

const PALETTE_VARIABLE_NAMES = [
	'--bg', '--surface', '--surface-raised', '--surface-subtle', '--input', '--ink', '--muted',
	'--line', '--line-strong', '--accent', '--accent-strong', '--accent-soft', '--on-accent',
	'--nav', '--nav-elevated', '--on-nav', '--danger', '--danger-soft', '--amber', '--amber-soft',
	'--focus', '--surface-accent', '--page', '--text', '--subtle', '--heading', '--hover',
	'--input-border', '--error', '--warning', '--warning-soft', '--nav2',
	'--login-bg', '--login-surface', '--login-ink', '--login-muted', '--login-line',
	'--login-input-border', '--login-accent', '--login-navigation', '--login-on-navigation',
	'--login-on-accent', '--login-danger'
] as const;

const HEX_COLOR = /^#[\da-fA-F]{6}$/;
const NAME_CONTROL = /\p{Cc}/u;
const CONTRAST_PAIRS = [
	{ foreground: 'text', background: 'background', minimum: 4.5 },
	{ foreground: 'text', background: 'surface', minimum: 4.5 },
	{ foreground: 'text', background: 'surfaceRaised', minimum: 4.5 },
	{ foreground: 'text', background: 'input', minimum: 4.5 },
	{ foreground: 'muted', background: 'background', minimum: 4.5 },
	{ foreground: 'muted', background: 'surface', minimum: 4.5 },
	{ foreground: 'muted', background: 'surfaceRaised', minimum: 4.5 },
	{ foreground: 'muted', background: 'input', minimum: 4.5 },
	{ foreground: 'onAccent', background: 'accent', minimum: 4.5 },
	{ foreground: 'onNavigation', background: 'navigation', minimum: 4.5 },
	{ foreground: 'danger', background: 'background', minimum: 4.5 },
	{ foreground: 'danger', background: 'surface', minimum: 4.5 },
	{ foreground: 'warning', background: 'background', minimum: 4.5 },
	{ foreground: 'warning', background: 'surface', minimum: 4.5 },
	{ foreground: 'borderStrong', background: 'input', minimum: 3 },
	{ foreground: 'borderStrong', background: 'surface', minimum: 3 },
	{ foreground: 'accent', background: 'background', minimum: 3 },
	{ foreground: 'accent', background: 'surface', minimum: 3 },
	{ foreground: 'accent', background: 'input', minimum: 3 }
] as const;

export function readPalettePreferences(
	storage: Pick<Storage, 'getItem'> | null | undefined
): PalettePreferences {
	if (!storage) return DEFAULT_PREFERENCES;
	try {
		const raw = storage.getItem(PALETTE_STORAGE_KEY);
		if (!raw) return DEFAULT_PREFERENCES;
		const value: unknown = JSON.parse(raw);
		return parsePreferences(value);
	} catch {
		return DEFAULT_PREFERENCES;
	}
}

export function persistPalettePreferences(
	storage: Pick<Storage, 'setItem'> | null | undefined,
	value: PalettePreferences
): boolean {
	if (!storage) return false;
	try {
		const sanitized = parsePreferences(value);
		if (!isRecord(value) || value.version !== 1) return false;
		storage.setItem(PALETTE_STORAGE_KEY, JSON.stringify(sanitized));
		return true;
	} catch {
		return false;
	}
}

export function resolvePalette(value: PalettePreferences): Palette {
	const preferences = parsePreferences(value);
	return PALETTE_PRESETS.find((palette) => palette.id === preferences.selectedPaletteId)
		?? preferences.customPalettes.find((palette) => palette.id === preferences.selectedPaletteId)
		?? PALETTE_PRESETS[0];
}

export function validatePalette(palette: Palette): { field: string; message: string }[] {
	const errors: { field: string; message: string }[] = [];
	if (!palette || typeof palette !== 'object') {
		return [{ field: 'palette', message: 'A paleta é inválida.' }];
	}

	const name = typeof palette.name === 'string' ? palette.name.trim() : '';
	const nameLength = Array.from(name).length;
	if (nameLength < 1 || nameLength > 40 || NAME_CONTROL.test(name)) {
		errors.push({ field: 'name', message: 'Informe um nome de 1 a 40 caracteres, sem caracteres de controle.' });
	}

	for (const mode of ['light', 'dark'] as const) {
		const colors = palette[mode] as PaletteColors | undefined;
		for (const field of PALETTE_COLOR_FIELDS) {
			const value = colors?.[field];
			if (typeof value !== 'string' || !HEX_COLOR.test(value)) {
				errors.push({ field: `${mode}.${field}`, message: 'Use uma cor hexadecimal no formato #RRGGBB.' });
			}
		}

		for (const pair of CONTRAST_PAIRS) {
			const foreground = colors?.[pair.foreground];
			const background = colors?.[pair.background];
			if (typeof foreground !== 'string' || !HEX_COLOR.test(foreground)
				|| typeof background !== 'string' || !HEX_COLOR.test(background)) continue;
			if (contrastRatio(foreground, background) + 1e-10 < pair.minimum) {
				const field = `${mode}.${pair.foreground}`;
				const minimum = pair.minimum === 4.5 ? '4,5:1' : '3:1';
				errors.push({
					field,
					message: `Contraste insuficiente entre ${pair.foreground} e ${pair.background}; mínimo ${minimum}.`
				});
			}
		}
	}
	return errors;
}

export function paletteVariables(palette: Palette, theme: ResolvedTheme): Record<string, string> {
	const colors = palette?.[theme];
	const loginColors = palette?.light;
	const loginNavigation = palette?.dark;
	if (!colors || !loginColors || !loginNavigation) return {};
	const c = (field: keyof PaletteColors) => normalizedHex(colors[field]);
	const login = (field: keyof PaletteColors) => normalizedHex(loginColors[field]);
	const loginNav = (field: keyof PaletteColors) => normalizedHex(loginNavigation[field]);
	const vars: Record<(typeof PALETTE_VARIABLE_NAMES)[number], string> = {
		'--bg': c('background'),
		'--surface': c('surface'),
		'--surface-raised': c('surfaceRaised'),
		'--surface-subtle': 'var(--surface-raised)',
		'--input': c('input'),
		'--ink': c('text'),
		'--muted': c('muted'),
		'--line': c('border'),
		'--line-strong': c('borderStrong'),
		'--accent': c('accent'),
		'--accent-strong': 'var(--accent)',
		'--accent-soft': mix(c('accent'), c('surface'), 0.12),
		'--on-accent': c('onAccent'),
		'--nav': c('navigation'),
		'--nav-elevated': 'var(--nav)',
		'--on-nav': c('onNavigation'),
		'--danger': c('danger'),
		'--danger-soft': mix(c('danger'), c('surface'), 0.12),
		'--amber': c('warning'),
		'--amber-soft': mix(c('warning'), c('surface'), 0.12),
		'--focus': 'var(--accent)',
		'--surface-accent': 'var(--accent-soft)',
		'--page': 'var(--bg)',
		'--text': 'var(--ink)',
		'--subtle': 'var(--muted)',
		'--heading': 'var(--ink)',
		'--hover': 'var(--accent-soft)',
		'--input-border': 'var(--line-strong)',
		'--error': 'var(--danger)',
		'--warning': 'var(--amber)',
		'--warning-soft': 'var(--amber-soft)',
		'--nav2': 'var(--nav-elevated)',
		'--login-bg': login('background'),
		'--login-surface': login('surface'),
		'--login-ink': login('text'),
		'--login-muted': login('muted'),
		'--login-line': login('border'),
		'--login-input-border': login('borderStrong'),
		'--login-accent': login('accent'),
		'--login-navigation': loginNav('navigation'),
		'--login-on-navigation': loginNav('onNavigation'),
		'--login-on-accent': login('onAccent'),
		'--login-danger': login('danger')
	};
	return vars;
}

export function applyPalette(root: HTMLElement, palette: Palette, theme: ResolvedTheme): void {
	const colors = palette?.[theme];
	if (!colors || !root?.style) return;
	const variables = paletteVariables(palette, theme);
	for (const name of PALETTE_VARIABLE_NAMES) root.style.setProperty(name, variables[name]);
	root.dataset.palette = palette.id;
	const document = root.ownerDocument;
	let meta = document?.querySelector<HTMLMetaElement>('meta[name="theme-color"]');
	if (!meta && document) {
		meta = document.createElement('meta');
		meta.name = 'theme-color';
		document.head.append(meta);
	}
	meta?.setAttribute('content', normalizedHex(colors.background));
}

function parsePreferences(value: unknown): PalettePreferences {
	if (!isRecord(value) || value.version !== 1 || !Array.isArray(value.customPalettes)) return DEFAULT_PREFERENCES;
	const inputCustomPalettes: unknown[] = value.customPalettes;
	const customPalettes: Palette[] = [];
	const ids = new Set(PALETTE_PRESETS.map(({ id }) => id));
	const names = new Set(PALETTE_PRESETS.map(({ name }) => name.toLowerCase()));
	for (const candidate of inputCustomPalettes) {
		const palette = normalizePalette(candidate);
		if (!palette || !palette.id.startsWith('custom-') || palette.id.length <= 'custom-'.length || ids.has(palette.id)) continue;
		const key = palette.name.toLowerCase();
		if (names.has(key) || validatePalette(palette).length > 0) continue;
		ids.add(palette.id);
		names.add(key);
		customPalettes.push(deepFreeze(palette));
	}
	const selectedPaletteId = typeof value.selectedPaletteId === 'string'
		&& (PALETTE_PRESETS.some(({ id }) => id === value.selectedPaletteId)
			|| customPalettes.some(({ id }) => id === value.selectedPaletteId))
		? value.selectedPaletteId
		: DEFAULT_PALETTE_ID;
	return Object.freeze({ version: 1, selectedPaletteId, customPalettes: Object.freeze(customPalettes) }) as PalettePreferences;
}

function normalizePalette(value: unknown): Palette | null {
	if (!isRecord(value) || typeof value.id !== 'string' || typeof value.name !== 'string') return null;
	const name = value.name.trim();
	if (Array.from(name).length < 1 || Array.from(name).length > 40 || NAME_CONTROL.test(name)) return null;
	const light = normalizeColors(value.light);
	const dark = normalizeColors(value.dark);
	if (!light || !dark) return null;
	return { id: value.id, name, light, dark };
}

function normalizeColors(value: unknown): PaletteColors | null {
	if (!isRecord(value)) return null;
	const colors: Record<string, string> = {};
	for (const field of PALETTE_COLOR_FIELDS) {
		const color = value[field];
		if (typeof color !== 'string' || !HEX_COLOR.test(color)) return null;
		colors[field] = color.toUpperCase();
	}
	return colors as unknown as PaletteColors;
}

function contrastRatio(first: string, second: string): number {
	const luminance = (hex: string): number => {
		const channels = [1, 3, 5].map((index) => parseInt(hex.slice(index, index + 2), 16) / 255)
			.map((channel) => channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4);
		return channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722;
	};
	const a = luminance(first);
	const b = luminance(second);
	return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

function mix(foreground: string, background: string, amount: number): string {
	return `color-mix(in srgb, ${foreground} ${amount * 100}%, ${background} ${100 - amount * 100}%)`;
}

function normalizedHex(value: string): string {
	return typeof value === 'string' && HEX_COLOR.test(value) ? value.toUpperCase() : '#000000';
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function deepFreeze<T>(value: T): T {
	if (value && typeof value === 'object' && !Object.isFrozen(value)) {
		Object.freeze(value);
		for (const child of Object.values(value as Record<string, unknown>)) deepFreeze(child);
	}
	return value;
}

