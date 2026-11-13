import { Play, Plus, Trash2 } from 'lucide-react';
import { useState, type FormEvent } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useBookmarkMutations, useBookmarks } from '@/app/data';
import { tr } from '@/lib/i18n';
import { formatDuration } from '@/lib/player';
import { MAX_BOOKMARK_LABEL } from '@/lib/transport';

/** Labelled moments in one lesson. A marker seeks the player; the list is keyboard-first. */
export function Bookmarks({ mediaId, currentTime, onSeek }: { mediaId: string; currentTime: number; onSeek: (seconds: number) => void }) {
	const bookmarks = useBookmarks(mediaId);
	const { create, remove } = useBookmarkMutations(mediaId);
	const [draft, setDraft] = useState<{ position: number; label: string } | null>(null);
	const save = (event: FormEvent) => {
		event.preventDefault();
		if (draft?.label.trim() && !create.isPending) create.mutate({ positionSeconds: draft.position, label: draft.label.trim() }, { onSuccess: () => setDraft(null) });
	};
	const error = bookmarks.error?.message || create.error?.message || remove.error?.message;
	const items = bookmarks.data ?? [];

	return (
		<div className="space-y-4">
			{!draft && (
				<Button variant="outline" onClick={() => setDraft({ position: Math.floor(currentTime), label: '' })}>
					<Plus />{tr(`Add timestamp at ${formatDuration(currentTime)}`, `Marcar em ${formatDuration(currentTime)}`)}
				</Button>
			)}
			{draft && (
				<form onSubmit={save} className="space-y-3 rounded-[1.5rem] border bg-card p-4" aria-label={tr('New timestamp', 'Novo marcador')}>
					<div className="space-y-2">
						<Label htmlFor="bookmark-label">{tr(`Label at ${formatDuration(draft.position)}`, `Rótulo em ${formatDuration(draft.position)}`)}</Label>
						{/* eslint-disable-next-line jsx-a11y/no-autofocus -- the form opens on request */}
						<Input id="bookmark-label" autoFocus required maxLength={MAX_BOOKMARK_LABEL} value={draft.label} onChange={(event) => setDraft({ ...draft, label: event.target.value })} placeholder={tr('Key idea, example, question…', 'Ideia-chave, exemplo, pergunta…')} />
					</div>
					<div className="flex justify-end gap-2">
						<Button type="button" variant="outline" onClick={() => setDraft(null)} disabled={create.isPending}>{tr('Cancel', 'Cancelar')}</Button>
						<Button type="submit" disabled={create.isPending || !draft.label.trim()}>{create.isPending ? tr('Saving…', 'Salvando…') : tr('Save timestamp', 'Salvar marcador')}</Button>
					</div>
				</form>
			)}
			{error && <p role="alert" className="text-sm text-destructive">{error}</p>}
			{bookmarks.isPending ? (
				<p role="status" className="text-sm text-muted-foreground">{tr('Loading timestamps…', 'Carregando marcadores…')}</p>
			) : items.length === 0 && !draft ? (
				<p className="text-sm text-muted-foreground">{tr('No timestamps yet. Mark a moment to jump straight back to it.', 'Ainda não há marcadores. Marque um momento para voltar direto a ele.')}</p>
			) : (
				<ul className="space-y-2">
					{items.map((bookmark) => (
						<li key={bookmark.bookmarkId} className="flex items-center gap-2 rounded-lg border bg-card p-2">
							<Button variant="ghost" size="sm" onClick={() => onSeek(bookmark.positionSeconds)} aria-label={tr(`Play from ${formatDuration(bookmark.positionSeconds)}`, `Reproduzir de ${formatDuration(bookmark.positionSeconds)}`)} className="font-mono tabular-nums">
								<Play />{formatDuration(bookmark.positionSeconds)}
							</Button>
							<span className="min-w-0 flex-1 py-2 text-sm">{bookmark.label}</span>
							<Button variant="ghost" size="icon" onClick={() => remove.mutate(bookmark.bookmarkId)} disabled={remove.isPending} aria-label={tr(`Delete timestamp: ${bookmark.label}`, `Excluir marcador: ${bookmark.label}`)}>
								<Trash2 />
							</Button>
						</li>
					))}
				</ul>
			)}
		</div>
	);
}
