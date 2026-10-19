import { findLessonLocation } from './router';
import type { Catalog, StudyNote } from './transport';

export type NotesPeriod = 'all' | 'today' | '7' | '30' | 'custom';
export type NotesSort = 'recent' | 'oldest';
export type NotesView = 'course' | 'date';

export type NotesFilters = {
	query: string;
	course: string;
	period: NotesPeriod;
	from: string;
	to: string;
};

export type NotesEntry = {
	note: StudyNote;
	courseId: string;
	courseTitle: string;
	moduleTitle: string | null;
	lessonTitle: string;
	lessonPath: string | null;
};

export type NotesGroup = { key: string; title: string; entries: NotesEntry[] };

export function createDefaultNotesFilters(): NotesFilters {
	return { query: '', course: 'all', period: 'all', from: '', to: '' };
}

/**
 * Junta cada nota à aula e ao curso atuais.
 *
 * O vínculo é feito pelo `media_id`, não pelo texto: uma nota continua no curso
 * certo mesmo depois de a aula ser renomeada ou reindexada. Quando a mídia saiu do
 * catálogo, a nota não some — cai no grupo de aulas fora do catálogo.
 */
export function buildNotesEntries(notes: StudyNote[], catalog: Catalog | null): NotesEntry[] {
	return notes.map((note) => {
		const location = findLessonLocation(catalog, note.mediaId);
		return {
			note,
			courseId: location?.courseId ?? `media:${note.mediaId}`,
			courseTitle: location?.courseTitle ?? 'Aulas fora do catálogo',
			moduleTitle: location?.moduleTitle ?? null,
			lessonTitle: location?.lesson.title ?? 'Aula não encontrada no catálogo',
			lessonPath: location?.lesson.path ?? null
		};
	});
}

/** Busca sem acento e sem caixa, como o restante da interface. */
function normalize(value: string): string {
	return value.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLocaleLowerCase('pt-BR');
}

function dayStart(seconds: number): number {
	const date = new Date(seconds * 1000);
	return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime() / 1000;
}

function parseDateInput(value: string, endOfDay: boolean): number | null {
	const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value.trim());
	if (!match) return null;
	const date = new Date(Number(match[1]), Number(match[2]) - 1, Number(match[3]), endOfDay ? 23 : 0, endOfDay ? 59 : 0, endOfDay ? 59 : 0, endOfDay ? 999 : 0);
	return date.getTime() / 1000;
}

/**
 * Aplica busca, curso, período e ordenação. O período usa a data de edição, que é
 * a mesma data mostrada na linha; "Hoje" começa à meia-noite local, não 24h atrás.
 */
export function filterNotes(entries: NotesEntry[], filters: NotesFilters, sort: NotesSort, nowSeconds: number): NotesEntry[] {
	const query = normalize(filters.query.trim());
	let from = Number.NEGATIVE_INFINITY;
	let to = Number.POSITIVE_INFINITY;
	if (filters.period === 'today') {
		from = dayStart(nowSeconds);
	} else if (filters.period === '7') {
		from = dayStart(nowSeconds) - 6 * 86_400;
	} else if (filters.period === '30') {
		from = dayStart(nowSeconds) - 29 * 86_400;
	} else if (filters.period === 'custom') {
		from = parseDateInput(filters.from, false) ?? Number.NEGATIVE_INFINITY;
		to = parseDateInput(filters.to, true) ?? Number.POSITIVE_INFINITY;
	}
	const filtered = entries.filter((entry) => {
		if (filters.course !== 'all' && entry.courseId !== filters.course) return false;
		if (entry.note.updatedAt < from || entry.note.updatedAt > to) return false;
		if (!query) return true;
		const haystack = normalize([entry.note.text, entry.lessonTitle, entry.courseTitle, entry.moduleTitle ?? ''].join(' '));
		return haystack.includes(query);
	});
	return filtered.sort((a, b) => sort === 'oldest' ? a.note.updatedAt - b.note.updatedAt : b.note.updatedAt - a.note.updatedAt);
}

function dateKey(seconds: number): string {
	const date = new Date(seconds * 1000);
	return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

function dateLabel(key: string, nowSeconds: number): string {
	const today = dateKey(nowSeconds);
	const yesterday = dateKey(dayStart(nowSeconds) - 86_400);
	if (key === today) return 'Hoje';
	if (key === yesterday) return 'Ontem';
	const [year, month, day] = key.split('-').map(Number);
	return new Date(year, month - 1, day).toLocaleDateString('pt-BR', { day: '2-digit', month: 'long', year: 'numeric' });
}

/** Agrupa preservando a ordem das notas; os grupos de curso saem em ordem alfabética. */
export function groupNotes(entries: NotesEntry[], view: NotesView, sort: NotesSort, nowSeconds: number): NotesGroup[] {
	const groups = new Map<string, NotesGroup>();
	for (const entry of entries) {
		const key = view === 'course' ? entry.courseId : dateKey(entry.note.updatedAt);
		const title = view === 'course' ? entry.courseTitle : dateLabel(key, nowSeconds);
		const group = groups.get(key) ?? { key, title, entries: [] };
		group.entries.push(entry);
		groups.set(key, group);
	}
	const result = [...groups.values()];
	if (view === 'course') result.sort((a, b) => a.title.localeCompare(b.title, 'pt-BR'));
	else result.sort((a, b) => sort === 'oldest' ? a.key.localeCompare(b.key) : b.key.localeCompare(a.key));
	return result;
}

/** Opções do seletor de curso, com a contagem de notas de cada curso. */
export function courseOptions(entries: NotesEntry[]): { id: string; title: string; count: number }[] {
	const options = new Map<string, { id: string; title: string; count: number }>();
	for (const entry of entries) {
		const option = options.get(entry.courseId) ?? { id: entry.courseId, title: entry.courseTitle, count: 0 };
		option.count += 1;
		options.set(entry.courseId, option);
	}
	return [...options.values()].sort((a, b) => a.title.localeCompare(b.title, 'pt-BR'));
}
