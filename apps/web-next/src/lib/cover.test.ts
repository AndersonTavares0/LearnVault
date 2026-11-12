import { describe, expect, it } from 'vitest';
import { courseCover, courseMonogram, selectCourseCoverSource } from './cover';

describe('generated course covers', () => {
	it('is deterministic for the same title', () => {
		const first = courseCover('Física Quântica');
		const second = courseCover('Física Quântica');
		expect(second).toEqual(first);
	});

	it('derives a two-letter monogram from significant words', () => {
		expect(courseMonogram('Física Quântica')).toBe('FQ');
		expect(courseMonogram('01 - Programação em Go')).toBe('PG');
		expect(courseMonogram('Matemática')).toBe('M');
	});

	it('never leaves the monogram empty and keeps accents out', () => {
		expect(courseMonogram('   ')).toBe('?');
		expect(courseMonogram('--')).toBe('?');
		expect(courseCover('Ação e Reação').monogram).toBe('AR');
	});

	it('uses gradient colors from the product palette and varies by title', () => {
		const first = courseCover('Física');
		const second = courseCover('História da Arte');

		for (const color of [first.from, first.to, second.from, second.to]) {
			expect(color).toMatch(/^hsl\(\d+ \d+% \d+%\)$/);
		}
		expect(first).not.toEqual(second);
	});

	it('derives a known pattern and never invents a course code', () => {
		const first = courseCover('Física Quântica');
		expect(first).not.toHaveProperty('code');
		expect(['grid', 'arcs', 'lines', 'dots', 'blocks', 'wave']).toContain(first.pattern);
		expect(courseCover('História da Arte').pattern).not.toBe('');
	});

	it('derives a deterministic gradient angle within a readable range', () => {
		const titles = ['Física Quântica', 'História da Arte', 'Programação em Go', '   ', '--', 'Matemática'];
		const angles = titles.map((title) => courseCover(title).angle);

		for (const angle of angles) {
			expect(Number.isInteger(angle)).toBe(true);
			expect(angle).toBeGreaterThanOrEqual(95);
			expect(angle).toBeLessThanOrEqual(200);
		}
		// O mesmo título produz sempre o mesmo ângulo, e títulos distintos variam.
		expect(courseCover('Física Quântica').angle).toBe(courseCover('Física Quântica').angle);
		expect(new Set(angles).size).toBeGreaterThan(1);
	});
});

describe('course cover precedence', () => {
	it('prefers a manual cover over an automatic thumbnail', () => {
		expect(selectCourseCoverSource({ manualCoverUrl: 'cover://uploaded', thumbnailUrl: 'blob:frame' })).toBe('manual');
	});

	it('prefers the automatic thumbnail when no manual cover exists', () => {
		expect(selectCourseCoverSource({ thumbnailUrl: 'blob:frame' })).toBe('thumbnail');
	});

	it('falls back to the generated title cover when neither source is present', () => {
		expect(selectCourseCoverSource({})).toBe('generated');
		expect(selectCourseCoverSource({ manualCoverUrl: null, thumbnailUrl: null })).toBe('generated');
		expect(selectCourseCoverSource({ manualCoverUrl: '', thumbnailUrl: '' })).toBe('generated');
	});
});

/** WCAG relative luminance of an `hsl(h s% l%)` colour, as produced by `courseCover`. */
function luminanceOfHsl(value: string): number {
	const [, hue, sat, light] = value.match(/hsl\((\d+(?:\.\d+)?) (\d+(?:\.\d+)?)% (\d+(?:\.\d+)?)%\)/) ?? [];
	const h = Number(hue) / 360;
	const s = Number(sat) / 100;
	const l = Number(light) / 100;
	const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
	const p = 2 * l - q;
	const channel = (t: number) => {
		const x = ((t % 1) + 1) % 1;
		const v = x < 1 / 6 ? p + (q - p) * 6 * x : x < 1 / 2 ? q : x < 2 / 3 ? p + (q - p) * (2 / 3 - x) * 6 : p;
		return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
	};
	return 0.2126 * channel(h + 1 / 3) + 0.7152 * channel(h) + 0.0722 * channel(h - 1 / 3);
}

describe('generated cover text contrast', () => {
	it('keeps white title text at 5:1 or more on the lightest corner of every palette hue', () => {
		for (let i = 0; i < 200; i += 1) {
			const cover = courseCover(`Curso ${i}`);
			const lightest = Math.max(luminanceOfHsl(cover.from), luminanceOfHsl(cover.to));
			const ratio = 1.05 / (lightest + 0.05);
			expect(ratio, `Curso ${i}: ${cover.from}`).toBeGreaterThanOrEqual(5);
		}
	});
});
