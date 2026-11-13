import { tr } from './i18n';
import type { UpdateStatus } from './transport';

export type UpdateOutcome = 'disabled' | 'available' | 'current' | 'error';

/** A desktop update check runs at most once a day, on the first launch inside that window. */
export const UPDATE_CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000;
const UPDATE_CHECKED_AT_KEY = 'learnvault-update-checked-at';

/** Last automatic check, in epoch milliseconds; `null` when there is none or storage is unavailable. */
export function readUpdateCheckAt(storage: Pick<Storage, 'getItem'> | null): number | null {
	try {
		const raw = storage?.getItem(UPDATE_CHECKED_AT_KEY);
		const value = raw ? Number(raw) : Number.NaN;
		return Number.isFinite(value) ? value : null;
	} catch {
		return null;
	}
}

/** True on the first launch and again after a full day, false in between. */
export function shouldCheckForUpdate(lastCheckedAt: number | null, now: number): boolean {
	return lastCheckedAt === null || now - lastCheckedAt >= UPDATE_CHECK_INTERVAL_MS;
}

/** Records the moment of an automatic check; a failed check counts too, so it never retries in a loop. */
export function markUpdateChecked(storage: Pick<Storage, 'setItem'> | null, now: number): void {
	try {
		storage?.setItem(UPDATE_CHECKED_AT_KEY, String(now));
	} catch {
		// Storage can be unavailable; the check then runs once per launch.
	}
}

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
