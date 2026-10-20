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

	it('uses the three original Graphite gradients and their fixed direction', () => {
		const approvedPalettes = new Set([
			'#295c67:#a7c9bd',
			'#695e4e:#d4b77e',
			'#3e4160:#a4a6d5'
		]);
		const titles = ['Física', 'História da Arte', 'Programação em Go', 'Matemática'];

		for (const title of titles) {
			const { from, to, angle } = courseCover(title);
			expect(approvedPalettes.has(`${from}:${to}`)).toBe(true);
			expect(angle).toBe(140);
		}
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
