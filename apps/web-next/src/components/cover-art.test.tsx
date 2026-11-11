import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { CoverArt, wrapTitle } from './cover-art';

afterEach(cleanup);

describe('wrapTitle', () => {
	it('breaks a title into short lines on word boundaries', () => {
		expect(wrapTitle('Data Analysis with Python')).toEqual(['Data Analysis', 'with Python']);
		expect(wrapTitle('Rust')).toEqual(['Rust']);
	});

	it('keeps three lines at most and marks what was cut', () => {
		const lines = wrapTitle('An Extremely Long Introduction to Advanced Distributed Systems Engineering Practice');
		expect(lines).toHaveLength(3);
		expect(lines[2].endsWith('…')).toBe(true);
	});
});

describe('CoverArt', () => {
	const bars = (container: HTMLElement) => container.querySelectorAll('rect[height="4"]');

	it('draws one bar per lesson and lights the finished ones', () => {
		const { container } = render(<CoverArt title="Rust from Zero" done={2} total={4} />);
		expect(bars(container)).toHaveLength(4);
		expect(container.querySelectorAll('.cover-paper rect.fill-white')).toHaveLength(2);
	});

	it('falls back to one continuous bar for very long courses', () => {
		const { container } = render(<CoverArt title="Everything" done={30} total={120} />);
		expect(bars(container)).toHaveLength(2);
	});

	it('draws one block per lesson in map mode and no title', () => {
		const { container } = render(<CoverArt title="Rust from Zero" done={1} total={6} map />);
		expect(container.querySelectorAll('rect[rx="3"]')).toHaveLength(6);
		expect(container.querySelector('text')).toBeNull();
	});
});
