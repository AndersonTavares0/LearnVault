import { describe, expect, it } from 'vitest';
import { countCatalogFiles, countCatalogNode, courseLessonEntries, formatCount, formatCourseCardMeta, formatCourseMeta, formatLessonCounts, formatLessonMeta, isLessonMissing } from './catalog';
import type { CatalogCounts } from './catalog';
import { createDefaultPlayerPreferences, updateLessonPreference } from './player-preferences';
import { courseIdOf, type CatalogNode, type FileEntry } from './transport';

function file(name: string, kind: string, availability = 'available'): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind, availability };
}

function missing(name: string, kind: string): FileEntry {
	return file(name, kind, 'missing');
}

function node(role: string, title: string, children: CatalogNode[] = [], files: FileEntry[] = []): CatalogNode {
	return { title, path: title, role, files, looseFiles: [], children };
}

/** Contagens de aulas isoladas: o resto fica zerado para não interferir no formato. */
function tally(lessons: number, missingLessons: number): CatalogCounts {
	return { modules: 0, lessons, missingLessons, videos: 0, materials: 0 };
}

const course = node('course', 'Rust', [
	node('module', 'Módulo 1', [
		node('lesson', 'Aula 1', [], [file('aula.mp4', 'media'), file('aula.srt', 'subtitle')]),
		node('lesson', 'Aula 2', [], [file('slide.pdf', 'material')])
	]),
	node('module', 'Módulo 2', [node('lesson', 'Aula 3', [], [file('aula-3.mp4', 'media'), file('aula-3.mp3', 'media')])]),
	node('lesson', 'Aula solta', [], [file('solto.bin', 'outro')])
]);

describe('countCatalogNode', () => {
	it('counts modules, lessons, videos and materials of the whole subtree', () => {
		expect(countCatalogNode(course)).toEqual({ modules: 2, lessons: 4, missingLessons: 0, videos: 3, materials: 2 });
	});

	it('counts the node itself when it is a lesson and ignores unknown kinds', () => {
		expect(countCatalogNode(node('lesson', 'Aula', [], [file('a.mp4', 'media'), file('x.bin', 'outro')]))).toEqual({ modules: 0, lessons: 1, missingLessons: 0, videos: 1, materials: 0 });
	});

	it('counts loose files of the node and of its children', () => {
		const category = node('category', 'TI', [node('lesson', 'Aula')], [file('nota.pdf', 'material')]);
		category.children[0].looseFiles = [file('extra.srt', 'subtitle')];
		expect(countCatalogNode(category)).toEqual({ modules: 0, lessons: 1, missingLessons: 0, videos: 0, materials: 2 });
	});

	it('returns zeros for an empty node', () => {
		expect(countCatalogNode(node('category', 'Vazia'))).toEqual({ modules: 0, lessons: 0, missingLessons: 0, videos: 0, materials: 0 });
	});
});

describe('countCatalogFiles', () => {
	it('counts only the files of the node itself', () => {
		const parent = node('lesson', 'Aula', [], [file('a.mp4', 'media')]);
		parent.children = [node('lesson', 'Bônus', [], [file('b.mp4', 'media'), file('b.pdf', 'material')])];
		expect(countCatalogFiles(parent)).toEqual({ videos: 1, materials: 0 });
		expect(countCatalogFiles(parent.children[0])).toEqual({ videos: 1, materials: 1 });
	});
});

describe('isLessonMissing', () => {
	it('is false for a lesson whose media is available', () => {
		expect(isLessonMissing(node('lesson', 'Aula', [], [file('a.mp4', 'media')]))).toBe(false);
	});

	it('is true when every media file of the lesson is missing', () => {
		expect(isLessonMissing(node('lesson', 'Aula', [], [missing('a.mp4', 'media')]))).toBe(true);
		expect(isLessonMissing(node('lesson', 'Aula', [], [missing('a.mp4', 'media'), missing('b.mp4', 'media')]))).toBe(true);
	});

	it('is true for a mixed lesson whose media is missing and whose subtitle is available', () => {
		// Decisão: só a mídia decide. A legenda sobrevivente não torna a aula reproduzível,
		// então a aula continua ausente — é o arquivo de mídia que a linha abre no player.
		expect(isLessonMissing(node('lesson', 'Aula', [], [missing('a.mp4', 'media'), file('a.srt', 'subtitle')]))).toBe(true);
	});

	it('is false when at least one media file is still available', () => {
		expect(isLessonMissing(node('lesson', 'Aula', [], [missing('a.mp4', 'media'), file('b.mp4', 'media')]))).toBe(false);
	});

	it('is false for a lesson without any media file', () => {
		expect(isLessonMissing(node('lesson', 'Aula', [], [file('slide.pdf', 'material'), file('a.srt', 'subtitle')]))).toBe(false);
		expect(isLessonMissing(node('lesson', 'Aula'))).toBe(false);
	});

	it('reads availability tolerantly and looks at loose files too', () => {
		const lesson = node('lesson', 'Aula', [], [{ ...missing('a.mp4', 'media'), availability: ' MISSING ' }]);
		expect(isLessonMissing(lesson)).toBe(true);
		const loose = node('lesson', 'Aula');
		loose.looseFiles = [missing('a.mp4', 'media')];
		expect(isLessonMissing(loose)).toBe(true);
	});
});

