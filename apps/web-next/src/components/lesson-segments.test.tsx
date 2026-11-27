import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { LessonSegments } from './lesson-segments';

afterEach(cleanup);

describe('LessonSegments', () => {
	it('draws one segment per lesson with its state and an accessible total', () => {
		render(<LessonSegments done={1} total={4} current={1} />);
		const bar = screen.getByRole('progressbar', { name: '1 of 4 lessons done' });
		expect([...bar.children].map((segment) => segment.getAttribute('data-state'))).toEqual(['done', 'current', 'pending', 'pending']);
	});

	it('follows explicit states when lessons were finished out of order', () => {
		render(<LessonSegments done={1} total={3} states={['pending', 'current', 'done']} />);
		expect([...screen.getByRole('progressbar').children].map((segment) => segment.getAttribute('data-state'))).toEqual(['pending', 'current', 'done']);
	});

	it('falls back to a single bar for very long courses', () => {
		render(<LessonSegments done={10} total={120} />);
		expect(screen.getByRole('progressbar').querySelectorAll('[data-state]')).toHaveLength(0);
	});
});
