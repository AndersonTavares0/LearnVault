import type { User } from './transport';

/** Tela inicial do aplicativo antes de qualquer navegação. */
export type InitialView = 'app' | 'login' | 'unconfigured';

/**
 * Decide a tela inicial a partir da sessão e da configuração da plataforma.
 *
 * O servidor HTTP está sempre configurado, então a ausência de sessão leva ao login.
 * O desktop sem biblioteca configurada não tem login: mostra o estado offline não
 * configurado, sem oferecer seletor de pasta nem escrever na biblioteca.
 */
export function initialView(user: User | null, configured: boolean): InitialView {
	if (user) return 'app';
	return configured ? 'login' : 'unconfigured';
}
