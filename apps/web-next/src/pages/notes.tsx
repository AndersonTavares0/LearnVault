import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Download, NotebookPen, Pencil, Search, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router';
import { CoursePicker, GONE } from '@/components/course-picker';
import { Markdown } from '@/components/markdown';
import { NoteEditor } from '@/components/note-editor';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useLocale } from '@/hooks/use-locale';
import { getLocale, tr } from '@/lib/i18n';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findLessonLocation, href, type LessonLocation } from '@/lib/router';
import { dayKey, notesThisWeek, notesToMarkdown } from '@/lib/notes-export';
import { normalizeForSearch } from '@/lib/search';
import { transport, type StudyNote } from '@/lib/transport';
import { useCatalog, useNotes } from '@/app/data';
import { PageHeader } from '@/app/shell';

type Entry = { note: StudyNote; location: LessonLocation | null };

const goneTitle = () => tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca');
const noteCourse = (location: LessonLocation | null) => location?.courseId ?? GONE;

/** Notes start from a course: pick one to open its notebook. */
export function NotesPage() {
	useLocale();
	const [params] = useSearchParams();
	const courseId = params.get('course');
	const notes = useNotes();
	const catalog = useCatalog();
	const counts = useMemo(() => {
		const map = new Map<string, number>();
		for (const note of notes.data ?? []) {
			const id = noteCourse(findLessonLocation(catalog.data ?? null, note.mediaId));
			map.set(id, (map.get(id) ?? 0) + 1);
		}
		return map;
	}, [notes.data, catalog.data]);
	if (!studySyncSupported) return <PageHeader icon={NotebookPen} title={tr('Notes', 'Notas')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	// Wait for both lists: without the catalog every note would look like it belongs to a lesson that is gone.
	if (notes.isPending || catalog.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>;
	if (notes.isError) return <p role="alert" className="text-destructive">{notes.error.message}</p>;
	if (courseId) return <Notebook courseId={courseId} />;
	const total = notes.data?.length ?? 0;
	const count = (id: string) => counts.get(id) ?? 0;
	return (
		<CoursePicker
			icon={NotebookPen}
			title={tr('Choose a course to open its Notes', 'Escolha um curso para abrir suas Notas')}
			description={tr(`${total} ${total === 1 ? 'note' : 'notes'} in all. This week you wrote ${notesThisWeek(notes.data ?? [])}.`, `${total} ${total === 1 ? 'nota' : 'notas'} no total. Nesta semana você escreveu ${notesThisWeek(notes.data ?? [])}.`)}
			footer={(id) => tr(`${count(id)} ${count(id) === 1 ? 'note' : 'notes'}`, `${count(id)} ${count(id) === 1 ? 'nota' : 'notas'}`)}
			to={(id) => `/notes?course=${encodeURIComponent(id)}`}
			hasGone={counts.has(GONE)}
			goneTitle={goneTitle()}
		/>
	);
}

/**
 * The notes of one course, as a notebook: grouped by lesson (or by date) and ordered by the
 * moment in the lesson, searchable by text or lesson.
 */
function Notebook({ courseId }: { courseId: string }) {
	const notes = useNotes();
	const catalog = useCatalog();
	const client = useQueryClient();
	const [query, setQuery] = useState('');
	const [view, setView] = useState<'lesson' | 'date'>('lesson');
	const [editing, setEditing] = useState<{ id: string; content: string } | null>(null);
	const refresh = () => client.invalidateQueries({ queryKey: ['notes'] });
	const update = useMutation({ mutationFn: ({ id, content }: { id: string; content: string }) => transport.updateNote(id, content), onSuccess: () => { setEditing(null); return refresh(); } });
	const remove = useMutation({ mutationFn: (id: string) => transport.deleteNote(id), onSuccess: refresh });

	const { groups, matches, courseTitle } = useMemo(() => {
		const needle = normalizeForSearch(query.trim());
		const entries: Entry[] = (notes.data ?? []).map((note) => ({ note, location: findLessonLocation(catalog.data ?? null, note.mediaId) }));
		const courseTitle = entries.find((entry) => entry.location?.courseId === courseId)?.location?.courseTitle ?? (courseId === GONE ? goneTitle() : (catalog.data?.nodes ?? []).find((node) => node.courseId === courseId)?.title ?? courseId);
		const matches = entries.filter(({ note, location }) => noteCourse(location) === courseId && (!needle || [note.content, location?.courseTitle, location?.lesson.title, note.relativePath].some((text) => text && normalizeForSearch(text).includes(needle))));
		const byLesson = new Map<string, { lesson: string; location: LessonLocation | null; updatedAt: number; items: Entry[] }>();
		for (const entry of matches) {
			const group = byLesson.get(entry.note.mediaId) ?? { lesson: entry.location?.lesson.title ?? entry.note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida'), location: entry.location, updatedAt: 0, items: [] };
			group.items.push(entry);
			group.updatedAt = Math.max(group.updatedAt, entry.note.updatedAt);
			byLesson.set(entry.note.mediaId, group);
		}
		const groups = [...byLesson.values()].sort((a, b) => b.updatedAt - a.updatedAt);
		for (const group of groups) group.items.sort((a, b) => a.note.positionSeconds - b.note.positionSeconds);
		return { groups, matches, courseTitle };
	}, [notes.data, catalog.data, query, courseId]);

	const total = matches.length;
	const dayGroups = [...new Set(matches.map((entry) => dayKey(entry.note.updatedAt)))].sort().reverse().map((day) => ({ day, items: matches.filter((entry) => dayKey(entry.note.updatedAt) === day).sort((a, b) => b.note.updatedAt - a.note.updatedAt) }));
	const exportMarkdown = () => {
		const rows = matches.map(({ note, location }) => ({ course: location?.courseTitle ?? goneTitle(), lesson: location?.lesson.title ?? note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida'), positionSeconds: note.positionSeconds, updatedAt: note.updatedAt, content: note.content }));
		const url = URL.createObjectURL(new Blob([notesToMarkdown(rows)], { type: 'text/markdown;charset=utf-8' }));
		const link = document.createElement('a');
		link.href = url;
		link.download = `learnvault-notes-${courseTitle.replace(/[^\p{L}\p{N}]+/gu, '-').replace(/^-|-$/g, '').toLowerCase() || 'course'}.md`;
		link.click();
		URL.revokeObjectURL(url);
	};

	const renderItem = ({ note, location }: Entry, showLesson: boolean) => (
			<li key={note.noteId} className="space-y-1 border-t py-3">
			{showLesson && <p className="text-sm text-muted-foreground">{location ? `${location.courseTitle} · ${location.lesson.title}` : note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida')}</p>}
				<div className="flex flex-wrap items-center gap-3 text-sm">
					{location ? (
						<Link to={href('course', location.courseId, location.lesson.path, note.positionSeconds).replace(/^#/, '')} aria-label={tr(`Open ${location.lesson.title} at ${formatDuration(note.positionSeconds)}`, `Abrir ${location.lesson.title} em ${formatDuration(note.positionSeconds)}`)} className="inline-flex min-h-11 items-center font-semibold text-primary tabular-nums hover:underline">
							{formatDuration(note.positionSeconds)}
						</Link>
					) : (
						<span className="text-muted-foreground tabular-nums">{formatDuration(note.positionSeconds)}</span>
					)}
					<span className="text-muted-foreground">{new Date(note.updatedAt * 1000).toLocaleDateString(getLocale(), { dateStyle: 'medium' })}</span>
					<span className="ml-auto flex gap-1">
						<Button variant="ghost" size="icon" onClick={() => setEditing({ id: note.noteId, content: note.content })} aria-label={tr('Edit note', 'Editar nota')}><Pencil /></Button>
						<Button variant="ghost" size="icon" disabled={remove.isPending} onClick={() => remove.mutate(note.noteId)} aria-label={tr('Delete note', 'Excluir nota')}><Trash2 /></Button>
					</span>
				</div>
				{editing?.id === note.noteId ? (
					<form className="space-y-2" onSubmit={(event) => { event.preventDefault(); if (editing.content.trim()) update.mutate({ id: editing.id, content: editing.content.trim() }); }}>
						<label htmlFor={`edit-${note.noteId}`} className="sr-only">{tr('Note text', 'Texto da nota')}</label>
						<NoteEditor id={`edit-${note.noteId}`} focusOnOpen rows={3} value={editing.content} onChange={(content) => setEditing({ ...editing, content })} />
						<div className="flex justify-end gap-2">
							<Button type="button" variant="outline" onClick={() => setEditing(null)}>{tr('Cancel', 'Cancelar')}</Button>
							<Button type="submit" disabled={update.isPending || !editing.content.trim()}>{tr('Save', 'Salvar')}</Button>
						</div>
					</form>
				) : (
					<Markdown text={note.content} className="max-w-prose font-serif text-lg leading-relaxed" />
				)}
			</li>
	);


	return (
		<div className="space-y-8">
			<Link to="/notes" className="inline-flex min-h-11 items-center gap-1.5 text-sm font-medium text-muted-foreground hover:text-foreground"><ArrowLeft className="size-4" aria-hidden="true" />{tr('All courses', 'Todos os cursos')}</Link>
			<PageHeader icon={NotebookPen} title={tr(`${courseTitle} notes`, `Notas de ${courseTitle}`)} description={tr(`${total} ${total === 1 ? 'note' : 'notes'} in this course.`, `${total} ${total === 1 ? 'nota' : 'notas'} neste curso.`)} />
			<div className="flex flex-wrap items-center gap-3">
				<div role="group" aria-label={tr('Group notes', 'Agrupar notas')} className="flex gap-2">
					<Button size="sm" variant={view === 'lesson' ? 'default' : 'outline'} aria-pressed={view === 'lesson'} onClick={() => setView('lesson')}>{tr('By lesson', 'Por aula')}</Button>
					<Button size="sm" variant={view === 'date' ? 'default' : 'outline'} aria-pressed={view === 'date'} onClick={() => setView('date')}>{tr('By date', 'Por data')}</Button>
				</div>
				{matches.length > 0 && <Button size="sm" variant="outline" className="ml-auto" onClick={exportMarkdown}><Download />{tr('Export as Markdown', 'Exportar em Markdown')}</Button>}
			</div>
			<div role="search" aria-label={tr('Notes', 'Notas')} className="relative max-w-md">
				<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
				<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={tr('Search notes or lessons', 'Buscar notas ou aulas')} aria-label={tr('Search notes', 'Buscar notas')} className="pl-9" />
			</div>
			{(update.error || remove.error) && <p role="alert" className="text-sm text-destructive">{(update.error ?? remove.error)?.message}</p>}
			{notes.isPending ? (
				<p role="status" className="text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>
			) : notes.data?.every((note) => noteCourse(findLessonLocation(catalog.data ?? null, note.mediaId)) !== courseId) ? (
				<p className="text-muted-foreground">{tr('No notes yet. While watching a lesson, open the Notes tab or press n to write one tied to that moment.', 'Ainda não há notas. Assistindo a uma aula, abra a aba Notas ou aperte n para escrever uma presa àquele momento.')}</p>
			) : groups.length === 0 ? (
				<p role="status" className="text-muted-foreground">{tr('No notes match your search.', 'Nenhuma nota corresponde à busca.')}</p>
			) : view === 'date' ? (
				dayGroups.map(({ day, items }) => (
					<section key={day} aria-label={day}>
						<h2 className="font-serif text-2xl font-bold">{new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { dateStyle: 'full' })}</h2>
						<ul className="mt-2">{items.map((entry) => renderItem(entry, true))}</ul>
					</section>
				))
			) : (
				groups.map((group) => (
					<section key={group.items[0].note.mediaId} aria-label={group.lesson}>
						<div className="flex flex-wrap items-end justify-between gap-2">
							<div className="min-w-0">
								<h2 className="font-serif text-2xl font-bold text-balance">{group.lesson}</h2>
							</div>
							{group.location && <Link to={href('course', group.location.courseId, group.location.lesson.path).replace(/^#/, '')} className="inline-flex min-h-11 items-center text-sm font-semibold text-primary hover:underline">{tr('Open lesson', 'Abrir aula')}</Link>}
						</div>
						<ul className="mt-2">
							{group.items.map((entry) => renderItem(entry, false))}
						</ul>
					</section>
				))
			)}
		</div>
	);
}
