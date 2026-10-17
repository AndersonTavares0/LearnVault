import { describe, expect, it } from 'vitest';
import { initialView, readConfig, resolveSession } from './session';
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

	it('never routes the desktop to login, even after a library is configured', () => {
		// A decisão de tela continua segura sem o bootstrap local: sem uma sessão pronta,
		// o desktop não oferece login nem abre a biblioteca por acidente.
		expect(initialView(null, 'configured', 'desktop')).toBe('offline');
		expect(initialView(null, 'unconfigured', 'desktop')).toBe('offline');
		expect(initialView(null, 'unknown', 'desktop')).toBe('offline');
		// Uma sessão existente ainda abre o aplicativo.
		expect(initialView(user, 'configured', 'desktop')).toBe('app');
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

describe('session bootstrap', () => {
	it('keeps the HTTP login reachable when the session probe fails', async () => {
		const session = await resolveSession(
			async () => {
				throw new Error('rede indisponível');
			},
			async () => ({ configured: true })
		);

		expect(session.user).toBeNull();
		expect(session.error).toBe('Não foi possível conectar ao serviço.');
		// A falha de `me()` não pode virar "biblioteca não configurada": a configuração
		// continua sendo lida e o HTTP mantém o login com o erro apropriado.
		expect(session.config).toBe('configured');
		expect(initialView(session.user, session.config)).toBe('login');
	});

	it('never offers desktop login when the configuration is unconfigured or unknown', async () => {
		const unconfigured = await resolveSession(
			async () => null,
			async () => ({ configured: false })
		);
		expect(initialView(unconfigured.user, unconfigured.config)).toBe('offline');

		const unknown = await resolveSession(async () => null, async () => {
			throw new Error('IPC indisponível');
		});
		expect(initialView(unknown.user, unknown.config)).toBe('offline');
	});

	it('opens the app for an existing session without reading the platform configuration', async () => {
		let reads = 0;
		const session = await resolveSession(async () => user, async () => {
			reads += 1;
			return { configured: true };
		});

		expect(initialView(session.user, session.config)).toBe('app');
		expect(reads).toBe(0);
	});

	it('creates a local desktop session only after configuration is confirmed', async () => {
		const configured = await resolveSession(async () => null, async () => ({ configured: true }), 'desktop');
		expect(configured.user).toMatchObject({ username: 'Biblioteca local', role: 'admin', mustChangeCredentials: false });
		expect(initialView(configured.user, configured.config, 'desktop')).toBe('app');

		const unknown = await resolveSession(async () => null, async () => { throw new Error('IPC indisponível'); }, 'desktop');
		expect(unknown.user).toBeNull();
		expect(initialView(unknown.user, unknown.config, 'desktop')).toBe('offline');
	});
});
