import { describe, expect, it } from 'vitest';
import { buildNotesEntries, courseOptions, createDefaultNotesFilters, filterNotes, groupNotes } from './notes-filter';
import type { Catalog, CatalogNode, FileEntry, StudyNote } from './transport';

function file(name: string, mediaId: string, kind = 'media', availability = 'available'): FileEntry {
	return { name, path: name, size: 10, mediaId, kind, availability };
}

function node(role: string, title: string, path: string, children: CatalogNode[] = [], files: FileEntry[] = [], looseFiles: FileEntry[] = []): CatalogNode {
	return { title, path, role, files, looseFiles, children };
}

// Catálogo de referência: um curso com módulo, um curso com aula direta na raiz e
// uma mídia solta na raiz do primeiro curso.
const catalog: Catalog = {
	complete: true,
	looseFiles: [],
	nodes: [
		node('course', 'Rust', 'rust', [
			node('module', 'Módulo 1', 'rust/mod-1', [
				node('lesson', 'Aula 1', 'rust/mod-1/aula-1', [], [file('a.mp4', 'm1')]),
				node('lesson', 'Aula 2', 'rust/mod-1/aula-2', [], [file('b.mp4', 'm2')])
			])
		], [], [file('solto.mp4', 'm4')]),
		node('course', 'Python', 'python', [node('lesson', 'Aula única', 'python/aula', [], [file('c.mp4', 'm3')])])
	]
};

// Meio-dia local de 10/05/2026: evita que fuso horário empurre notas para outro dia.
const NOW = new Date(2026, 4, 10, 12, 0, 0).getTime() / 1000;

function note(mediaId: string, text: string, updatedAt: number, noteId = 1): StudyNote {
	return { noteId, mediaId, text, positionSeconds: null, createdAt: updatedAt, updatedAt };
}

function entries(notes: StudyNote[]) {
	return buildNotesEntries(notes, catalog);
}

describe('buildNotesEntries', () => {
	it('links each note to its course, module and lesson through the media id', () => {
		const [first, second, third] = entries([note('m1', 'texto', NOW), note('m3', 'texto', NOW), note('m4', 'texto', NOW)]);

		expect(first).toMatchObject({ courseId: 'rust', courseTitle: 'Rust', moduleTitle: 'Módulo 1', lessonTitle: 'Aula 1', lessonPath: 'rust/mod-1/aula-1' });
		expect(second).toMatchObject({ courseId: 'python', courseTitle: 'Python', moduleTitle: null, lessonTitle: 'Aula única' });
		// Mídia solta na raiz do curso continua avulsa: título limpo e sem módulo.
		expect(third).toMatchObject({ courseId: 'rust', moduleTitle: null, lessonTitle: 'solto', lessonPath: 'solto.mp4' });
	});

	it('keeps notes whose media left the catalog instead of hiding them', () => {
		const [orphan] = entries([note('m9', 'texto', NOW)]);

		expect(orphan).toMatchObject({
			courseId: 'media:m9',
			courseTitle: 'Aulas fora do catálogo',
			moduleTitle: null,
			lessonTitle: 'Aula não encontrada no catálogo',
			lessonPath: null
		});
	});

	it('tolerates a catalog that has not loaded yet', () => {
		const [entry] = buildNotesEntries([note('m1', 'texto', NOW)], null);
		expect(entry.courseId).toBe('media:m1');
	});
});

