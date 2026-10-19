import { describe, expect, it } from 'vitest';
import { MAX_NOTE_TEXT_BYTES, noteTextBytes, noteTextTooLarge } from './studyNotes';

describe('limite de texto da nota', () => {
	it('mede o texto em bytes UTF-8, a unidade do servidor', () => {
		expect(noteTextBytes('abc')).toBe(3);
		// Acento ocupa dois bytes; emoji ocupa quatro.
		expect(noteTextBytes('á')).toBe(2);
		expect(noteTextBytes('😀')).toBe(4);
	});

	it('aceita o texto até o limite de bytes do servidor', () => {
		expect(MAX_NOTE_TEXT_BYTES).toBe(20_000);
		expect(noteTextTooLarge('a'.repeat(MAX_NOTE_TEXT_BYTES))).toBe(false);
		expect(noteTextTooLarge('a'.repeat(MAX_NOTE_TEXT_BYTES + 1))).toBe(true);
	});

	it('rejeita antes do servidor o texto que só passaria pelo maxlength do navegador', () => {
		// 10.001 caracteres acentuados: 10.001 unidades UTF-16 (aceitas pelo `maxlength`),
		// mas 20.002 bytes — acima do limite do servidor.
		const acentuado = 'á'.repeat(10_001);
		expect(acentuado.length).toBeLessThan(MAX_NOTE_TEXT_BYTES);
		expect(noteTextTooLarge(acentuado)).toBe(true);
	});
});
