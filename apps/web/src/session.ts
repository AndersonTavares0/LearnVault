import type { PlatformConfig, User } from './transport';

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
 * Decide a tela inicial a partir da sessão e do estado de configuração.
 *
 * Só a plataforma explicitamente configurada (o servidor HTTP) oferece login. Um desktop
 * sem biblioteca configurada ou cuja configuração não pôde ser lida nunca cai no login:
 * os comandos de autenticação do desktop rejeitam, então o usuário ficaria preso.
 */
export function initialView(user: User | null, config: ConfigState): InitialView {
	if (user) return 'app';
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
