import { describe, expect, it } from 'vitest';
import {
	applyLocale,
	DEFAULT_LOCALE,
	getLocale,
	LOCALES,
	parseLocale,
	persistLocale,
	readLocale,
	setLocale,
	tr
} from './i18n.svelte';

describe('i18n helpers and state', () => {
	it('defines supported locales with english as default', () => {
		expect(LOCALES).toEqual(['en', 'pt-BR']);
		expect(DEFAULT_LOCALE).toBe('en');
	});

	it('parses valid locale strings and normalizes variants', () => {
		expect(parseLocale('en')).toBe('en');
		expect(parseLocale('en-US')).toBe('en');
		expect(parseLocale('en-GB')).toBe('en');
		expect(parseLocale('pt-BR')).toBe('pt-BR');
		expect(parseLocale('pt-br')).toBe('pt-BR');
		expect(parseLocale('pt')).toBe('pt-BR');
		expect(parseLocale('fr')).toBeNull();
		expect(parseLocale(null)).toBeNull();
		expect(parseLocale(123)).toBeNull();
	});

	it('reads locale from storage, fallback, or default', () => {
		const mockStorage = {
			data: {} as Record<string, string>,
			getItem(key: string) {
				return this.data[key] ?? null;
			},
			setItem(key: string, val: string) {
				this.data[key] = val;
			}
		};

		expect(readLocale(mockStorage, 'pt-BR')).toBe('pt-BR');
		expect(readLocale(mockStorage, null)).toBe('en');

		mockStorage.setItem('learnvault-locale', 'pt-BR');
		expect(readLocale(mockStorage, 'en')).toBe('pt-BR');
	});

	it('persists locale safely and tolerates storage errors', () => {
		const storage = {
			saved: '',
			setItem(_k: string, val: string) {
				this.saved = val;
			}
		};
		expect(persistLocale(storage, 'pt-BR')).toBe(true);
		expect(storage.saved).toBe('pt-BR');

		const failingStorage = {
			setItem() {
				throw new Error('QuotaExceeded');
			}
		};
		expect(persistLocale(failingStorage, 'en')).toBe(false);
	});

	it('applies locale to HTML document element lang attribute', () => {
		const element = { lang: 'en' } as HTMLElement;
		applyLocale(element, 'pt-BR');
		expect(element.lang).toBe('pt-BR');
	});

	it('reactively switches locale and translates strings with tr()', () => {
		setLocale('en');
		expect(getLocale()).toBe('en');
		expect(tr('Courses', 'Cursos')).toBe('Courses');

		setLocale('pt-BR');
		expect(getLocale()).toBe('pt-BR');
		expect(tr('Courses', 'Cursos')).toBe('Cursos');

		// Reset back to English default
		setLocale('en');
		expect(getLocale()).toBe('en');
	});
});
