import { useMutation, useQueryClient } from '@tanstack/react-query';
import { Pencil, Search, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { Link } from 'react-router';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { useLocale } from '@/hooks/use-locale';
import { getLocale, tr } from '@/lib/i18n';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findLessonLocation, href, type LessonLocation } from '@/lib/router';
import { normalizeForSearch } from '@/lib/search';
import { transport, type StudyNote } from '@/lib/transport';
import { useCatalog, useNotes } from '@/app/data';
import { PageHeader } from '@/app/shell';

type Entry = { note: StudyNote; location: LessonLocation | null };

/** Every note the user wrote, searchable by text, course or lesson, grouped by course. */
export function NotesPage() {
	useLocale();
	const notes = useNotes();
	const catalog = useCatalog();
	const client = useQueryClient();
	const [query, setQuery] = useState('');
	const [editing, setEditing] = useState<{ id: string; content: string } | null>(null);
	const refresh = () => client.invalidateQueries({ queryKey: ['notes'] });
	const update = useMutation({ mutationFn: ({ id, content }: { id: string; content: string }) => transport.updateNote(id, content), onSuccess: () => { setEditing(null); return refresh(); } });
	const remove = useMutation({ mutationFn: (id: string) => transport.deleteNote(id), onSuccess: refresh });

	const groups = useMemo(() => {
		const needle = normalizeForSearch(query.trim());
		const entries: Entry[] = (notes.data ?? []).map((note) => ({ note, location: findLessonLocation(catalog.data ?? null, note.mediaId) }));
		const matches = entries.filter(({ note, location }) => !needle || [note.content, location?.courseTitle, location?.lesson.title, note.relativePath].some((text) => text && normalizeForSearch(text).includes(needle)));
		const byCourse = new Map<string, Entry[]>();
		for (const entry of matches) {
			const title = entry.location?.courseTitle ?? tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca');
			byCourse.set(title, [...(byCourse.get(title) ?? []), entry]);
		}
		return [...byCourse.entries()].map(([title, items]) => ({ title, items: items.sort((a, b) => b.note.updatedAt - a.note.updatedAt) }));
	}, [notes.data, catalog.data, query]);

	if (!studySyncSupported) return <PageHeader title={tr('Notes', 'Notas')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	const total = notes.data?.length ?? 0;

	return (
		<div className="space-y-6">
			<PageHeader title={tr('Notes', 'Notas')} description={tr(`${total} notes across your courses`, `${total} notas nos seus cursos`)} />
			<div role="search" className="relative max-w-md">
				<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
				<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={tr('Search notes, courses or lessons', 'Buscar notas, cursos ou aulas')} aria-label={tr('Search notes', 'Buscar notas')} className="pl-9" />
			</div>
			{(update.error || remove.error) && <p role="alert" className="text-sm text-destructive">{(update.error ?? remove.error)?.message}</p>}
			{notes.isPending ? (
				<p role="status" className="text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>
			) : total === 0 ? (
				<p className="text-muted-foreground">{tr('No notes yet. While watching a lesson, open the Notes tab or press n to write one tied to that moment.', 'Ainda não há notas. Assistindo a uma aula, abra a aba Notas ou aperte n para escrever uma presa àquele momento.')}</p>
			) : groups.length === 0 ? (
				<p role="status" className="text-muted-foreground">{tr('No notes match your search.', 'Nenhuma nota corresponde à busca.')}</p>
			) : (
				groups.map((group) => (
					<section key={group.title} aria-label={group.title} className="space-y-3">
						<h2 className="text-lg font-semibold">{group.title}</h2>
						<ul className="space-y-2">
							{group.items.map(({ note, location }) => (
								<li key={note.noteId} className="space-y-2 rounded-lg border bg-card p-4">
									<div className="flex flex-wrap items-center gap-2 text-sm">
										{location ? (
											<Link to={href('course', location.courseId, location.lesson.path, note.positionSeconds).replace(/^#/, '')} className="inline-flex min-h-11 items-center font-medium text-primary hover:underline">
												{location.lesson.title} · {formatDuration(note.positionSeconds)}
											</Link>
										) : (
											<span className="text-muted-foreground">{note.relativePath ?? tr('Unknown lesson', 'Aula desconhecida')}</span>
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
										<p className="whitespace-pre-wrap">{note.content}</p>
									)}
								</li>
							))}
						</ul>
					</section>
				))
			)}
		</div>
	);
}
