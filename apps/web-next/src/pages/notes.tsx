import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Download, NotebookPen, Pencil, Search, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { Link } from 'react-router';
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

/**
 * Every note the user wrote, as a notebook: grouped by lesson and ordered by the moment in the
 * lesson, searchable by text, course or lesson, with a course filter. One older note is brought
 * back at the top, so writing notes pays off later.
 */
export function NotesPage() {
	useLocale();
	const notes = useNotes();
	const catalog = useCatalog();
	const client = useQueryClient();
	const [query, setQuery] = useState('');
	const [course, setCourse] = useState<string | null>(null);
	// Fixed when the page opens, so the note brought back does not change on every render.
	const [openedAt] = useState(() => Math.floor(Date.now() / 1000));
	const [view, setView] = useState<'lesson' | 'date'>('lesson');
	const [editing, setEditing] = useState<{ id: string; content: string } | null>(null);
	const refresh = () => client.invalidateQueries({ queryKey: ['notes'] });
	const update = useMutation({ mutationFn: ({ id, content }: { id: string; content: string }) => transport.updateNote(id, content), onSuccess: () => { setEditing(null); return refresh(); } });
	const remove = useMutation({ mutationFn: (id: string) => transport.deleteNote(id), onSuccess: refresh });

	const { groups, matches, courses, revisit } = useMemo(() => {
		const needle = normalizeForSearch(query.trim());
		const gone = tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca');
		const entries: Entry[] = (notes.data ?? []).map((note) => ({ note, location: findLessonLocation(catalog.data ?? null, note.mediaId) }));
		const courses = [...new Set(entries.map((entry) => entry.location?.courseTitle ?? gone))];
		const matches = entries.filter(({ note, location }) => (!course || (location?.courseTitle ?? gone) === course) && (!needle || [note.content, location?.courseTitle, location?.lesson.title, note.relativePath].some((text) => text && normalizeForSearch(text).includes(needle))));
		const byLesson = new Map<string, { course: string; lesson: string; location: LessonLocation | null; updatedAt: number; items: Entry[] }>();
		for (const entry of matches) {
			const group = byLesson.get(entry.note.mediaId) ?? { course: entry.location?.courseTitle ?? gone, lesson: entry.location?.lesson.title ?? entry.note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida'), location: entry.location, updatedAt: 0, items: [] };
			group.items.push(entry);
			group.updatedAt = Math.max(group.updatedAt, entry.note.updatedAt);
			byLesson.set(entry.note.mediaId, group);
		}
		const groups = [...byLesson.values()].sort((a, b) => b.updatedAt - a.updatedAt);
		for (const group of groups) group.items.sort((a, b) => a.note.positionSeconds - b.note.positionSeconds);
		// The note left alone the longest, among those that can still be opened.
		const revisit = entries.filter((entry) => entry.location).sort((a, b) => a.note.updatedAt - b.note.updatedAt)[0] ?? null;
		// A note from a couple of days ago or older: a fresh one is already on screen right below.
		const old = revisit && revisit.note.updatedAt < openedAt - 2 * 86_400 ? revisit : null;
		return { groups, matches, courses, revisit: entries.length > 3 ? old : null };
	}, [notes.data, catalog.data, query, course, openedAt]);

	if (!studySyncSupported) return <PageHeader icon={NotebookPen} title={tr('Notes', 'Notas')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	const total = notes.data?.length ?? 0;
	const week = notesThisWeek(notes.data ?? []);
	const dayGroups = [...new Set(matches.map((entry) => dayKey(entry.note.updatedAt)))].sort().reverse().map((day) => ({ day, items: matches.filter((entry) => dayKey(entry.note.updatedAt) === day).sort((a, b) => b.note.updatedAt - a.note.updatedAt) }));
	const exportMarkdown = () => {
		const rows = matches.map(({ note, location }) => ({ course: location?.courseTitle ?? tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca'), lesson: location?.lesson.title ?? note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida'), positionSeconds: note.positionSeconds, updatedAt: note.updatedAt, content: note.content }));
		const url = URL.createObjectURL(new Blob([notesToMarkdown(rows)], { type: 'text/markdown;charset=utf-8' }));
		const link = document.createElement('a');
		link.href = url;
		link.download = `learnvault-notes${course ? `-${course.replace(/[^\p{L}\p{N}]+/gu, '-').toLowerCase()}` : ''}.md`;
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
						{/* eslint-disable-next-line jsx-a11y/no-autofocus -- editing starts from an explicit click */}
						<textarea id={`edit-${note.noteId}`} autoFocus rows={3} maxLength={10_000} value={editing.content} onChange={(event) => setEditing({ ...editing, content: event.target.value })} className="w-full rounded-lg border border-input bg-background p-3 text-sm" />
						<div className="flex justify-end gap-2">
							<Button type="button" variant="outline" onClick={() => setEditing(null)}>{tr('Cancel', 'Cancelar')}</Button>
							<Button type="submit" disabled={update.isPending || !editing.content.trim()}>{tr('Save', 'Salvar')}</Button>
						</div>
					</form>
				) : (
					<p className="max-w-prose font-serif text-lg leading-relaxed whitespace-pre-wrap">{note.content}</p>
				)}
			</li>
	);


	return (
		<div className="space-y-8">
			<PageHeader icon={NotebookPen} title={tr('Notes', 'Notas')} description={tr(`${total} notes in ${Math.max(courses.length, 1)} ${courses.length === 1 ? 'course' : 'courses'}. This week you wrote ${week}.`, `${total} notas em ${Math.max(courses.length, 1)} ${courses.length === 1 ? 'curso' : 'cursos'}. Nesta semana você escreveu ${week}.`)} />
			<div className="flex flex-wrap items-center gap-3">
			<div role="group" aria-label={tr('Group notes', 'Agrupar notas')} className="flex gap-2">
				<Button size="sm" variant={view === 'lesson' ? 'default' : 'outline'} aria-pressed={view === 'lesson'} onClick={() => setView('lesson')}>{tr('By lesson', 'Por aula')}</Button>
				<Button size="sm" variant={view === 'date' ? 'default' : 'outline'} aria-pressed={view === 'date'} onClick={() => setView('date')}>{tr('By date', 'Por data')}</Button>
			</div>
			{total > 0 && <Button size="sm" variant="outline" className="ml-auto" disabled={matches.length === 0} onClick={exportMarkdown}><Download />{tr('Export as Markdown', 'Exportar em Markdown')}</Button>}
			</div>
			<div role="search" className="relative max-w-md">
				<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
				<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={tr('Search notes, courses or lessons', 'Buscar notas, cursos ou aulas')} aria-label={tr('Search notes', 'Buscar notas')} className="pl-9" />
			</div>
			{courses.length > 1 && (
				<div role="group" aria-label={tr('Course', 'Curso')} className="flex flex-wrap gap-2">
					<Button size="sm" variant={course === null ? 'default' : 'outline'} aria-pressed={course === null} onClick={() => setCourse(null)}>{tr('All courses', 'Todos os cursos')}</Button>
					{courses.map((title) => <Button key={title} size="sm" variant={course === title ? 'default' : 'outline'} aria-pressed={course === title} onClick={() => setCourse(title)}>{title}</Button>)}
				</div>
			)}
			{revisit?.location && !query && !course && (
				<aside aria-label={tr('A note to revisit', 'Uma nota para revisitar')} className="rounded-[2rem] border border-brand-soft bg-panel p-6 sm:p-8">
					<p className="text-sm text-muted-foreground">{revisit.location.courseTitle} · {new Date(revisit.note.updatedAt * 1000).toLocaleDateString(getLocale(), { dateStyle: 'medium' })}</p>
					<p className="mt-2 line-clamp-4 max-w-prose font-serif text-xl leading-relaxed">{revisit.note.content}</p>
					<Link to={href('course', revisit.location.courseId, revisit.location.lesson.path, revisit.note.positionSeconds).replace(/^#/, '')} className="mt-3 inline-flex min-h-11 items-center font-semibold text-[var(--a-soft)] hover:underline focus-visible:ring-2 focus-visible:ring-white/60 focus-visible:outline-none">{tr('Go back to that moment', 'Voltar àquele momento')}</Link>
				</aside>
			)}
			{(update.error || remove.error) && <p role="alert" className="text-sm text-destructive">{(update.error ?? remove.error)?.message}</p>}
			{notes.isPending ? (
				<p role="status" className="text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>
			) : total === 0 ? (
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
					<section key={group.items[0].note.mediaId} aria-label={`${group.course}: ${group.lesson}`}>
						<div className="flex flex-wrap items-end justify-between gap-2">
							<div className="min-w-0">
								<p className="text-sm text-muted-foreground">{group.course}</p>
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
