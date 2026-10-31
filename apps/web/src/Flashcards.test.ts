import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import Flashcards from './Flashcards.svelte';

describe('flashcards component', () => {
	it('renders header, title, and initial empty state when rendered', () => {
		const { body } = render(Flashcards, { props: { mediaId: '018f26a1-0000-7000-8000-000000000001' } });
		expect(body).toContain('Cartões de Memorização');
		expect(body).toContain('Novo Cartão');
		expect(body).toContain('Revisar');
		expect(body).toContain('Gerenciar');
		expect(body).toContain('Nenhum cartão criado para esta aula ainda');
		expect(body).toContain('Crie perguntas e respostas para revisar conceitos-chave usando repetição espaçada.');
	});
});
