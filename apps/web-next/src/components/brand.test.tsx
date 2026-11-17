import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { BrandMark, Motto } from './brand';

afterEach(() => {
	cleanup();
	setLocale('en', null);
});

describe('BrandMark', () => {
	it('is decorative and draws the full portico with a gold door and pediment dot', () => {
		const { container } = render(<BrandMark />);
		const svg = container.querySelector('svg');
		expect(svg?.getAttribute('aria-hidden')).toBe('true');
		expect(container.querySelectorAll('[fill="var(--reward-fill)"]')).toHaveLength(2);
		expect(container.querySelectorAll('[fill="currentColor"]').length).toBeGreaterThan(2);
	});

	it('draws the reduced cut for small sizes: a door and no pediment dot', () => {
		const { container } = render(<BrandMark compact />);
		expect(container.querySelectorAll('[fill="var(--reward-fill)"]')).toHaveLength(1);
		expect(container.querySelector('circle')).toBeNull();
	});
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
