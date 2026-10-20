/**
 * Gradientes gerados a partir das três capas da prancha Graphite 01.
 * Mantém as capas determinísticas sem substituir capas manuais ou miniaturas.
 */

const PALETTES = [
	['#295c67', '#a7c9bd'],
	['#695e4e', '#d4b77e'],
	['#3e4160', '#a4a6d5']
] as const;

export type CourseCover = {
	/** Cor inicial do gradiente. */
	from: string;
	/** Cor final do gradiente. */
	to: string;
	/** Iniciais exibidas na capa. */
	monogram: string;
	/** Direção fixa do gradiente original Graphite. */
	angle: number;
};

/** Hash FNV-1a de 32 bits, estável entre execuções. */
function hashTitle(value: string): number {
	let hash = 0x811c9dc5;
	for (let index = 0; index < value.length; index += 1) {
		hash ^= value.charCodeAt(index);
		hash = Math.imul(hash, 0x01000193);
	}
	return hash >>> 0;
}

/** Palavras de ligação ignoradas nas iniciais, para o monograma ficar significativo. */
const STOPWORDS = new Set(['a', 'o', 'as', 'os', 'de', 'da', 'do', 'das', 'dos', 'e', 'em', 'com', 'para', 'por', 'no', 'na']);

/** Normaliza o título para comparação e iniciais, removendo acentos. */
function normalize(value: string): string {
	return value.normalize('NFD').replace(/\p{Diacritic}/gu, '').trim();
}

/** Iniciais: até duas palavras relevantes (com letras, sem ligações), em maiúsculas. */
export function courseMonogram(title: string): string {
	const words = normalize(title)
		.split(/[\s._-]+/)
		.filter((word) => /\p{L}/u.test(word) && !STOPWORDS.has(word.toLowerCase()))
		.slice(0, 2);
	const initials = words.map((word) => word[0]?.toUpperCase() ?? '').join('');
	return initials || '?';
}

/** Fonte efetiva da capa, na ordem de precedência definida na ADR 0002. */
export type CourseCoverSource = 'manual' | 'thumbnail' | 'generated';

/**
 * Resolve a capa a exibir: capa personalizada tem precedência, seguida da miniatura
 * automática e, como fallback, da capa gerada pelo título. Valores vazios contam como
 * ausentes, então a futura fonte de upload (issue #237) só precisa informar uma URL.
 */
export function selectCourseCoverSource(input: {
	manualCoverUrl?: string | null;
	thumbnailUrl?: string | null;
}): CourseCoverSource {
	if (input.manualCoverUrl) return 'manual';
	if (input.thumbnailUrl) return 'thumbnail';
	return 'generated';
}

/** Capa determinística usando a paleta original Graphite 01. */
export function courseCover(title: string): CourseCover {
	const palette = PALETTES[hashTitle(normalize(title).toLowerCase()) % PALETTES.length];
	return {
		from: palette[0],
		to: palette[1],
		monogram: courseMonogram(title),
		angle: 140
	};
}