describe('formatCount', () => {
	it('uses the singular for exactly one and the plural otherwise', () => {
		expect(formatCount(1, 'aula', 'aulas')).toBe('1 aula');
		expect(formatCount(2, 'aula', 'aulas')).toBe('2 aulas');
		expect(formatCount(0, 'material', 'materiais')).toBe('0 materiais');
	});
});

describe('formatLessonCounts', () => {
	it('shows the available lessons and appends the missing ones when they exist', () => {
		expect(formatLessonCounts(tally(4, 0))).toBe('4 aulas');
		expect(formatLessonCounts(tally(4, 4))).toBe('4 aulas · 4 ausentes');
		expect(formatLessonCounts(tally(1, 3))).toBe('1 aula · 3 ausentes');
	});

	it('keeps the available count at zero instead of hiding a fully missing module', () => {
		expect(formatLessonCounts(tally(0, 4))).toBe('0 aulas · 4 ausentes');
		expect(formatLessonCounts(tally(0, 1))).toBe('0 aulas · 1 ausente');
		expect(formatLessonCounts(tally(0, 0))).toBe('0 aulas');
	});
});

describe('formatCourseMeta', () => {
	it('joins every non-zero count in a stable order', () => {
		expect(formatCourseMeta({ modules: 3, lessons: 12, missingLessons: 0, videos: 20, materials: 4 })).toBe('3 módulos · 12 aulas · 4 materiais');
	});

	it('omits zero parts and uses the singular for one', () => {
		expect(formatCourseMeta({ modules: 1, lessons: 1, missingLessons: 0, videos: 0, materials: 1 })).toBe('1 módulo · 1 aula · 1 material');
		expect(formatCourseMeta({ modules: 0, lessons: 25, missingLessons: 0, videos: 25, materials: 180 })).toBe('25 aulas · 180 materiais');
	});

	it('reports available and missing lessons side by side', () => {
		expect(formatCourseMeta({ modules: 2, lessons: 4, missingLessons: 4, videos: 8, materials: 3 })).toBe('2 módulos · 4 aulas · 4 ausentes · 3 materiais');
		expect(formatCourseMeta({ modules: 1, lessons: 0, missingLessons: 3, videos: 3, materials: 0 })).toBe('1 módulo · 0 aulas · 3 ausentes');
	});

	it('returns an empty string when there is nothing to count', () => {
		expect(formatCourseMeta({ modules: 0, lessons: 0, missingLessons: 0, videos: 0, materials: 0 })).toBe('');
	});
});

describe('formatCourseCardMeta', () => {
	it('keeps only modules and lessons for the card caption', () => {
		expect(formatCourseCardMeta({ modules: 3, lessons: 12, missingLessons: 0, videos: 20, materials: 4 })).toBe('3 módulos · 12 aulas');
		expect(formatCourseCardMeta({ modules: 0, lessons: 25, missingLessons: 0, videos: 25, materials: 180 })).toBe('25 aulas');
		expect(formatCourseCardMeta({ modules: 2, lessons: 0, missingLessons: 0, videos: 0, materials: 9 })).toBe('2 módulos');
	});

	it('shows the missing lessons on the card instead of dropping them', () => {
		expect(formatCourseCardMeta({ modules: 2, lessons: 4, missingLessons: 4, videos: 0, materials: 0 })).toBe('2 módulos · 4 aulas · 4 ausentes');
		expect(formatCourseCardMeta({ modules: 0, lessons: 0, missingLessons: 2, videos: 0, materials: 0 })).toBe('0 aulas · 2 ausentes');
	});

	it('returns an empty string for a course without structure yet', () => {
		expect(formatCourseCardMeta({ modules: 0, lessons: 0, missingLessons: 0, videos: 0, materials: 0 })).toBe('');
	});
});

describe('formatLessonMeta', () => {
	it('shows videos and materials with correct plurals', () => {
		expect(formatLessonMeta({ videos: 2, materials: 3 })).toBe('2 vídeos · 3 materiais');
		expect(formatLessonMeta({ videos: 1, materials: 1 })).toBe('1 vídeo · 1 material');
	});

	it('omits zero parts and returns an empty string when silent', () => {
		expect(formatLessonMeta({ videos: 0, materials: 5 })).toBe('5 materiais');
		expect(formatLessonMeta({ videos: 4, materials: 0 })).toBe('4 vídeos');
		expect(formatLessonMeta({ videos: 0, materials: 0 })).toBe('');
	});
});

