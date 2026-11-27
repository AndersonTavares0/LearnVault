import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { CoverArt } from './cover-art';

afterEach(cleanup);

describe('CoverArt', () => {
	it('sets the course title and lesson count on a studio plate, with no monogram', () => {
		const { container } = render(<CoverArt title="Rust from Zero" done={2} total={4} />);
		expect(container.textContent).toContain('Rust from Zero');
		expect(container.textContent).toContain('4 lessons');
		expect(container.textContent).not.toContain('RF');
		expect(container.querySelector('svg')).toBeNull();
		expect(container.firstElementChild?.getAttribute('style')).toContain('--cover-hue');
	});

	it('uses the singular for a one-lesson course and omits the count when there are no lessons', () => {
		const { container, unmount } = render(<CoverArt title="Intro" done={0} total={1} />);
		expect(container.textContent).toContain('1 lesson');
		unmount();
		const empty = render(<CoverArt title="Intro" done={0} total={0} />);
		expect(empty.container.textContent).not.toContain('lesson');
	});

	it('keeps the same ground for the same course', () => {
		const first = render(<CoverArt title="Rust from Zero" courseId="c1" done={0} total={4} />);
		const second = render(<CoverArt title="Rust from Zero" courseId="c1" done={0} total={4} />);
		expect(first.container.firstElementChild?.getAttribute('style')).toBe(second.container.firstElementChild?.getAttribute('style'));
	});

	it('shows only the first letter on small thumbnails', () => {
		const { container } = render(<CoverArt title="Rust from Zero" done={0} total={4} compact />);
		expect(container.textContent).toBe('R');
	});

	it('draws one block per lesson in map mode and no title', () => {
		const { container } = render(<CoverArt title="Rust from Zero" done={1} total={6} map />);
		expect(container.querySelectorAll('rect[rx="3"]')).toHaveLength(6);
		expect(container.textContent).toBe('');
	});
});
