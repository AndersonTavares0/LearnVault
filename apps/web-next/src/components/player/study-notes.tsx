import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { PenLine, Pencil, SquarePlus, Trash2 } from 'lucide-react';
import { Link } from 'react-router';
import { useState, type KeyboardEvent } from 'react';
import { ConfirmDelete } from '@/components/confirm-delete';
import { Markdown } from '@/components/markdown';
import { NoteEditor } from '@/components/note-editor';
import { Button, buttonVariants } from '@/components/ui/button';
import { getLocale, tr } from '@/lib/i18n';
import { formatDuration } from '@/lib/player';
import { transport, type StudyNote } from '@/lib/transport';

export const lessonNotesKey = (mediaId: string) => ['notes', mediaId] as const;

export function useLessonNotes(mediaId: string | null) {
	return useQuery({ queryKey: lessonNotesKey(mediaId ?? ''), enabled: Boolean(mediaId), queryFn: () => transport.listNotes(mediaId!) });
}

/** The amber timestamp chip: a note's time, and the draft's time while it is being written. */
const chip = 'inline-flex min-h-11 items-center rounded-md border border-primary/45 px-2 text-[11.5px] font-semibold tabular-nums text-primary hover:bg-primary/10 focus-visible:outline-2 focus-visible:outline-ring lg:min-h-7 lg:px-1.5';

