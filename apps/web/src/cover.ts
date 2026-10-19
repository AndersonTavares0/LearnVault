/**
 * Capa gerada de um curso, sem imagens: cor e tipografia derivadas do título.
 *
 * O visual parte da identidade editorial do LearnVault — papel quente, verde profundo e
 * terracota — restringindo o matiz a uma faixa terrosa e usando o monograma em serifa
 * como elemento gráfico. A função é determinística: o mesmo título produz sempre a mesma
 * capa, e dois cursos com títulos diferentes quase sempre produzem capas distinguíveis.
 */

/** Matizes canônicos da paleta do produto: verde, oliva, âmbar e terracota. */
const HUES = [158, 130, 86, 40, 20];

export type CourseCover = {
	/** Cor inicial do gradiente. */
	from: string;
	/** Cor final do gradiente. */
	to: string;
	/** Iniciais exibidas na capa. */
	monogram: string;
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

/** Capa determinística do curso. */
export function courseCover(title: string): CourseCover {
	const hash = hashTitle(normalize(title).toLowerCase());
	const hue = HUES[hash % HUES.length];
	const shift = 8 + (hash % 8);
	return {
		from: `hsl(${hue} 26% 31%)`,
		to: `hsl(${(hue + shift) % 360} 32% 17%)`,
		monogram: courseMonogram(title)
	};
}
