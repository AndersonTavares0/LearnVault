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
 * Só a plataforma HTTP explicitamente configurada oferece login. O desktop nunca cai no
 * login: os comandos de autenticação do desktop rejeitam, então o usuário ficaria preso —
 * inclusive depois de escolher a pasta da biblioteca. Nesse caso ele permanece na tela de
 * configuração offline, que reflete se a biblioteca já está configurada.
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
	/** Sessão atual, ou `null` quando não há login. */
	user: User | null;
	/** Estado de configuração da plataforma. */
	config: ConfigState;
	/** Mensagem legível de falha na leitura da sessão, ou string vazia. */
	error: string;
};

/**
 * Resolve a sessão inicial sem deixar uma falha de `me()` esconder a configuração.
 *
 * Uma falha ao ler a sessão (por exemplo, rede indisponível no HTTP) não pode ser
 * confundida com "biblioteca não configurada": a configuração da plataforma é lida sempre
 * que não há usuário, inclusive depois de uma falha de `me()`. Assim o HTTP mantém o login
 * disponível com um erro apropriado, enquanto o desktop continua sem oferecer login quando
 * a configuração é `unconfigured`/`unknown`.
 */
export async function resolveSession(
	readSession: () => Promise<User | null>,
	readPlatformConfig: () => Promise<PlatformConfig>
): Promise<SessionBootstrap> {
	let user: User | null = null;
	let error = '';
	try {
		user = await readSession();
	} catch {
		error = 'Não foi possível conectar ao serviço.';
	}
	const config = user ? 'unknown' : await readConfig(readPlatformConfig);
	return { user, config, error };
}
