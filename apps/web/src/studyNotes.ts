// Regras de texto das notas pessoais que o servidor valida em bytes UTF-8.
//
// O `maxlength` do navegador conta unidades UTF-16, então um texto acentuado passa pelo
// atributo e é recusado pelo servidor com 422. A interface mede o mesmo limite do servidor
// para avisar antes, em vez de deixar o usuário descobrir pelo erro.

/** Limite de bytes UTF-8 do texto de uma nota; espelha `MAX_NOTE_TEXT_BYTES` do núcleo. */
export const MAX_NOTE_TEXT_BYTES = 20_000;

const encoder = new TextEncoder();

/** Tamanho do texto em bytes UTF-8, a mesma unidade que o servidor e o banco usam. */
export function noteTextBytes(value: string): number {
	return encoder.encode(value).length;
}

/** Verdadeiro quando o texto passa do limite de bytes aceito pelo servidor. */
export function noteTextTooLarge(value: string): boolean {
	return noteTextBytes(value) > MAX_NOTE_TEXT_BYTES;
}
