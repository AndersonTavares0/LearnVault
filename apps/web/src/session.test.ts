import { describe, expect, it } from 'vitest';
import { defaultRoute, desktopLocalUser, resolveSession } from './session';
import type { User } from './transport';

const user: User = { userId: 'u1', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: false };

describe('session bootstrap', () => {
	it('keeps the HTTP login reachable when the session probe fails', async () => {
		const session = await resolveSession(async () => {
			throw new Error('rede indisponível');
		});

		expect(session.user).toBeNull();
		expect(session.error).toBe('Não foi possível conectar ao serviço.');
	});

	it('uses the server session on HTTP', async () => {
		const session = await resolveSession(async () => user);

		expect(session.user).toBe(user);
		expect(session.error).toBe('');
	});

	it('uses the local identity on desktop without probing the server', async () => {
		let reads = 0;
		const session = await resolveSession(async () => {
			reads += 1;
			return null;
		}, 'desktop');

		expect(session.user).toMatchObject({ username: 'Biblioteca local', role: 'admin', mustChangeCredentials: false });
		expect(reads).toBe(0);
	});

	it('keeps the desktop local identity even without any imported course', () => {
		// A tela inicial do desktop convida a importar um curso; ela não pode depender de
		// uma configuração prévia nem cair no formulário de login.
		expect(desktopLocalUser.isAdmin).toBe(true);
	});
});

describe('build default route', () => {
	it('starts the HTTP build at login and the desktop build at home', () => {
		// O desktop não tem sessão para iniciar: um endereço de login não faz sentido nele.
		expect(defaultRoute('http')).toBe('login');
		expect(defaultRoute('desktop')).toBe('home');
	});
});

/**
 * Guardas de markup do shell foram removidas: o que permanece aqui são os contratos
 * comportamentais de sessão e rota padrão; o chrome HTTP fica coberto por E2E.
 */

