import type { Route } from './router';
import type { TransportKind, User } from './transport';

/** Estado inicial resolvido na montagem do aplicativo. */
export type SessionBootstrap = {
	/** Sessão atual; o desktop recebe uma identidade local sem login. */
	user: User | null;
	/** Mensagem legível de falha na leitura da sessão, ou string vazia. */
	error: string;
};

/**
 * Rota inicial do build quando o hash não nomeia uma rota navegável.
 *
 * O servidor começa no login: sem sessão, o formulário é o ponto de partida. O desktop
 * não tem sessão nem login, então qualquer endereço de login ou desconhecido cai na
 * tela inicial em vez de abrir um formulário que não consegue autenticar.
 */
export function defaultRoute(kind: TransportKind): Route {
	return kind === 'desktop' ? 'home' : 'login';
}

/** Identidade local, sem credenciais e sem dados do caminho dos cursos. */
export const desktopLocalUser: User = {
	userId: 'local',
	username: 'Biblioteca local',
	role: 'admin',
	isAdmin: true,
	mustChangeCredentials: false
};

/**
 * Resolve a sessão inicial do aplicativo.
 *
 * O desktop é sempre local: sem credenciais e sem portas de rede, ele usa a identidade
 * sintética e segue direto para a aplicação, mesmo sem nenhum curso importado (a tela
 * inicial convida a importar). O HTTP lê a sessão do servidor e, em caso de falha de
 * rede, mantém o formulário de login com uma mensagem apropriada em vez de declarar a
 * sessão encerrada.
 */
export async function resolveSession(
	readSession: () => Promise<User | null>,
	kind: TransportKind = 'http'
): Promise<SessionBootstrap> {
	if (kind === 'desktop') return { user: desktopLocalUser, error: '' };
	try {
		return { user: await readSession(), error: '' };
	} catch {
		return { user: null, error: 'Não foi possível conectar ao serviço.' };
	}
}
