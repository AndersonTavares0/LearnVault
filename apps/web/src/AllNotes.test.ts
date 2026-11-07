import { render } from 'svelte/server';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setLocale } from './i18n.svelte';
import AllNotes from './AllNotes.svelte';

describe('AllNotes component', () => {
	beforeEach(() => setLocale('en'));
	afterEach(() => setLocale('en'));

	it('renders notes title, search input and description in English and pt-BR', () => {
		const enRender = render(AllNotes, { props: { catalog: null } });
		expect(enRender.body).toContain('My Notes');
		expect(enRender.body).toContain('Notes saved during your lessons across all courses.');
		expect(enRender.body).toContain('Search in notes, courses, or lessons…');

		setLocale('pt-BR');
		const ptRender = render(AllNotes, { props: { catalog: null } });
		expect(ptRender.body).toContain('Minhas Anotações');
		expect(ptRender.body).toContain('Anotações salvas durante suas aulas em todos os cursos.');
		expect(ptRender.body).toContain('Buscar em anotações, cursos ou aulas…');
	});
});
