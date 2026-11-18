/**
 * Generated cover of a course, with no images: colour and typography derived from the title.
 *
 * The look starts from LearnVault's editorial identity — warm paper, deep green and
 * terracotta — restricting the hue to an earthy band and using the serif monogram
 * as the graphic element. The function is deterministic: the same title always produces the same
 * cover, and two courses with different titles almost always produce distinguishable covers.
 */

/** Cloth colors for generated covers: moss, olive, ochre, brick, ink blue, slate teal and plum. */
const HUES = [28, 38, 14, 140, 100, 195, 345];

export type CourseCoverPattern = 'grid' | 'arcs' | 'lines' | 'dots' | 'blocks' | 'wave';

export type CourseCover = {
	/** Starting colour of the gradient. */
	from: string;
	/** End colour of the gradient. */
	to: string;
	/** Iniciais exibidas na capa. */
	monogram: string;
	/**
	 * Angle of the gradient in degrees, deterministic per title.
	 *
	 * Varies between 95° and 200° so that each cover has its own readable tilt,
	 * without depending on randomness or on render order.
	 */
	angle: number;
	/** Geometric pattern of the cover, deterministic per title. */
	pattern: CourseCoverPattern;
};

/** 32-bit FNV-1a hash, stable across runs. */
function hashTitle(value: string): number {
	let hash = 0x811c9dc5;
	for (let index = 0; index < value.length; index += 1) {
		hash ^= value.charCodeAt(index);
		hash = Math.imul(hash, 0x01000193);
	}
	return hash >>> 0;
}

/** Connecting words ignored in the initials, so the monogram stays meaningful. */
const STOPWORDS = new Set(['a', 'o', 'as', 'os', 'de', 'da', 'do', 'das', 'dos', 'e', 'em', 'com', 'para', 'por', 'no', 'na']);

/** Normalizes the title for comparison and initials, removing accents. */
function normalize(value: string): string {
	return value.normalize('NFD').replace(/\p{Diacritic}/gu, '').trim();
}

/** Initials: up to two relevant words (with letters, no connectives), in capitals. */
export function courseMonogram(title: string): string {
	const words = normalize(title)
		.split(/[\s._-]+/)
		.map((word) => word.replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, ''))
		.filter((word) => /\p{L}/u.test(word) && !STOPWORDS.has(word.toLowerCase()))
		.slice(0, 2);
	const initials = words.map((word) => word[0]?.toUpperCase() ?? '').join('');
	// A year-named course ("2010") has no initials; its digits are the title on the cover.
	const digits = normalize(title).replace(/\D/g, '').slice(0, 4);
	return initials || digits || '?';
}

/** Effective source of the cover, in the order of precedence defined in ADR 0002. */
export type CourseCoverSource = 'manual' | 'thumbnail' | 'generated';

/**
 * Resolves the cover to show: a custom cover takes precedence, followed by the automatic
 * thumbnail and, as a fallback, the cover generated from the title. Empty values count as
 * absent, so the future upload source (issue #237) only needs to supply a URL.
 */
export function selectCourseCoverSource(input: {
	manualCoverUrl?: string | null;
	thumbnailUrl?: string | null;
}): CourseCoverSource {
	if (input.manualCoverUrl) return 'manual';
	if (input.thumbnailUrl) return 'thumbnail';
	return 'generated';
}

const PATTERNS: CourseCoverPattern[] = ['grid', 'arcs', 'lines', 'dots', 'blocks', 'wave'];

/** Deterministic cover of the course. */
export function courseCover(title: string): CourseCover {
	const hash = hashTitle(normalize(title).toLowerCase());
	const hue = HUES[hash % HUES.length];
	const shift = 8 + (hash % 8);
	// High bits of the hash feed the angle, so that it varies even when the hue repeats.
	const angle = 95 + ((hash >>> 8) % 106);
	return {
		from: `hsl(${hue} 30% 34%)`,
		to: `hsl(${(hue + shift) % 360} 34% 22%)`,
		monogram: courseMonogram(title),
		angle,
		pattern: PATTERNS[(hash >>> 16) % PATTERNS.length]
	};
}
