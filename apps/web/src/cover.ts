/**
 * Capa gerada de um curso, sem imagens: cor e tipografia derivadas do título.
 *
 * O visual parte da identidade Graphite do LearnVault — neutros frios e acento
 * petróleo — restringindo o matiz a uma faixa fria e usando o monograma como
 * elemento gráfico. A função é determinística: o mesmo título produz sempre a mesma
 * capa, e dois cursos com títulos diferentes quase sempre produzem capas distinguíveis.
 */

/** Matizes canônicos da paleta do produto: petróleo, turquesa e azul-acinzentado frio. */
const HUES = [188, 196, 204, 182, 210];

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
