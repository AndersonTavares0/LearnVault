// Rótulos da verificação de atualização. A decisão é pura e testável; a interface só
// exibe o resultado. Nada é baixado nem instalado automaticamente: no máximo um aviso e
// um link para a release publicada.
import type { UpdateStatus } from './transport';

export type UpdateOutcome = 'disabled' | 'available' | 'current' | 'error';

// Desligado tem prioridade: quando a checagem está desabilitada por configuração, não há
// erro a mostrar mesmo que a resposta traga um campo `error` residual.
export function updateOutcome(status: UpdateStatus): UpdateOutcome {
	if (!status.enabled) return 'disabled';
	if (status.error) return 'error';
	if (status.available) return 'available';
	return 'current';
}

// Uma única linha para cada desfecho, em pt-BR e sem emojis.
export function updateMessage(status: UpdateStatus): string {
	switch (updateOutcome(status)) {
		case 'disabled': return 'A verificação de atualização está desligada neste servidor.';
		case 'available': return `Nova versão disponível: ${status.latest ?? 'desconhecida'}.`;
		case 'error': return status.error ?? 'Não foi possível verificar a atualização.';
		case 'current': return 'Você está usando a versão mais recente.';
	}
}

// Só oferece o link quando há uma versão nova e a URL é HTTPS: nunca um destino
// inesperado, e nunca um link para a release atual quando já se está atualizado.
export function updateReleaseUrl(status: UpdateStatus): string | null {
	const url = status.available ? status.url : null;
	return url && url.startsWith('https://') ? url : null;
}
