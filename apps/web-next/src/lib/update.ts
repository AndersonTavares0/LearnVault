import { tr } from './i18n';
import type { UpdateStatus } from './transport';

export type UpdateOutcome = 'disabled' | 'available' | 'current' | 'error';

export function updateOutcome(status: UpdateStatus): UpdateOutcome {
	if (!status.enabled) return 'disabled';
	if (status.error) return 'error';
	if (status.available) return 'available';
	return 'current';
}

export function updateMessage(status: UpdateStatus): string {
	switch (updateOutcome(status)) {
		case 'disabled':
			return tr('Update checks are disabled on this server.', 'A verificação de atualização está desligada neste servidor.');
		case 'available':
			return tr(
				`New version available: ${status.latest ?? 'unknown'}.`,
				`Nova versão disponível: ${status.latest ?? 'desconhecida'}.`
			);
		case 'error':
			return status.error ?? tr('Could not check for updates.', 'Não foi possível verificar a atualização.');
		case 'current':
			return tr('You are using the latest version.', 'Você está usando a versão mais recente.');
	}
}

// Só oferece o link da versão informada no repositório oficial, nunca outro destino
// HTTPS recebido na resposta e nunca um link quando já se está atualizado.
export function updateReleaseUrl(status: UpdateStatus): string | null {
	const url = status.available ? status.url : null;
	if (!url || !status.latest) return null;

	try {
		const parsed = new URL(url);
		const releasePath = '/AndersonTavares0/LearnVault/releases/tag/';
		const tag = parsed.pathname.startsWith(releasePath)
			? parsed.pathname.slice(releasePath.length).replace(/^v/, '')
			: '';
		const [tagVersion, buildMetadata, ...extraMetadata] = tag.split('+');
		const validBuildMetadata = buildMetadata === undefined
			|| (extraMetadata.length === 0
				&& buildMetadata !== ''
				&& buildMetadata.split('.').every((identifier) => /^[0-9A-Za-z-]+$/.test(identifier)));
		const expectedPath = tagVersion === status.latest && validBuildMetadata;
		if (
			parsed.origin !== 'https://github.com'
			|| parsed.username
			|| parsed.password
			|| parsed.search
			|| parsed.hash
			|| !expectedPath
		) {
			return null;
		}
		return parsed.href;
	} catch {
		return null;
	}
}
