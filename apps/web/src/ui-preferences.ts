import type { TransportKind } from './transport';

const STORAGE_KEY_PREFIX = 'learnvault.ui-preferences.v1:';
const MAX_STORED_BYTES = 4_096;

export interface UiPreferenceStorage {
	getItem(key: string): string | null;
	setItem(key: string, value: string): void;
}

/**
 * Preferências de layout que não pertencem ao player: a sidebar pode virar um
 * rail de ícones e o painel da aula pode ser recolhido para o vídeo ocupar a
 * tela. Os dois estados são lembrados por usuário para não surpreender quem
 * alterna de conta.
 */
export type UiPreferences = {
	sidebarCollapsed: boolean;
	panelCollapsed: boolean;
};

export function createDefaultUiPreferences(): UiPreferences {
	return { sidebarCollapsed: false, panelCollapsed: false };
}

/** Preferências web são separadas por usuário; o desktop tem um perfil local fixo. */
export function uiPreferencesScope(kind: TransportKind, userId: string | null | undefined): string | null {
	if (kind === 'desktop') return 'desktop-local';
	const normalizedUserId = userId?.trim();
	return normalizedUserId ? `http-user:${normalizedUserId}` : null;
}

export function uiPreferencesKey(scope: string): string {
	return `${STORAGE_KEY_PREFIX}${encodeURIComponent(scope)}`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function normalizePreferences(value: unknown): UiPreferences {
	const defaults = createDefaultUiPreferences();
	if (!isRecord(value)) return defaults;
	return {
		sidebarCollapsed: typeof value.sidebarCollapsed === 'boolean' ? value.sidebarCollapsed : false,
		panelCollapsed: typeof value.panelCollapsed === 'boolean' ? value.panelCollapsed : false
	};
}

/** Storage pode estar bloqueado ou corrompido; layout nunca impede o uso do app. */
export function readUiPreferences(storage: UiPreferenceStorage | null, scope: string | null): UiPreferences {
	if (!storage || !scope) return createDefaultUiPreferences();
	try {
		const raw = storage.getItem(uiPreferencesKey(scope));
		if (!raw || raw.length > MAX_STORED_BYTES) return createDefaultUiPreferences();
		return normalizePreferences(JSON.parse(raw) as unknown);
	} catch {
		return createDefaultUiPreferences();
	}
}

/** Home de tela estreita compacta a sidebar só enquanto a pessoa nunca escolheu. */
export function hasStoredUiPreferences(storage: UiPreferenceStorage | null, scope: string | null): boolean {
	if (!storage || !scope) return false;
	try {
		return storage.getItem(uiPreferencesKey(scope)) !== null;
	} catch {
		return false;
	}
}

export function writeUiPreferences(
	storage: UiPreferenceStorage | null,
	scope: string | null,
	preferences: UiPreferences
): boolean {
	if (!storage || !scope) return false;
	try {
		const serialized = JSON.stringify(normalizePreferences(preferences));
		if (serialized.length > MAX_STORED_BYTES) return false;
		storage.setItem(uiPreferencesKey(scope), serialized);
		return true;
	} catch {
		return false;
	}
}
