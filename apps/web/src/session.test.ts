import { describe, expect, it } from 'vitest';
import { initialView } from './session';
import type { User } from './transport';

const user: User = { userId: 'u1', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: false };

describe('initial view', () => {
	it('opens the app when a session exists', () => {
		expect(initialView(user, false)).toBe('app');
		expect(initialView(user, true)).toBe('app');
	});
	it('asks for login when the platform is configured and there is no session', () => {
		expect(initialView(null, true)).toBe('login');
	});
	it('shows the offline unconfigured state when the desktop has no library', () => {
		expect(initialView(null, false)).toBe('unconfigured');
	});
});