/** Timestamped notes for one lesson; a note's time button seeks the player. */
export function StudyNotes({ mediaId, currentTime, onSeek, addRequest, onMakeCard, allNotesTo }: { mediaId: string; currentTime: number; onSeek: (seconds: number) => void; addRequest: number; onMakeCard?: (note: StudyNote) => void; allNotesTo?: string }) {
	const client = useQueryClient();
	const notes = useLessonNotes(mediaId);
	const [draft, setDraft] = useState<{ position: number; content: string } | null>(null);
	const [editing, setEditing] = useState<{ id: string; content: string } | null>(null);
	const [deleting, setDeleting] = useState<StudyNote | null>(null);
	const [lastRequest, setLastRequest] = useState(addRequest);
	const [saved, setSaved] = useState(false);
	const refresh = () => Promise.all([client.invalidateQueries({ queryKey: lessonNotesKey(mediaId) }), client.invalidateQueries({ queryKey: ['notes'], exact: true })]);
	const create = useMutation({ mutationFn: (note: { position: number; content: string }) => transport.createNote(mediaId, note.position, note.content), onSuccess: () => { setDraft(null); setSaved(true); return refresh(); } });
	// The lesson's timestamp is not part of an edit: updateNote changes only the text.
	const update = useMutation({ mutationFn: ({ id, content }: { id: string; content: string }) => transport.updateNote(id, content), onSuccess: () => { setEditing(null); setSaved(true); return refresh(); } });
	const remove = useMutation({ mutationFn: (noteId: string) => transport.deleteNote(noteId), onSuccess: () => { setDeleting(null); setSaved(false); return refresh(); } });

	// The `n` shortcut asks the panel to open a draft at the current time.
	if (addRequest !== lastRequest) {
		setLastRequest(addRequest);
		if (!draft) setDraft({ position: Math.floor(currentTime), content: '' });
	}

	const save = () => {
		if (draft?.content.trim() && !create.isPending) create.mutate({ position: draft.position, content: draft.content.trim() });
	};
	const saveEdit = () => {
		if (editing?.content.trim() && !update.isPending) update.mutate({ id: editing.id, content: editing.content.trim() });
	};
	const onKeyDown = (event: KeyboardEvent) => {
		if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
			event.preventDefault();
			save();
		}
	};
	const onEditKeyDown = (event: KeyboardEvent) => {
		if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
			event.preventDefault();
			saveEdit();
		}
	};
	const items: StudyNote[] = [...(notes.data ?? [])].sort((a, b) => a.positionSeconds - b.positionSeconds);
	const error = notes.error?.message || create.error?.message || update.error?.message || remove.error?.message;

	return (
		<div className="space-y-4">
			<div className="flex min-h-11 items-center justify-between gap-3">
				<p role="status" aria-live="polite" className="text-sm text-muted-foreground">{saved ? tr('Note saved', 'Nota salva') : ''}</p>
				{allNotesTo && <Link to={allNotesTo} className="inline-flex min-h-11 items-center text-sm font-medium text-muted-foreground hover:text-foreground">{tr('View all', 'Ver todas')}</Link>}
			</div>
			{error && <p role="alert" className="text-sm text-destructive">{error}</p>}
			{draft ? (
				<div className="space-y-3 rounded-xl border border-[var(--study-rule)] bg-background p-3">
					<span className={chip}>{formatDuration(draft.position)}</span>
					<NoteEditor id="note-draft" label={tr(`Note at ${formatDuration(draft.position)}`, `Nota em ${formatDuration(draft.position)}`)} focusOnOpen rows={3} value={draft.content} onChange={(content) => setDraft({ ...draft, content })} onKeyDown={onKeyDown} describedBy="note-draft-hint" />
					<div className="flex flex-wrap items-center justify-between gap-2">
						<p id="note-draft-hint" className="text-xs text-muted-foreground">{tr('Ctrl+Enter saves the note.', 'Ctrl+Enter salva a nota.')}</p>
						<div className="flex gap-2">
							<Button variant="outline" onClick={() => setDraft(null)} disabled={create.isPending}>{tr('Cancel', 'Cancelar')}</Button>
							<Button onClick={save} disabled={create.isPending || !draft.content.trim()}>{create.isPending ? tr('Saving…', 'Salvando…') : tr('Save note', 'Salvar nota')}</Button>
						</div>
					</div>
				</div>
			) : (
				// Desktop: standard secondary outline trigger. Phone: primary filled action.
				<button type="button" onClick={() => setDraft({ position: Math.floor(currentTime), content: '' })} className={`${buttonVariants({ variant: 'outline' })} w-full justify-center max-lg:border-primary max-lg:bg-primary max-lg:font-semibold max-lg:text-primary-foreground max-lg:hover:bg-primary/90 dark:max-lg:border-primary dark:max-lg:bg-primary dark:max-lg:text-primary-foreground dark:max-lg:hover:bg-primary/90`}>
					<PenLine className="size-4 shrink-0" aria-hidden="true" />{tr(`Write a note at ${formatDuration(currentTime)}`, `Escrever uma nota em ${formatDuration(currentTime)}`)}
				</button>
			)}
			{notes.isPending ? (
				<p role="status" className="text-sm text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>
			) : items.length === 0 && !draft ? (
				<p className="text-sm text-muted-foreground">{tr('No notes in this lesson yet. Notes are tied to the moment in the lesson, so you can jump back later.', 'Ainda não há notas nesta aula. As notas ficam presas ao momento da aula, para você voltar depois.')}</p>
			) : (
				<ul className="divide-y divide-[var(--study-rule)]">
					{items.map((note) => (
						<li key={note.noteId} className="flex items-start gap-3 py-3">
							<button type="button" onClick={() => onSeek(note.positionSeconds)} aria-label={tr(`Play from ${formatDuration(note.positionSeconds)}`, `Reproduzir de ${formatDuration(note.positionSeconds)}`)} className={chip}>
								{formatDuration(note.positionSeconds)}
							</button>
							<div className="min-w-0 flex-1 space-y-1">
								{editing?.id === note.noteId ? (
									<form className="space-y-2" onSubmit={(event) => { event.preventDefault(); saveEdit(); }}>
										<NoteEditor id={`edit-${note.noteId}`} label={tr('Note text', 'Texto da nota')} focusOnOpen rows={3} value={editing.content} onChange={(content) => setEditing({ ...editing, content })} onKeyDown={onEditKeyDown} />
										<div className="flex justify-end gap-2">
											<Button type="button" variant="outline" onClick={() => setEditing(null)} disabled={update.isPending}>{tr('Cancel', 'Cancelar')}</Button>
											<Button type="submit" disabled={update.isPending || !editing.content.trim()}>{update.isPending ? tr('Saving…', 'Salvando…') : tr('Save', 'Salvar')}</Button>
										</div>
									</form>
								) : (
									<Markdown text={note.content} className="text-sm leading-relaxed" />
								)}
								<p className="text-xs text-muted-foreground">{new Date(note.createdAt * 1000).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric' })}</p>
							</div>
							{onMakeCard && (
								<Button variant="ghost" size="sm" onClick={() => onMakeCard(note)} title={tr('Create a review card from this note', 'Criar um cartão de revisão desta nota')} aria-label={tr(`Make card from note at ${formatDuration(note.positionSeconds)}`, `Criar cartão da nota em ${formatDuration(note.positionSeconds)}`)}>
									<SquarePlus />{tr('Card', 'Cartão')}
								</Button>
							)}
							<Button variant="ghost" size="icon" onClick={() => setEditing({ id: note.noteId, content: note.content })} aria-label={tr(`Edit note at ${formatDuration(note.positionSeconds)}`, `Editar nota em ${formatDuration(note.positionSeconds)}`)}>
								<Pencil />
							</Button>
							<Button variant="ghost" size="icon" onClick={() => setDeleting(note)} disabled={remove.isPending} aria-label={tr(`Delete note at ${formatDuration(note.positionSeconds)}`, `Excluir nota em ${formatDuration(note.positionSeconds)}`)}>
								<Trash2 />
							</Button>
						</li>
					))}
				</ul>
			)}
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
