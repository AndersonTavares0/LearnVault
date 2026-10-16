import type { PlatformConfig, TransportKind, User } from './transport';

/**
 * Estado de configuração da plataforma observado na inicialização.
 *
 * `unknown` é o resultado de uma leitura IPC que falhou: o desktop não sabe se há
 * biblioteca, então não pode oferecer login nem se declarar configurado.
 */
export type ConfigState = 'configured' | 'unconfigured' | 'unknown';

/** Tela inicial do aplicativo antes de qualquer navegação. */
export type InitialView = 'app' | 'login' | 'offline';

/**
 * Decide a tela inicial a partir da sessão, do estado de configuração e do transporte.
 *
 * Só a plataforma HTTP explicitamente configurada oferece login. O desktop usa uma sessão
 * local sintética apenas depois de confirmar a biblioteca; sem essa confirmação continua
 * offline e nunca cai no formulário de login.
 */
export function initialView(user: User | null, config: ConfigState, kind: TransportKind = 'http'): InitialView {
	if (user) return 'app';
	if (kind === 'desktop') return 'offline';
	return config === 'configured' ? 'login' : 'offline';
}

/**
 * Lê a configuração da plataforma e reduz a leitura a um estado explícito.
 *
 * Uma falha na leitura IPC vira `unknown` em vez de propagar: a falha não pode virar
 * "configurado por omissão" e reabrir o login no desktop.
 */
export async function readConfig(read: () => Promise<PlatformConfig>): Promise<ConfigState> {
	try {
		const status = await read();
		return status.configured ? 'configured' : 'unconfigured';
	} catch {
		return 'unknown';
	}
}

/** Estado inicial resolvido na montagem do aplicativo. */
export type SessionBootstrap = {
	/** Sessão atual; o desktop configurado recebe uma identidade local sem login. */
	user: User | null;
	/** Estado de configuração da plataforma. */
	config: ConfigState;
	/** Mensagem legível de falha na leitura da sessão, ou string vazia. */
	error: string;
};

/** Identidade local, sem credenciais e sem dados do caminho da biblioteca. */
export const desktopLocalUser: User = {
	userId: 'local',
	username: 'Biblioteca local',
	role: 'admin',
	isAdmin: true,
	mustChangeCredentials: false
};

/**
 * Resolve a sessão inicial sem deixar uma falha de `me()` esconder a configuração.
 *
 * Uma falha ao ler a sessão (por exemplo, rede indisponível no HTTP) não pode ser
 * confundida com "biblioteca não configurada": a configuração da plataforma é lida sempre
 * que não há usuário, inclusive depois de uma falha de `me()`. Assim o HTTP mantém o login
 * disponível com um erro apropriado. No desktop, uma configuração confirmada vira uma
 * identidade local sem credenciais; estados `unconfigured`/`unknown` continuam offline.
 */
export async function resolveSession(
	readSession: () => Promise<User | null>,
	readPlatformConfig: () => Promise<PlatformConfig>,
	kind: TransportKind = 'http'
): Promise<SessionBootstrap> {
	let user: User | null = null;
	let error = '';
	try {
		user = await readSession();
	} catch {
		error = 'Não foi possível conectar ao serviço.';
	}
	const config = user ? 'unknown' : await readConfig(readPlatformConfig);
	if (!user && kind === 'desktop' && config === 'configured') user = desktopLocalUser;
	return { user, config, error };
}
