import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { ActivityHeatmap } from './activity-heatmap';

afterEach(cleanup);

describe('ActivityHeatmap', () => {
	it('counts only the study days inside the last year and draws one cell per day', () => {
		const { container } = render(<ActivityHeatmap today="2026-11-01" activeDays={['2026-10-31', '2026-11-01', '2025-01-01']} />);
		expect(screen.getByText('2 days with study in the last year')).toBeTruthy();
		const cells = container.querySelectorAll('[data-heatmap-grid] > span');
		expect(cells.length).toBe(371);
		expect(container.querySelectorAll('[data-heatmap-grid] > span.bg-primary').length).toBe(2);
	});

	it('uses the singular for a single study day', () => {
		render(<ActivityHeatmap today="2026-11-01" activeDays={['2026-11-01']} />);
		expect(screen.getByText('1 day with study in the last year')).toBeTruthy();
	});
});
