import { cleanup, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { renderApp } from '@/test/render-app';

afterEach(cleanup);

// The shell is one column: the bar first, then the region that scrolls (issue #607). While the
// document was the scroller, the bar scrolled away and the window scrollbar crossed it.
describe('app shell', () => {
	it('scrolls the content region instead of the document', () => {
		renderApp('/');
		const bar = screen.getByRole('banner');
		const main = screen.getByRole('main');
		const shell = bar.parentElement!;

		expect(shell.className).toContain('h-dvh');
		expect(shell.className).toContain('flex-col');
		expect(bar.className).toContain('shrink-0');
		expect(main.className).toContain('app-scroll');
		expect(main.className).toContain('overflow-y-auto');
		expect(main.className).toContain('min-h-0');

		// The bar comes before the scroller and never sits inside it, so it cannot scroll away.
		expect(bar.contains(main)).toBe(false);
		const order = Array.from(shell.children);
		expect(order.indexOf(bar)).toBeLessThan(order.indexOf(main));

		// The phone tab bar floats over the bottom edge, so the scroller keeps the clearance.
		expect(main.firstElementChild?.className).toContain('pb-28');
	});
});
