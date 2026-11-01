import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import AllNotes from './AllNotes.svelte';

describe('AllNotes component', () => {
	it('renders notes title, search input and loading state or initial shell', () => {
		const { body } = render(AllNotes, { props: { catalog: null } });
		expect(body).toContain('Minhas Anotações');
		expect(body).toContain('Anotações salvas durante suas aulas em todos os cursos.');
		expect(body).toContain('Buscar em anotações, cursos ou aulas…');
	});
});
