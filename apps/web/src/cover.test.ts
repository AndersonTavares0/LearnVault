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
