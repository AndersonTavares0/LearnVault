export const LOCALES = ['en', 'pt-BR'] as const;
export type Locale = (typeof LOCALES)[number];
export const DEFAULT_LOCALE: Locale = 'en';
export const LOCALE_STORAGE_KEY = 'learnvault-locale';

export const LOCALE_LABELS: Record<Locale, string> = {
	en: 'English',
	'pt-BR': 'Português (Brasil)'
};

export function parseLocale(value: unknown): Locale | null {
	if (typeof value !== 'string') return null;
	const trimmed = value.trim().toLowerCase();
	if (trimmed === 'en' || trimmed.startsWith('en-')) return 'en';
	if (trimmed === 'pt-br' || trimmed === 'pt' || trimmed.startsWith('pt-')) return 'pt-BR';
	return null;
}

export function readLocale(
	storage?: Pick<Storage, 'getItem'> | null,
	fallbackLanguage?: string | null
): Locale {
	try {
		const parsed = parseLocale(storage?.getItem(LOCALE_STORAGE_KEY));
		if (parsed) return parsed;
	} catch {
		// Ignore storage access errors
	}
	if (fallbackLanguage) {
		const parsedFallback = parseLocale(fallbackLanguage);
		if (parsedFallback) return parsedFallback;
	}
	return DEFAULT_LOCALE;
}

export function persistLocale(storage: Pick<Storage, 'setItem'> | null | undefined, locale: Locale): boolean {
	if (!storage) return false;
	try {
		storage.setItem(LOCALE_STORAGE_KEY, locale);
		return true;
	} catch {
		return false;
	}
}

export function applyLocale(root: HTMLElement | null | undefined, locale: Locale): void {
	if (root) root.lang = locale;
}

function safeStorage(): Storage | null {
	try {
		return typeof window !== 'undefined' ? window.localStorage : null;
	} catch {
		return null;
	}
}

let currentLocale: Locale = readLocale(safeStorage());

export function getLocale(): Locale {
	return currentLocale;
}

export function setLocale(
	locale: Locale,
	storage: Pick<Storage, 'setItem'> | null | undefined = safeStorage(),
	root?: HTMLElement | null
): Locale {
	currentLocale = locale;
	persistLocale(storage, locale);
	if (root) {
		applyLocale(root, locale);
	} else if (typeof document !== 'undefined') {
		applyLocale(document.documentElement, locale);
	}
	return locale;
}

export function tr(en: string, ptBr: string, locale?: Locale): string {
	return (locale ?? currentLocale) === 'pt-BR' ? ptBr : en;
}
