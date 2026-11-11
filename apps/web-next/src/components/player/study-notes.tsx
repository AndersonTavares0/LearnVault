import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Play, Plus, Trash2 } from 'lucide-react';
import { Link } from 'react-router';
import { Eyebrow } from '@/components/panel';
import { useState, type KeyboardEvent } from 'react';
import { Markdown } from '@/components/markdown';
import { NoteEditor } from '@/components/note-editor';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { tr } from '@/lib/i18n';
import { formatDuration } from '@/lib/player';
import { transport, type StudyNote } from '@/lib/transport';

export const lessonNotesKey = (mediaId: string) => ['notes', mediaId] as const;

export function useLessonNotes(mediaId: string | null) {
	return useQuery({ queryKey: lessonNotesKey(mediaId ?? ''), enabled: Boolean(mediaId), queryFn: () => transport.listNotes(mediaId!) });
}

/** Timestamped notes for one lesson; a note's time button seeks the player. */
export function StudyNotes({ mediaId, currentTime, onSeek, addRequest, title, trail, allNotesTo }: { mediaId: string; currentTime: number; onSeek: (seconds: number) => void; addRequest: number; title?: string; trail?: string; allNotesTo?: string }) {
	const client = useQueryClient();
	const notes = useLessonNotes(mediaId);
	const [draft, setDraft] = useState<{ position: number; content: string } | null>(null);
	const [lastRequest, setLastRequest] = useState(addRequest);
	const [saved, setSaved] = useState(false);
	const refresh = () => Promise.all([client.invalidateQueries({ queryKey: lessonNotesKey(mediaId) }), client.invalidateQueries({ queryKey: ['notes'], exact: true })]);
	const create = useMutation({ mutationFn: (note: { position: number; content: string }) => transport.createNote(mediaId, note.position, note.content), onSuccess: () => { setDraft(null); setSaved(true); return refresh(); } });
	const remove = useMutation({ mutationFn: (noteId: string) => transport.deleteNote(noteId), onSuccess: () => { setSaved(false); return refresh(); } });

	// The `n` shortcut asks the panel to open a draft at the current time.
	if (addRequest !== lastRequest) {
		setLastRequest(addRequest);
		if (!draft) setDraft({ position: Math.floor(currentTime), content: '' });
	}

	const save = () => {
		if (draft?.content.trim() && !create.isPending) create.mutate({ position: draft.position, content: draft.content.trim() });
	};
	const onKeyDown = (event: KeyboardEvent) => {
		if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
			event.preventDefault();
			save();
		}
	};
	const items: StudyNote[] = [...(notes.data ?? [])].sort((a, b) => a.positionSeconds - b.positionSeconds);
	const error = notes.error?.message || create.error?.message || remove.error?.message;

	return (
		<div className="space-y-4">
			{title && (
				<div className="flex flex-wrap items-start justify-between gap-3 border-b pb-4">
					<div className="min-w-0">
						<Eyebrow>{tr('My notes', 'Minhas notas')}</Eyebrow>
						<p className="mt-1 truncate font-heading text-xl font-bold">{title}</p>
						{trail && <p className="truncate text-sm text-muted-foreground">{trail}</p>}
					</div>
					<div className="flex items-center gap-3">
						<p role="status" aria-live="polite" className="text-sm text-muted-foreground">{saved ? tr('Saved · just now', 'Salva · agora') : ''}</p>
						{allNotesTo && <Link to={allNotesTo} className="inline-flex min-h-11 items-center rounded-full border px-4 text-sm font-medium hover:bg-brand-tint">{tr('View all', 'Ver todas')}</Link>}
					</div>
				</div>
			)}
			{!draft && (
				<Button variant="outline" onClick={() => setDraft({ position: Math.floor(currentTime), content: '' })}>
					<Plus />{tr(`Add note at ${formatDuration(currentTime)}`, `Anotar em ${formatDuration(currentTime)}`)}
				</Button>
			)}
			{error && <p role="alert" className="text-sm text-destructive">{error}</p>}
			{draft && (
				<div className="space-y-2 rounded-[1.5rem] border bg-card p-4">
					<Label htmlFor="note-draft">{tr(`Note at ${formatDuration(draft.position)}`, `Nota em ${formatDuration(draft.position)}`)}</Label>
					<NoteEditor id="note-draft" focusOnOpen rows={3} value={draft.content} onChange={(content) => setDraft({ ...draft, content })} onKeyDown={onKeyDown} describedBy="note-draft-hint" />
					<p id="note-draft-hint" className="text-xs text-muted-foreground">{tr('Ctrl+Enter saves the note.', 'Ctrl+Enter salva a nota.')}</p>
					<div className="flex justify-end gap-2">
						<Button variant="outline" onClick={() => setDraft(null)} disabled={create.isPending}>{tr('Cancel', 'Cancelar')}</Button>
						<Button onClick={save} disabled={create.isPending || !draft.content.trim()}>{create.isPending ? tr('Saving…', 'Salvando…') : tr('Save note', 'Salvar nota')}</Button>
					</div>
				</div>
			)}
			{notes.isPending ? (
				<p role="status" className="text-sm text-muted-foreground">{tr('Loading notes…', 'Carregando notas…')}</p>
			) : items.length === 0 && !draft ? (
				<p className="text-sm text-muted-foreground">{tr('No notes in this lesson yet. Notes are tied to the moment in the lesson, so you can jump back later.', 'Ainda não há notas nesta aula. As notas ficam presas ao momento da aula, para você voltar depois.')}</p>
			) : (
				<ul className="space-y-2">
					{items.map((note) => (
						<li key={note.noteId} className="flex items-start gap-2 rounded-lg border bg-card p-2">
							<Button variant="ghost" size="sm" onClick={() => onSeek(note.positionSeconds)} aria-label={tr(`Play from ${formatDuration(note.positionSeconds)}`, `Reproduzir de ${formatDuration(note.positionSeconds)}`)} className="font-mono tabular-nums">
								<Play />{formatDuration(note.positionSeconds)}
							</Button>
							<Markdown text={note.content} className="min-w-0 flex-1 py-3 text-sm" />
							<Button variant="ghost" size="icon" onClick={() => remove.mutate(note.noteId)} disabled={remove.isPending} aria-label={tr(`Delete note at ${formatDuration(note.positionSeconds)}`, `Excluir nota em ${formatDuration(note.positionSeconds)}`)}>
								<Trash2 />
							</Button>
						</li>
					))}
				</ul>
			)}
		</div>
	);
}
