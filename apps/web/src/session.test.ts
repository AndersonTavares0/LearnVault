import { describe, expect, it } from 'vitest';
import { initialView, readConfig } from './session';
import type { User } from './transport';

const user: User = { userId: 'u1', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: false };

describe('initial view', () => {
	it('opens the app when a session exists, regardless of configuration', () => {
		expect(initialView(user, 'configured')).toBe('app');
		expect(initialView(user, 'unconfigured')).toBe('app');
		expect(initialView(user, 'unknown')).toBe('app');
	});

	it('asks for login only when the platform is explicitly configured', () => {
		expect(initialView(null, 'configured')).toBe('login');
	});

	it('never asks for login when the desktop has no library or the configuration read failed', () => {
		expect(initialView(null, 'unconfigured')).toBe('offline');
		// O desktop rejeita login; cair no formulário de login deixaria o usuário preso.
		expect(initialView(null, 'unknown')).not.toBe('login');
		expect(initialView(null, 'unknown')).toBe('offline');
	});
});

describe('platform configuration read', () => {
	it('maps a successful read to configured or unconfigured', async () => {
		await expect(readConfig(async () => ({ configured: true }))).resolves.toBe('configured');
		await expect(readConfig(async () => ({ configured: false }))).resolves.toBe('unconfigured');
	});

	it('treats an IPC configuration failure as unknown, never configured', async () => {
		await expect(
			readConfig(async () => {
				throw new Error('banco de dados local indisponível');
			})
		).resolves.toBe('unknown');
	});
});
