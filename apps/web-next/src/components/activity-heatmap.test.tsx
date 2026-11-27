import { cleanup, fireEvent, render, screen } from '@testing-library/react';
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

	it('offers a keyboard-accessible table of daily study details', async () => {
		render(<ActivityHeatmap today="2026-11-01" activeDays={['2026-11-01']} activity={{ '2026-11-01': { score: 3, lessons: 2, focusMinutes: 25 } }} />);

		const disclosure = screen.getByText('View daily study activity');
		disclosure.focus();
		expect(document.activeElement).toBe(disclosure);
		expect(screen.queryByRole('table')).toBeNull();
		fireEvent.click(disclosure);
		expect(await screen.findByRole('table', { name: 'Daily study activity' })).toBeTruthy();
		expect(screen.getByRole('row', { name: /Nov 1, 2026 2 lessons, 25 min focus/ })).toBeTruthy();
		expect(screen.getByRole('row', { name: /Oct 31, 2026 No study recorded/ })).toBeTruthy();
		expect(screen.queryByRole('row', { name: /Nov 2, 2026/ })).toBeNull();
	});

	it('scales cell intensity across levels and shows details in a hover tooltip', () => {
		const { container } = render(
			<ActivityHeatmap
				today="2026-11-01"
				activeDays={['2026-10-29', '2026-10-30', '2026-10-31', '2026-11-01']}
				activity={{
					'2026-10-29': { score: 1, lessons: 1 },
					'2026-10-30': { score: 2, lessons: 1, focusMinutes: 15 },
					'2026-10-31': { score: 4, lessons: 2, focusMinutes: 30 },
					'2026-11-01': { score: 6, lessons: 3, focusMinutes: 50 }
				}}
			/>
		);
		expect(container.querySelector('[data-day="2026-10-29"]')?.getAttribute('data-level')).toBe('1');
		expect(container.querySelector('[data-day="2026-10-30"]')?.getAttribute('data-level')).toBe('2');
		expect(container.querySelector('[data-day="2026-10-31"]')?.getAttribute('data-level')).toBe('3');
		const heavy = container.querySelector('[data-day="2026-11-01"]')!;
		expect(heavy.getAttribute('data-level')).toBe('4');

		fireEvent.mouseEnter(heavy);
		expect(screen.getByRole('tooltip').textContent).toContain('3 lessons, 50 min focus');

		fireEvent.mouseLeave(container.querySelector('[data-heatmap-grid]')!);
		expect(screen.queryByRole('tooltip')).toBeNull();
	});
});
