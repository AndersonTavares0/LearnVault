import { describe, expect, it } from 'vitest';
import { courseCover, courseMonogram } from './cover';

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
});
