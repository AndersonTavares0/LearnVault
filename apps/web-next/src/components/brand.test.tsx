import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { Motto } from './brand';

afterEach(() => {
	cleanup();
	setLocale('en', null);
});

describe('Motto', () => {
	it('marks the Latin quotation and shows the translation in the learner language', () => {
		render(<Motto name="sapereAude" />);
		expect(screen.getByText('Sapere aude').getAttribute('lang')).toBe('la');
		expect(screen.getByText('Dare to know')).toBeTruthy();
		cleanup();
		setLocale('pt-BR', null);
		render(<Motto name="sapereAude" />);
		expect(screen.getByText('Ouse saber')).toBeTruthy();
	});

	it('sets the Greek line in the reading serif, because Playfair Display has no Greek glyphs', () => {
		render(<Motto name="archeHemisy" />);
		const greek = screen.getByText('ἀρχὴ ἥμισυ παντός');
		expect(greek.getAttribute('lang')).toBe('grc');
		expect(greek.className).toContain('font-reading');
		expect(greek.className).not.toContain('font-heading');
	});
});
