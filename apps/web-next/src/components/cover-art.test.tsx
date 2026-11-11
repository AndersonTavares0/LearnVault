import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { CoverArt } from './cover-art';

afterEach(cleanup);

describe('CoverArt', () => {
	it('sets the initials large and draws no title or progress', () => {
		const { container } = render(<CoverArt title="Rust from Zero" done={2} total={4} />);
		expect(container.querySelector('text')?.textContent).toBe('RF');
		expect(container.querySelectorAll('rect')).toHaveLength(2);
	});

	it('uses one letter on small thumbnails', () => {
		const { container } = render(<CoverArt title="Rust from Zero" done={0} total={4} compact />);
		expect(container.querySelector('text')?.textContent).toBe('R');
	});

	it('draws one block per lesson in map mode and no title', () => {
		const { container } = render(<CoverArt title="Rust from Zero" done={1} total={6} map />);
		expect(container.querySelectorAll('rect[rx="3"]')).toHaveLength(6);
		expect(container.querySelector('text')).toBeNull();
	});
});
