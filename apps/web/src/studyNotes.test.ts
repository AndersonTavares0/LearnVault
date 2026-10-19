import { describe, expect, it } from 'vitest';
import { MAX_NOTE_TEXT_CHARS, noteTextChars, noteTextTooLarge } from './studyNotes';

describe('limite de texto da nota', () => {
	it('mede o texto em caracteres, a unidade do servidor', () => {
		expect(noteTextChars('abc')).toBe(3);
		// Acento e emoji ocupam um caractere cada, mesmo ocupando dois e quatro bytes.
		expect(noteTextChars('á')).toBe(1);
		expect(noteTextChars('😀')).toBe(1);
	});

	it('aceita o texto até o limite de caracteres do servidor', () => {
		expect(MAX_NOTE_TEXT_CHARS).toBe(20_000);
		expect(noteTextTooLarge('a'.repeat(MAX_NOTE_TEXT_CHARS))).toBe(false);
		expect(noteTextTooLarge('a'.repeat(MAX_NOTE_TEXT_CHARS + 1))).toBe(true);
	});

	it('aceita texto acentuado que em bytes passaria do limite antigo', () => {
		// 20.000 caracteres acentuados são 40.000 bytes: o limite medido em bytes recusaria
		// antes de o usuário chegar ao limite que a interface mostra.
		const acentuado = 'á'.repeat(20_000);
		expect(noteTextTooLarge(acentuado)).toBe(false);
	});

	it('rejeita antes do servidor o texto que passa do limite em caracteres', () => {
		const excedido = 'á'.repeat(20_001);
		expect(noteTextTooLarge(excedido)).toBe(true);
	});
});
