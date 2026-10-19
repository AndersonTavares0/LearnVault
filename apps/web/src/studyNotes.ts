// Regras de texto das notas pessoais que o servidor valida em caracteres.
//
// O `maxlength` do navegador conta unidades UTF-16, então um emoji ocupa duas unidades
// enquanto o servidor conta um caractere. A interface mede o mesmo limite do servidor
// (caracteres) para avisar antes, em vez de deixar o usuário descobrir pelo erro.

/** Limite de caracteres do texto de uma nota; espelha `MAX_NOTE_TEXT_CHARS` do núcleo. */
export const MAX_NOTE_TEXT_CHARS = 20_000;

/** Tamanho do texto em caracteres, a mesma unidade que o servidor e o banco usam. */
export function noteTextChars(value: string): number {
	return [...value].length;
}

/** Verdadeiro quando o texto passa do limite de caracteres aceito pelo servidor. */
export function noteTextTooLarge(value: string): boolean {
	return noteTextChars(value) > MAX_NOTE_TEXT_CHARS;
}