describe('filterNotes', () => {
	const filters = createDefaultNotesFilters();

	it('searches without accents and without case', () => {
		const list = entries([note('m1', 'Física Quântica', NOW), note('m2', 'álgebra linear', NOW)]);

		expect(filterNotes(list, { ...filters, query: 'fisica quantica' }, 'recent', NOW).map((entry) => entry.note.mediaId)).toEqual(['m1']);
		expect(filterNotes(entries([note('m1', 'Resumo de FÍSICA', NOW)]), { ...filters, query: 'física' }, 'recent', NOW)).toHaveLength(1);
	});

	it('matches the lesson and course titles too', () => {
		const list = entries([note('m1', 'sem pista', NOW), note('m3', 'sem pista', NOW)]);

		expect(filterNotes(list, { ...filters, query: 'aula 1' }, 'recent', NOW)).toHaveLength(1);
		expect(filterNotes(list, { ...filters, query: 'python' }, 'recent', NOW).map((entry) => entry.note.mediaId)).toEqual(['m3']);
	});

	it('filters by course', () => {
		const list = entries([note('m1', 'a', NOW), note('m3', 'b', NOW)]);
		expect(filterNotes(list, { ...filters, course: 'python' }, 'recent', NOW).map((entry) => entry.note.mediaId)).toEqual(['m3']);
	});

	it('understands today, 7 and 30 days from local midnight', () => {
		const list = entries([
			note('m1', 'hoje', NOW - 3600),
			note('m2', 'ontem', NOW - 86_400),
			note('m3', 'cinco dias', NOW - 5 * 86_400),
			note('m4', 'quarenta dias', NOW - 40 * 86_400)
		]);

		expect(filterNotes(list, { ...filters, period: 'today' }, 'recent', NOW).map((entry) => entry.note.text)).toEqual(['hoje']);
		expect(filterNotes(list, { ...filters, period: '7' }, 'recent', NOW).map((entry) => entry.note.text)).toEqual(['hoje', 'ontem', 'cinco dias']);
		expect(filterNotes(list, { ...filters, period: '30' }, 'recent', NOW).map((entry) => entry.note.text)).toEqual(['hoje', 'ontem', 'cinco dias']);
		expect(filterNotes(list, { ...filters, period: 'all' }, 'recent', NOW)).toHaveLength(4);
	});

	it('accepts a custom date range and ignores an incomplete one', () => {
		const third = new Date(2026, 4, 3, 9, 0, 0).getTime() / 1000;
		const list = entries([note('m1', 'no intervalo', third), note('m2', 'fora', NOW)]);

		expect(filterNotes(list, { ...filters, period: 'custom', from: '2026-05-01', to: '2026-05-05' }, 'recent', NOW).map((entry) => entry.note.text)).toEqual(['no intervalo']);
		expect(filterNotes(list, { ...filters, period: 'custom', from: '', to: '' }, 'recent', NOW)).toHaveLength(2);
	});

	it('orders by most recent by default and by oldest when asked', () => {
		const list = entries([note('m1', 'antiga', NOW - 100, 1), note('m2', 'nova', NOW - 10, 2)]);

		expect(filterNotes(list, filters, 'recent', NOW).map((entry) => entry.note.text)).toEqual(['nova', 'antiga']);
		expect(filterNotes(list, filters, 'oldest', NOW).map((entry) => entry.note.text)).toEqual(['antiga', 'nova']);
	});
});

describe('groupNotes', () => {
	it('groups by course in alphabetical order', () => {
		const list = entries([note('m1', 'a', NOW), note('m3', 'b', NOW), note('m9', 'c', NOW)]);
		expect(groupNotes(list, 'course', 'recent', NOW).map((group) => group.title)).toEqual(['Aulas fora do catálogo', 'Python', 'Rust']);
	});

	it('groups by day with Hoje and Ontem labels', () => {
		const list = entries([note('m1', 'hoje', NOW - 10), note('m2', 'ontem', NOW - 86_400), note('m3', 'hoje de novo', NOW - 3600)]);
		const groups = groupNotes(list, 'date', 'recent', NOW);

		expect(groups.map((group) => group.title)).toEqual(['Hoje', 'Ontem']);
		expect(groups[0].entries).toHaveLength(2);
		expect(groups[0].key).toBe('2026-05-10');
		expect(groups[1].key).toBe('2026-05-09');
	});
});

describe('courseOptions', () => {
	it('counts the notes per course and sorts by title', () => {
		const list = entries([note('m1', 'a', NOW), note('m2', 'b', NOW), note('m3', 'c', NOW), note('m9', 'd', NOW)]);

		expect(courseOptions(list)).toEqual([
			{ id: 'media:m9', title: 'Aulas fora do catálogo', count: 1 },
			{ id: 'python', title: 'Python', count: 1 },
			{ id: 'rust', title: 'Rust', count: 2 }
		]);
	});
});
