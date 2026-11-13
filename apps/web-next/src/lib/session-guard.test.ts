import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SESSION_EXPIRED_EVENT, installSessionGuard } from './session-guard';

const respond = (status: number) => async () => new Response('{"error":"x"}', { status });

describe('session guard', () => {
	let events = 0;
	const listener = () => {
		events += 1;
	};

	beforeEach(() => {
		events = 0;
		window.addEventListener(SESSION_EXPIRED_EVENT, listener);
	});

	afterEach(() => {
		window.removeEventListener(SESSION_EXPIRED_EVENT, listener);
		vi.unstubAllGlobals();
	});

	it('announces an expired session when an API call answers 401', async () => {
		vi.stubGlobal('fetch', respond(401));
		installSessionGuard();
		const response = await fetch('/api/catalog');
		expect(response.status).toBe(401);
		expect(events).toBe(1);
	});

	it('ignores a failed sign-in', async () => {
		vi.stubGlobal('fetch', respond(401));
		installSessionGuard();
		await fetch('/api/login', { method: 'POST' });
		expect(events).toBe(0);
	});

	it('ignores non-API responses', async () => {
		vi.stubGlobal('fetch', respond(401));
		installSessionGuard();
		await fetch('/login');
		expect(events).toBe(0);
	});

	it('passes successful responses through untouched', async () => {
		vi.stubGlobal('fetch', async () => new Response('ok', { status: 200 }));
		installSessionGuard();
		const response = await fetch('/api/catalog');
		expect(response.status).toBe(200);
		expect(await response.text()).toBe('ok');
		expect(events).toBe(0);
	});
});
