import { render } from 'svelte/server';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { setLocale } from './i18n.svelte';
import Flashcards from './Flashcards.svelte';

describe('flashcards component', () => {
	beforeEach(() => setLocale('en'));
	afterEach(() => setLocale('en'));

	it('renders header, title, and initial empty state in English and pt-BR', () => {
		const enRender = render(Flashcards, { props: { mediaId: '018f26a1-0000-7000-8000-000000000001' } });
		expect(enRender.body).toContain('Flashcards');
		expect(enRender.body).toContain('New Card');
		expect(enRender.body).toContain('Review');
		expect(enRender.body).toContain('Manage');
		expect(enRender.body).toContain('No flashcards created for this lesson yet');
		expect(enRender.body).toContain('Create questions and answers to review key concepts using spaced repetition.');

		setLocale('pt-BR');
		const ptRender = render(Flashcards, { props: { mediaId: '018f26a1-0000-7000-8000-000000000001' } });
		expect(ptRender.body).toContain('Cartões de Memorização');
		expect(ptRender.body).toContain('Novo Cartão');
		expect(ptRender.body).toContain('Revisar');
		expect(ptRender.body).toContain('Gerenciar');
		expect(ptRender.body).toContain('Nenhum cartão criado para esta aula ainda');
		expect(ptRender.body).toContain('Crie perguntas e respostas para revisar conceitos-chave usando repetição espaçada.');
	});
});