describe('lesson counting with missing media', () => {
	const mixedModule = node('module', 'Módulo 1', [
		node('lesson', 'Aula 1', [], [file('aula-1.mp4', 'media')]),
		node('lesson', 'Aula 2', [], [file('aula-2.mp4', 'media')]),
		node('lesson', 'Aula 3', [], [file('aula-3.mp4', 'media')]),
		node('lesson', 'Aula 4', [], [file('aula-4.mp4', 'media')]),
		node('lesson', 'Aula 5', [], [missing('aula-5.mp4', 'media')]),
		node('lesson', 'Aula 6', [], [missing('aula-6.mp4', 'media'), file('aula-6.srt', 'subtitle')]),
		node('lesson', 'Aula 7', [], [missing('aula-7.mp4', 'media')]),
		node('lesson', 'Aula 8', [], [missing('aula-8.mp4', 'media')])
	]);

	it('splits available and missing lessons instead of dropping the missing ones', () => {
		expect(countCatalogNode(mixedModule)).toMatchObject({ modules: 1, lessons: 4, missingLessons: 4 });
		expect(formatLessonCounts(countCatalogNode(mixedModule))).toBe('4 aulas · 4 ausentes');
	});

	it('counts a lesson without media as an available lesson', () => {
		const module = node('module', 'Módulo', [node('lesson', 'Só material', [], [file('slide.pdf', 'material')])]);
		expect(countCatalogNode(module)).toMatchObject({ lessons: 1, missingLessons: 0 });
	});

	it('keeps the meta readable when every lesson is missing', () => {
		const onlyMissing = node('course', 'Curso', [node('module', 'Módulo', [node('lesson', 'Aula', [], [missing('aula.mp4', 'media')])])], [file('aviso.pdf', 'material')]);
		const counts = countCatalogNode(onlyMissing);
		// As somas de arquivos continuam descrevendo o que está catalogado; a aula ausente
		// aparece na própria parte de aulas, sem sumir da meta nem virar rótulo vazio.
		expect(counts).toEqual({ modules: 1, lessons: 0, missingLessons: 1, videos: 1, materials: 1 });
		expect(formatCourseMeta(counts)).toBe('1 módulo · 0 aulas · 1 ausente · 1 material');
		expect(formatCourseCardMeta(counts)).toBe('1 módulo · 0 aulas · 1 ausente');
	});

	it('restores the lesson when a later index reports the media as available again', () => {
		const media = missing('aula.mp4', 'media');
		const catalog = node('course', 'Curso', [node('module', 'Módulo', [node('lesson', 'Aula', [], [media])])]);
		expect(countCatalogNode(catalog)).toMatchObject({ lessons: 0, missingLessons: 1 });
		expect(formatLessonCounts(countCatalogNode(catalog))).toBe('0 aulas · 1 ausente');
		// A reindexação regrava `availability = 'available'` quando o arquivo volta ao disco
		// (crates/core/src/rescan.rs). O cliente deriva tudo do payload que recebe: a aula
		// volta a ser contada como disponível e o selo some sozinho.
		media.availability = 'available';
		expect(isLessonMissing(catalog.children[0].children[0])).toBe(false);
		expect(countCatalogNode(catalog)).toMatchObject({ lessons: 1, missingLessons: 0 });
		expect(formatLessonCounts(countCatalogNode(catalog))).toBe('1 aula');
	});
});

describe('courseLessonEntries', () => {
	it('lists every lesson in catalog order with the media the playlist will open', () => {
		const course = node('course', 'Rust', [
			node('module', 'Módulo 1', [
				node('lesson', 'Aula 1', [], [file('a.mp4', 'media'), file('b.mp4', 'media')]),
				node('lesson', 'Aula 2', [], [missing('c.mp4', 'media')])
			]),
			node('lesson', 'Aula solta', [], [file('d.mp4', 'media')])
		]);

		expect(courseLessonEntries(course)).toEqual([
			{ path: 'Aula 1', title: 'Aula 1', mediaId: 'a.mp4' },
			{ path: 'Aula 2', title: 'Aula 2', mediaId: null },
			{ path: 'Aula solta', title: 'Aula solta', mediaId: 'd.mp4' }
		]);
	});

	it('honours the lesson media preference so the checkbox follows the lesson, not the first file', () => {
		const course = node('course', 'Rust', [node('lesson', 'Aula 1', [], [file('a.mp4', 'media'), file('b.mp4', 'media')])]);
		const preferences = updateLessonPreference(createDefaultPlayerPreferences(), courseIdOf(course), 'Aula 1', { mediaId: 'b.mp4' });

		expect(courseLessonEntries(course, preferences)[0].mediaId).toBe('b.mp4');
	});

	it('returns nothing for a course without lessons yet', () => {
		expect(courseLessonEntries(node('course', 'Vazio'))).toEqual([]);
	});
});
