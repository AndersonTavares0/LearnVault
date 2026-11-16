import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Check, Download, NotebookPen, Search } from 'lucide-react';
import { useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router';
import { GONE } from '@/components/course-picker';
import { ConfirmDelete } from '@/components/confirm-delete';
import { Markdown } from '@/components/markdown';
import { NoteEditor } from '@/components/note-editor';
import { Panel } from '@/components/panel';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useLocale } from '@/hooks/use-locale';
import { getLocale, tr } from '@/lib/i18n';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findLessonLocation, href, type LessonLocation } from '@/lib/router';
import { dayKey, notesThisWeek, notesToMarkdown } from '@/lib/notes-export';
import { normalizeForSearch } from '@/lib/search';
import { transport, type StudyNote } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useCatalog, useNotes } from '@/app/data';
import { PageHeader } from '@/app/shell';

type Entry = { note: StudyNote; location: LessonLocation | null };

const goneTitle = () => tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca');
const noteCourse = (location: LessonLocation | null) => location?.courseId ?? GONE;

/** Notes start from the notes: every note, newest lesson first, filtered by course on demand. */
export function NotesPage() {
	useLocale();
	const [params] = useSearchParams();
	const notes = useNotes();
	const catalog = useCatalog();
	if (!studySyncSupported) return <PageHeader icon={NotebookPen} title={tr('Notes', 'Notas')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	// Wait for both lists: without the catalog every note would look like it belongs to a lesson that is gone.
	if (notes.isPending || catalog.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>;
	if (notes.isError) return <p role="alert" className="text-destructive">{notes.error.message}</p>;
	return <Notebook courseId={params.get('course')} />;
}

/**
 * The notes as a notebook, of every course or of one: grouped by lesson (or by date) and ordered
 * by the moment in the lesson, searchable by text or lesson.
 */
function Notebook({ courseId }: { courseId: string | null }) {
	const [, setParams] = useSearchParams();
	const notes = useNotes();
	const catalog = useCatalog();
	const client = useQueryClient();
	const [query, setQuery] = useState('');
	const [view, setView] = useState<'lesson' | 'date'>('lesson');
	const [editing, setEditing] = useState<{ id: string; content: string } | null>(null);
	const [deleting, setDeleting] = useState<StudyNote | null>(null);
	const refresh = () => client.invalidateQueries({ queryKey: ['notes'] });
	const update = useMutation({ mutationFn: ({ id, content }: { id: string; content: string }) => transport.updateNote(id, content), onSuccess: () => { setEditing(null); return refresh(); } });
	const remove = useMutation({ mutationFn: (id: string) => transport.deleteNote(id), onSuccess: () => { setDeleting(null); return refresh(); } });

	const { groups, matches, courseTitle, courses, inScope, totalNotes } = useMemo(() => {
		const needle = normalizeForSearch(query.trim());
		const entries: Entry[] = (notes.data ?? []).map((note) => ({ note, location: findLessonLocation(catalog.data ?? null, note.mediaId) }));
		const named = new Map<string, { title: string; count: number }>();
		for (const { location } of entries) {
			const id = noteCourse(location);
			named.set(id, { title: location?.courseTitle ?? goneTitle(), count: (named.get(id)?.count ?? 0) + 1 });
		}
		const courses = [...named].map(([id, value]) => ({ id, ...value })).sort((a, b) => a.title.localeCompare(b.title, getLocale()));
		const courseTitle = courseId === null ? tr('All courses', 'Todos os cursos') : named.get(courseId)?.title ?? (courseId === GONE ? goneTitle() : (catalog.data?.nodes ?? []).find((node) => node.courseId === courseId)?.title ?? courseId);
		const inScope = entries.filter(({ location }) => courseId === null || noteCourse(location) === courseId).length;
		const matches = entries.filter(({ note, location }) => (courseId === null || noteCourse(location) === courseId) && (!needle || [note.content, location?.courseTitle, location?.lesson.title, note.relativePath].some((text) => text && normalizeForSearch(text).includes(needle))));
		const byLesson = new Map<string, { lesson: string; location: LessonLocation | null; updatedAt: number; items: Entry[] }>();
		for (const entry of matches) {
			const group = byLesson.get(entry.note.mediaId) ?? { lesson: entry.location?.lesson.title ?? entry.note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida'), location: entry.location, updatedAt: 0, items: [] };
			group.items.push(entry);
			group.updatedAt = Math.max(group.updatedAt, entry.note.updatedAt);
			byLesson.set(entry.note.mediaId, group);
		}
		const groups = [...byLesson.values()].sort((a, b) => b.updatedAt - a.updatedAt);
		for (const group of groups) group.items.sort((a, b) => a.note.positionSeconds - b.note.positionSeconds);
		return { groups, matches, courseTitle, courses, inScope, totalNotes: entries.length };
	}, [notes.data, catalog.data, query, courseId]);

	const courseSections = useMemo(() => {
		const sections = new Map<string, { id: string; title: string; count: number; groups: typeof groups }>();
		for (const group of groups) {
			const id = noteCourse(group.location);
			const section = sections.get(id) ?? { id, title: group.location?.courseTitle ?? goneTitle(), count: 0, groups: [] };
			section.count += group.items.length;
			section.groups.push(group);
			sections.set(id, section);
		}
		return [...sections.values()];
	}, [groups]);
	const total = matches.length;
	const isFiltered = courseId !== null || query.trim().length > 0;
	const dayGroups = useMemo(() => {
		const days = [...new Set(matches.map((entry) => dayKey(entry.note.updatedAt)))].sort().reverse();
		return days.map((day) => {
			const items = matches.filter((entry) => dayKey(entry.note.updatedAt) === day).sort((a, b) => b.note.updatedAt - a.note.updatedAt);
			const byCourse = new Map<string, { id: string; title: string; lessons: Map<string, { title: string; location: LessonLocation | null; items: Entry[] }> }>();
			for (const entry of items) {
				const cId = noteCourse(entry.location);
				const cTitle = entry.location?.courseTitle ?? goneTitle();
				const course = byCourse.get(cId) ?? { id: cId, title: cTitle, lessons: new Map() };
				const lKey = entry.note.mediaId;
				const lTitle = entry.location?.lesson.title ?? entry.note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida');
				const lesson = course.lessons.get(lKey) ?? { title: lTitle, location: entry.location, items: [] };
				lesson.items.push(entry);
				course.lessons.set(lKey, lesson);
				byCourse.set(cId, course);
			}
			return {
				day,
				courses: [...byCourse.values()].map((course) => ({ ...course, lessons: [...course.lessons.values()] }))
			};
		});
	}, [matches]);
	const weekCount = notesThisWeek(matches.map(({ note }) => note));
	const exportMarkdown = () => {
		const rows = matches.map(({ note, location }) => ({ course: location?.courseTitle ?? goneTitle(), lesson: location?.lesson.title ?? note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida'), positionSeconds: note.positionSeconds, updatedAt: note.updatedAt, content: note.content }));
		const url = URL.createObjectURL(new Blob([notesToMarkdown(rows)], { type: 'text/markdown;charset=utf-8' }));
		const link = document.createElement('a');
		link.href = url;
		link.download = `learnvault-notes-${courseTitle.replace(/[^\p{L}\p{N}]+/gu, '-').replace(/^-|-$/g, '').toLowerCase() || 'course'}.md`;
		link.click();
		URL.revokeObjectURL(url);
	};

	const renderItem = ({ note, location }: Entry) => (
		<li key={note.noteId} className="group -mx-3 flex flex-wrap items-start gap-4 rounded-xl px-3 py-3 transition-colors duration-150 focus-within:bg-panel/60 hover:bg-panel/60 sm:flex-nowrap">
			<div className="w-14 shrink-0 pt-0.5">
					{location ? (
						<Link to={href('course', location.courseId, location.lesson.path, note.positionSeconds).replace(/^#/, '')} aria-label={tr(`Open ${location.lesson.title} at ${formatDuration(note.positionSeconds)}`, `Abrir ${location.lesson.title} em ${formatDuration(note.positionSeconds)}`)} className="relative inline-flex h-8 min-w-14 items-center justify-center rounded-lg bg-brand-soft px-2 text-sm font-semibold text-primary tabular-nums after:absolute after:-inset-y-2 after:inset-x-0 after:content-[''] hover:underline">
							{formatDuration(note.positionSeconds)}
						</Link>
					) : (
						<span className="inline-flex h-8 min-w-14 items-center justify-center rounded-lg bg-muted px-2 text-sm text-muted-foreground tabular-nums">{formatDuration(note.positionSeconds)}</span>
					)}
			</div>
			<div className="min-w-0 flex-1 space-y-1">
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
					<Markdown text={note.content} className="max-w-prose font-[Literata_Variable,Georgia,serif] text-[1.0625rem] leading-relaxed" />
				)}
			</div>
			<div className="ml-auto flex shrink-0 items-center gap-1 self-start text-xs text-muted-foreground">
				<span className="whitespace-nowrap">{new Date(note.updatedAt * 1000).toLocaleDateString(getLocale(), { dateStyle: 'medium' })}</span>
				<span aria-hidden="true" className="opacity-50">·</span>
				<Button variant="ghost" size="xs" className="h-9 px-1.5 text-xs font-medium text-muted-foreground opacity-75 transition-opacity duration-150 group-focus-within:opacity-100 group-hover:opacity-100 hover:text-foreground [@media(hover:none)]:opacity-100" onClick={() => setEditing({ id: note.noteId, content: note.content })} aria-label={tr('Edit note', 'Editar nota')}>{tr('Edit', 'Editar')}</Button>
				<span aria-hidden="true" className="opacity-50">·</span>
				<Button variant="ghost" size="xs" className="h-9 px-1.5 text-xs font-medium text-muted-foreground opacity-75 transition-opacity duration-150 group-focus-within:opacity-100 group-hover:opacity-100 hover:text-destructive [@media(hover:none)]:opacity-100" disabled={remove.isPending} onClick={() => setDeleting(note)} aria-label={tr('Delete note', 'Excluir nota')}>{tr('Delete', 'Excluir')}</Button>
			</div>
		</li>
	);

	return (
		<div className="space-y-8">
			{courseId !== null && <Link to="/notes" className="inline-flex min-h-11 items-center gap-1.5 text-sm font-medium text-muted-foreground hover:text-foreground"><ArrowLeft className="size-4" aria-hidden="true" />{tr('All notes', 'Todas as notas')}</Link>}
			<PageHeader
				icon={NotebookPen}
				title={courseId === null ? tr('Notes', 'Notas') : tr(`${courseTitle} notes`, `Notas de ${courseTitle}`)}
				description={(courseId === null ? tr(`${total} ${total === 1 ? 'note' : 'notes'} across ${courses.length} ${courses.length === 1 ? 'course' : 'courses'}`, `${total} ${total === 1 ? 'nota' : 'notas'} em ${courses.length} ${courses.length === 1 ? 'curso' : 'cursos'}`) : tr(`${total} ${total === 1 ? 'note' : 'notes'} in this course`, `${total} ${total === 1 ? 'nota' : 'notas'} neste curso`)) + ` · ${tr(`${groups.length} ${groups.length === 1 ? 'lesson' : 'lessons'}`, `${groups.length} ${groups.length === 1 ? 'aula' : 'aulas'}`)} · ${tr(`${weekCount} this week`, `${weekCount} nesta semana`)}`}
				actions={matches.length > 0 ? (
					<Button variant="outline" className="rounded-full" onClick={exportMarkdown}>
						<Download />
						{isFiltered
							? tr(`Export ${total} filtered ${total === 1 ? 'note' : 'notes'} as Markdown`, `Exportar ${total} ${total === 1 ? 'nota filtrada' : 'notas filtradas'} em Markdown`)
							: tr(`Export all ${total} ${total === 1 ? 'note' : 'notes'} as Markdown`, `Exportar todas as notas (${total}) em Markdown`)}
					</Button>
				) : undefined}
			/>
			<div className={cn('grid items-start gap-8', courses.length > 1 && 'lg:grid-cols-[15rem_minmax(0,1fr)]')}>
				{courses.length > 1 && (
					<aside aria-label={tr('Courses', 'Cursos')} className="hidden space-y-1 lg:block">
						<button type="button" onClick={() => setParams({})} className={cn('flex min-h-11 w-full items-center justify-between gap-2 rounded-xl px-3.5 py-2 text-left text-sm transition-colors duration-150', courseId === null ? 'bg-panel font-semibold text-foreground' : 'text-muted-foreground hover:bg-brand-tint hover:text-foreground')}>
							<span className="truncate">{tr('All courses', 'Todos os cursos')}</span>
							<span className="shrink-0 text-xs tabular-nums">{totalNotes}</span>
						</button>
						{courses.map((course) => (
							<button key={course.id} type="button" onClick={() => setParams({ course: course.id })} className={cn('flex min-h-11 w-full items-center justify-between gap-2 rounded-xl px-3.5 py-2 text-left text-sm transition-colors duration-150', courseId === course.id ? 'bg-panel font-semibold text-foreground' : 'text-muted-foreground hover:bg-brand-tint hover:text-foreground')}>
								<span className="line-clamp-2 min-w-0">{course.title}</span>
								<span className="shrink-0 text-xs tabular-nums">{course.count}</span>
							</button>
						))}
					</aside>
				)}
				<div className="min-w-0 space-y-6">
					<div className="flex flex-wrap items-center gap-3">
						<div role="search" aria-label={tr('Notes', 'Notas')} className="relative min-w-0 flex-1 basis-60">
							<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
							<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={tr('Search notes or lessons', 'Buscar notas ou aulas')} aria-label={tr('Search notes', 'Buscar notas')} className="pl-9" />
						</div>
						{courses.length > 1 && (
							<select aria-label={tr('Filter by course', 'Filtrar por curso')} value={courseId ?? ''} onChange={(event) => setParams(event.target.value ? { course: event.target.value } : {})} className="h-11 min-w-0 max-w-full rounded-lg border border-input bg-background px-3 text-sm sm:max-w-xs lg:hidden">
								<option value="">{tr('All courses', 'Todos os cursos')}</option>
								{courses.map((course) => <option key={course.id} value={course.id}>{course.title} ({course.count})</option>)}
							</select>
						)}
						<div role="group" aria-label={tr('Group notes', 'Agrupar notas')} className="flex gap-2">
							<Button size="sm" variant={view === 'lesson' ? 'selected' : 'outline'} aria-pressed={view === 'lesson'} onClick={() => setView('lesson')}>{tr('By lesson', 'Por aula')}{view === 'lesson' && <Check className="size-3.5 text-primary" aria-hidden="true" />}</Button>
							<Button size="sm" variant={view === 'date' ? 'selected' : 'outline'} aria-pressed={view === 'date'} onClick={() => setView('date')}>{tr('By date', 'Por data')}{view === 'date' && <Check className="size-3.5 text-primary" aria-hidden="true" />}</Button>
						</div>
					</div>
					{(update.error || remove.error) && <p role="alert" className="text-sm text-destructive">{(update.error ?? remove.error)?.message}</p>}
					{notes.isPending ? (
						<p role="status" className="text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>
					) : inScope === 0 ? (
						<Panel className="p-5 text-sm text-muted-foreground">{tr('No notes yet. While watching a lesson, open the Notes tab or press n to write one tied to that moment.', 'Ainda não há notas. Assistindo a uma aula, abra a aba Notas ou aperte n para escrever uma presa àquele momento.')}</Panel>
					) : groups.length === 0 ? (
						<p role="status" className="text-muted-foreground">{tr('No notes match your search.', 'Nenhuma nota corresponde à busca.')}</p>
					) : view === 'date' ? (
						dayGroups.map(({ day, courses: dayCourses }) => (
							<section key={day} aria-label={day}>
								<Panel className="space-y-4">
									<h2 className="font-heading text-xl font-semibold">{new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { dateStyle: 'full' })}</h2>
									<div className="space-y-4">
										{dayCourses.map((course) => (
											<div key={course.id} className="space-y-3 border-t pt-3 first:border-t-0 first:pt-0">
												<h3 className="text-sm font-semibold text-muted-foreground">{course.title}</h3>
												{course.lessons.map((lesson) => (
													<div key={lesson.items[0].note.mediaId} className="space-y-1">
														<div className="flex flex-wrap items-baseline justify-between gap-2">
															<h4 className="text-base font-semibold text-balance">{lesson.title}</h4>
															{lesson.location && <Link to={href('course', lesson.location.courseId, lesson.location.lesson.path).replace(/^#/, '')} className="inline-flex min-h-11 items-center text-sm font-semibold text-primary hover:underline">{tr('Open lesson', 'Abrir aula')}</Link>}
														</div>
														<ul className="divide-y">{lesson.items.map((entry) => renderItem(entry))}</ul>
													</div>
												))}
											</div>
										))}
									</div>
								</Panel>
							</section>
						))
					) : (
						courseSections.map((section) => {
							const lessons = section.groups.map((group) => (
								<section key={group.items[0].note.mediaId} aria-label={group.lesson} className="border-t pt-5 pb-3">
									<div className="flex flex-wrap items-baseline justify-between gap-2">
										<h3 className="text-base font-semibold text-balance">{group.lesson}<span className="ml-2.5 text-xs font-normal text-muted-foreground tabular-nums">{tr(`${group.items.length} ${group.items.length === 1 ? 'note' : 'notes'}`, `${group.items.length} ${group.items.length === 1 ? 'nota' : 'notas'}`)}</span></h3>
										{group.location && <Link to={href('course', group.location.courseId, group.location.lesson.path).replace(/^#/, '')} className="inline-flex min-h-11 items-center text-sm font-semibold text-primary hover:underline">{tr('Open lesson', 'Abrir aula')}</Link>}
									</div>
									<ul className="mt-1 divide-y divide-border/50">{group.items.map((entry) => renderItem(entry))}</ul>
								</section>
							));
							return courseId === null ? (
								<section key={section.id} aria-label={section.title} className="space-y-1">
									<h2 className="pt-2 font-sans text-xl font-bold text-balance">{section.title}<span className="ml-3 text-sm font-normal text-muted-foreground tabular-nums">{tr(`${section.count} ${section.count === 1 ? 'note' : 'notes'}`, `${section.count} ${section.count === 1 ? 'nota' : 'notas'}`)}</span></h2>
									{lessons}
								</section>
							) : <div key={section.id}>{lessons}</div>;
						})
					)}
				</div>
			</div>
			<ConfirmDelete
				open={deleting !== null}
				onOpenChange={(open) => !open && setDeleting(null)}
				title={tr('Delete this note?', 'Excluir esta nota?')}
				description={tr('The note is removed for good. The lesson and your progress stay as they are.', 'A nota será removida para sempre. A aula e o progresso continuam como estão.')}
				label={tr('Delete note', 'Excluir nota')}
				pending={remove.isPending}
				onConfirm={() => { if (deleting) remove.mutate(deleting.noteId); }}
			/>
		</div>
	);
}
