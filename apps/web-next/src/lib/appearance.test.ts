import { describe, expect, it, vi } from 'vitest';
import { ACCENT_PRESETS, applyAppearance, appearanceStore, DEFAULT_APPEARANCE, SURFACES, SURFACE_SWATCHES, contrast, darkAccent, loadAppearance, parseAppearance, readAppearance, readableAccent, updateAppearance } from '@/lib/appearance';

describe('appearance', () => {
	it('falls back to the defaults for anything missing or invalid', () => {
		expect(parseAppearance(null)).toEqual(DEFAULT_APPEARANCE);
		expect(parseAppearance({ accent: 'red', radius: 'huge', showXp: 'yes' })).toEqual(DEFAULT_APPEARANCE);
		expect(parseAppearance({ accent: '#c2185b', radius: 'round', showXp: false })).toMatchObject({ accent: '#C2185B', radius: 'round', showXp: false });
	});

	it('reads unreadable storage as the defaults', () => {
		expect(readAppearance({ getItem: () => { throw new Error('denied'); } })).toEqual(DEFAULT_APPEARANCE);
		expect(readAppearance({ getItem: () => '{bad json' })).toEqual(DEFAULT_APPEARANCE);
	});

	it('keeps every preset readable under white text', () => {
		for (const preset of ACCENT_PRESETS) expect(contrast('#FFFFFF', readableAccent(preset.hex)), preset.id).toBeGreaterThanOrEqual(4.5);
	});

	it('darkens a colour that is too light until white text reads on it', () => {
		const adjusted = readableAccent('#FFE066');
		expect(adjusted).not.toBe('#FFE066');
		expect(contrast('#FFFFFF', adjusted)).toBeGreaterThanOrEqual(4.5);
	});

	it('applies a change to <html> and saves it', () => {
		const root = document.createElement('html');
		const saved = new Map<string, string>();
		const storage = { getItem: (key: string) => saved.get(key) ?? null, setItem: (key: string, value: string) => void saved.set(key, value), removeItem: (key: string) => void saved.delete(key) } as unknown as Storage;
		updateAppearance({ accent: '#6A32C9', radius: 'round', density: 'compact' }, storage, root);
		expect(root.style.getPropertyValue('--brand')).toBe('#6A32C9');
		expect(root.dataset.radius).toBe('round');
		expect(root.dataset.density).toBe('compact');
		expect(JSON.parse(saved.get('learnvault-appearance')!)).toMatchObject({ accent: '#6A32C9' });
	});

	it('publishes a new snapshot when the system theme changes', () => {
		const root = document.createElement('html');
		let dark = false;
		let changed = () => {};
		const matchMedia = vi.fn().mockImplementation(() => ({
			matches: dark,
			media: '(prefers-color-scheme: dark)',
			onchange: null,
			addListener: () => {},
			removeListener: () => {},
			addEventListener: (_type: string, listener: EventListenerOrEventListenerObject) => { changed = listener as unknown as () => void; },
			removeEventListener: () => {},
			dispatchEvent: () => false
		}) as MediaQueryList);
		vi.stubGlobal('matchMedia', matchMedia);
		const unsubscribe = appearanceStore.subscribe(() => {});
		loadAppearance({ getItem: () => JSON.stringify({ ...DEFAULT_APPEARANCE, theme: 'system' }) } as unknown as Storage, root);
		dark = true;
		let notifications = 0;
		const stop = appearanceStore.subscribe(() => { notifications += 1; });
		changed();
		expect(root.dataset.theme).toBe('dark');
		expect(notifications).toBe(1);
		stop();
		unsubscribe();
		vi.unstubAllGlobals();
	});
});

describe('brand palette', () => {
	it('defaults to the warm ink accent, with its muted brass twin on the dark page', () => {
		expect(DEFAULT_APPEARANCE.accent).toBe('#1C1815');
		expect(ACCENT_PRESETS[0]).toMatchObject({ id: 'ink', hex: '#1C1815', dark: '#C9B594', tone: '#5C4229' });
		expect(darkAccent('#1C1815')).toBe('#C9B594');
	});

	it('keeps white text on the ink accent and the brass accent on the dark panel above 4.5:1', () => {
		expect(contrast('#FFFFFF', readableAccent('#1C1815'))).toBeGreaterThanOrEqual(4.5);
		const accent = darkAccent(DEFAULT_APPEARANCE.accent);
		expect(contrast('#292B2E', accent)).toBeGreaterThanOrEqual(4.5);
		expect(contrast('#1C1209', accent)).toBeGreaterThanOrEqual(4.5);
		expect(contrast('#292B2E', darkAccent('#222222'))).toBeGreaterThanOrEqual(4.5);
	});

	it('gives the reward colour at least 3:1 as a graphic on every surface, light and dark', () => {
		for (const surface of SURFACES) {
			expect(contrast('#A8761F', SURFACE_SWATCHES[surface].light), `${surface} light`).toBeGreaterThanOrEqual(3);
			expect(contrast('#C9B594', SURFACE_SWATCHES[surface].dark), `${surface} dark`).toBeGreaterThanOrEqual(3);
		}
	});

	it.each(['light', 'dark'] as const)('keeps small reward text at least 4.5:1 on its %s tint', (theme) => {
		const root = document.createElement('html');
		applyAppearance({ ...DEFAULT_APPEARANCE, theme }, root);
		expect(contrast(root.style.getPropertyValue('--reward'), root.style.getPropertyValue('--reward-tint'))).toBeGreaterThanOrEqual(4.5);
	});

	it('puts ink on the gold reward fill, the higher-contrast choice on each page', () => {
		// The text colour on the fill is not used by any component today. On the light page the gold is
		// only 4.4:1 under ink, which is enough for large text and UI parts but not for body text, so it
		// must not carry small text; it still beats white (4.0:1).
		expect(contrast('#1C1815', '#A8761F')).toBeGreaterThan(contrast('#FFFFFF', '#A8761F'));
		expect(contrast('#1C1815', '#A8761F')).toBeGreaterThanOrEqual(3);
		expect(contrast('#1C1815', '#C9B594')).toBeGreaterThanOrEqual(4.5);
	});

	it('tints the light page from brown for the ink accent, and from the accent itself for every other colour', () => {
		const root = document.createElement('html');
		applyAppearance(parseAppearance({ accent: '#1C1815', theme: 'light' }), root);
		expect(root.style.getPropertyValue('--brand')).toBe('#1C1815');
		expect(root.style.getPropertyValue('--brand-tone')).toBe('#5C4229');
		applyAppearance(parseAppearance({ accent: '#1C1815', theme: 'dark' }), root);
		expect(root.style.getPropertyValue('--brand-tone')).toBe(root.style.getPropertyValue('--brand'));
		applyAppearance(parseAppearance({ accent: '#27508F', theme: 'light' }), root);
		expect(root.style.getPropertyValue('--brand-tone')).toBe('#27508F');
	});
});
