import { describe, expect, it } from 'vitest';
import { ACCENT_PRESETS, DEFAULT_APPEARANCE, contrast, parseAppearance, readAppearance, readableAccent, updateAppearance } from '@/lib/appearance';

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
});
