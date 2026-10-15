import { describe, expect, it } from 'vitest';
import { href, routeFromHash } from './router';

describe('hash router', () => {
	it('defaults unknown hashes to login and supports the core routes', () => {
		expect(routeFromHash('')).toBe('login');
		expect(routeFromHash('#/catalog')).toBe('catalog');
		expect(routeFromHash('#/player/lesson')).toBe('player');
	});
	it('creates server-independent links', () => expect(href('home')).toBe('#/home'));
});
