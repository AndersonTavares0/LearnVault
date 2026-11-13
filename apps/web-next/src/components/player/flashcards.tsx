import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Pencil, Plus, Trash2, Upload } from 'lucide-react';
import { useEffect, useRef, useState, type ChangeEvent, type FormEvent, type KeyboardEvent } from 'react';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { MAX_IMPORT_CARDS, parseCards } from '@/lib/cards';
import { tr } from '@/lib/i18n';
import { formatDuration } from '@/lib/player';
import { transport, type Flashcard, type ReviewRating } from '@/lib/transport';

const nowSeconds = () => Math.floor(Date.now() / 1000);

function dueLabel(nextDue: number, now: number): string {
	const diff = nextDue - now;
	if (diff <= 0) return tr('Due now', 'Para revisar agora');
	const hours = Math.round(diff / 3600);
	if (hours < 24) return tr(`In ${Math.max(1, hours)} h`, `Em ${Math.max(1, hours)} h`);
	return tr(`In ${Math.round(diff / 86400)} days`, `Em ${Math.round(diff / 86400)} dias`);
}

/** Spaced-repetition cards for one lesson: review what is due, or manage the deck. */
export function Flashcards({ mediaId, currentTime = 0, addRequest = 0 }: { mediaId: string; currentTime?: number; addRequest?: number }) {
	const client = useQueryClient();
	const key = ['flashcards', mediaId] as const;
	const cards = useQuery({ queryKey: key, queryFn: () => transport.listFlashcards(mediaId) });
	const [mode, setMode] = useState<'review' | 'manage'>('review');
	const [revealed, setRevealed] = useState(false);
	const [form, setForm] = useState<{ cardId: string | null; front: string; back: string; position: number | null } | null>(null);
	const [now, setNow] = useState(nowSeconds);
	// The lesson's "Make card" button asks this panel to open a blank card at the player time.
	const [lastRequest, setLastRequest] = useState(addRequest);
	if (addRequest !== lastRequest) {
		setLastRequest(addRequest);
		setMode('manage');
		setForm({ cardId: null, front: '', back: '', position: Math.floor(currentTime) });
	}
	useEffect(() => {
		const timer = setInterval(() => setNow(nowSeconds()), 10_000);
		return () => clearInterval(timer);
	}, []);

	const patch = (card: Flashcard) => client.setQueryData<Flashcard[]>(key, (list = []) => (list.some((item) => item.cardId === card.cardId) ? list.map((item) => (item.cardId === card.cardId ? card : item)) : [...list, card]));
	const review = useMutation({ mutationFn: ({ cardId, rating }: { cardId: string; rating: ReviewRating }) => transport.reviewFlashcard(cardId, rating), onSuccess: (card) => { patch(card); setRevealed(false); setNow(nowSeconds()); } });
	const save = useMutation({
		mutationFn: (input: { cardId: string | null; front: string; back: string; position: number | null }) => (input.cardId ? transport.updateFlashcard(input.cardId, input.front, input.back) : transport.createFlashcard(mediaId, input.front, input.back, input.position)),
		onSuccess: (card) => { patch(card); setForm(null); }
	});
	const remove = useMutation({ mutationFn: (cardId: string) => transport.deleteFlashcard(cardId), onSuccess: (_, cardId) => client.setQueryData<Flashcard[]>(key, (list = []) => list.filter((item) => item.cardId !== cardId)) });

	// Import: an Anki text export (tab-separated) or a simple CSV, added to this lesson's deck.
	const fileInput = useRef<HTMLInputElement>(null);
	const [importMessage, setImportMessage] = useState('');
	const importCards = useMutation({
		mutationFn: async (file: File) => {
			const parsed = parseCards(await file.text());
			const batch = parsed.slice(0, MAX_IMPORT_CARDS);
			for (const card of batch) patch(await transport.createFlashcard(mediaId, card.front, card.back, null));
			return { added: batch.length, skipped: parsed.length - batch.length };
		},
		onSuccess: ({ added, skipped }) => setImportMessage(added === 0
			? tr('No cards found in that file. Use one card per line: front, a tab, then back.', 'Nenhum cartão encontrado no arquivo. Use um cartão por linha: frente, uma tabulação e o verso.')
			: tr(`${added} cards imported.${skipped ? ` ${skipped} more were left out; import them in another file.` : ''}`, `${added} cartões importados.${skipped ? ` Outros ${skipped} ficaram de fora; importe-os em outro arquivo.` : ''}`)),
		onError: () => { setImportMessage(''); void client.invalidateQueries({ queryKey: key }); }
	});
	const onFile = (event: ChangeEvent<HTMLInputElement>) => {
		const file = event.target.files?.[0];
		event.target.value = '';
		if (file) importCards.mutate(file);
	};

	const all = cards.data ?? [];
	const due = all.filter((card) => card.nextDue <= now);
	const current = due[0] ?? null;
	const error = cards.error?.message || review.error?.message || save.error?.message || remove.error?.message || importCards.error?.message;

	const submit = (event?: FormEvent) => {
		event?.preventDefault();
		if (form && form.front.trim() && form.back.trim() && !save.isPending) save.mutate({ cardId: form.cardId, front: form.front.trim(), back: form.back.trim(), position: form.position });
	};
	// Shortcuts stay inside the panel so they never fight the player's.
	const onKeyDown = (event: KeyboardEvent) => {
		const typing = (event.target as HTMLElement).closest('textarea, input');
		if (form && (event.ctrlKey || event.metaKey) && event.key === 'Enter') { event.preventDefault(); submit(); return; }
		if (form && event.key === 'Escape') { event.preventDefault(); setForm(null); return; }
		if (typing || mode !== 'review' || !current || review.isPending) return;
		if (!revealed && event.key === 'Enter') { event.preventDefault(); setRevealed(true); }
		else if (revealed && event.key === '1') { event.preventDefault(); review.mutate({ cardId: current.cardId, rating: 'again' }); }
		else if (revealed && event.key === '2') { event.preventDefault(); review.mutate({ cardId: current.cardId, rating: 'good' }); }
	};

	return (
		// eslint-disable-next-line jsx-a11y/no-static-element-interactions -- keyboard shortcuts for the review controls inside
		<div className="space-y-4" onKeyDown={onKeyDown}>
			<div className="flex flex-wrap items-center gap-2">
				<div role="group" aria-label={tr('Flashcards view', 'Visualização dos cartões')} className="flex gap-1">
					<Button size="sm" variant={mode === 'review' ? 'secondary' : 'ghost'} aria-pressed={mode === 'review'} onClick={() => { setMode('review'); setForm(null); }}>{tr('Review', 'Revisar')}</Button>
					<Button size="sm" variant={mode === 'manage' ? 'secondary' : 'ghost'} aria-pressed={mode === 'manage'} onClick={() => setMode('manage')}>{tr('Manage', 'Gerenciar')}</Button>
				</div>
				<p className="text-sm text-muted-foreground">{tr(`${due.length} due · ${all.length} total`, `${due.length} para revisar · ${all.length} no total`)}</p>
				<span className="ml-auto flex gap-2">
					<input ref={fileInput} type="file" accept=".txt,.tsv,.csv,text/plain,text/csv,text/tab-separated-values" className="sr-only" tabIndex={-1} aria-label={tr('Card file to import', 'Arquivo de cartões para importar')} onChange={onFile} />
					<Button size="sm" variant="ghost" disabled={importCards.isPending} onClick={() => fileInput.current?.click()}><Upload />{importCards.isPending ? tr('Importing…', 'Importando…') : tr('Import', 'Importar')}</Button>
					{!form && <Button size="sm" variant="outline" onClick={() => setForm({ cardId: null, front: '', back: '', position: Math.floor(currentTime) })}><Plus />{tr('New card', 'Novo cartão')}</Button>}
				</span>
			</div>
			<p role="status" aria-live="polite" className="text-sm text-muted-foreground empty:hidden">{importMessage}</p>
			{error && <p role="alert" className="text-sm text-destructive">{error}</p>}

			{form && (
				<form onSubmit={submit} className="space-y-3 rounded-lg border bg-card p-4" aria-label={form.cardId ? tr('Edit card', 'Editar cartão') : tr('New card', 'Novo cartão')}>
					<div className="space-y-2">
						<Label htmlFor="card-front">{tr('Front: question or concept', 'Frente: pergunta ou conceito')}</Label>
						{/* eslint-disable-next-line jsx-a11y/no-autofocus -- the form opens on request */}
						<textarea id="card-front" autoFocus rows={2} maxLength={4000} required value={form.front} onChange={(event) => setForm({ ...form, front: event.target.value })} className="w-full rounded-lg border border-input bg-background p-3 text-sm" />
					</div>
					<div className="space-y-2">
						<Label htmlFor="card-back">{tr('Back: answer or explanation', 'Verso: resposta ou explicação')}</Label>
						<textarea id="card-back" rows={3} maxLength={4000} required value={form.back} onChange={(event) => setForm({ ...form, back: event.target.value })} className="w-full rounded-lg border border-input bg-background p-3 text-sm" />
					</div>
					{form.position !== null && <p className="text-xs text-muted-foreground">{tr(`Linked to this lesson at ${formatDuration(form.position)}.`, `Vinculado a esta aula em ${formatDuration(form.position)}.`)}</p>}
					<p className="text-xs text-muted-foreground">{tr('Ctrl+Enter saves, Esc cancels.', 'Ctrl+Enter salva, Esc cancela.')}</p>
					<div className="flex justify-end gap-2">
						<Button type="button" variant="outline" onClick={() => setForm(null)}>{tr('Cancel', 'Cancelar')}</Button>
						<Button type="submit" disabled={save.isPending || !form.front.trim() || !form.back.trim()}>{save.isPending ? tr('Saving…', 'Salvando…') : tr('Save card', 'Salvar cartão')}</Button>
					</div>
				</form>
			)}

			{cards.isPending ? (
				<p role="status" className="text-sm text-muted-foreground">{tr('Loading cards…', 'Carregando cartões…')}</p>
			) : mode === 'review' ? (
				current ? (
					<div className="space-y-4 rounded-lg border bg-card p-6" aria-live="polite">
						<p className="text-lg font-medium whitespace-pre-wrap">{current.front}</p>
						{revealed ? (
							<>
								<p className="border-t pt-4 whitespace-pre-wrap">{current.back}</p>
								<div className="flex flex-wrap gap-2">
									<Button variant="outline" onClick={() => review.mutate({ cardId: current.cardId, rating: 'again' })} disabled={review.isPending} aria-keyshortcuts="1">{tr('Again (1)', 'De novo (1)')}</Button>
									<Button onClick={() => review.mutate({ cardId: current.cardId, rating: 'good' })} disabled={review.isPending} aria-keyshortcuts="2">{tr('Good (2)', 'Bom (2)')}</Button>
								</div>
							</>
						) : (
							<Button onClick={() => setRevealed(true)} aria-keyshortcuts="Enter">{tr('Show answer', 'Mostrar resposta')}</Button>
						)}
					</div>
				) : (
					<p className="text-sm text-muted-foreground">{all.length ? tr('Nothing to review right now. Cards come back when they are due.', 'Nada para revisar agora. Os cartões voltam quando vencerem.') : tr('No cards in this lesson yet. Turn a key idea into a question to review it later.', 'Ainda não há cartões nesta aula. Transforme uma ideia-chave em pergunta para revisar depois.')}</p>
				)
			) : (
				<ul className="space-y-2">
					{all.map((card) => (
						<li key={card.cardId} className="flex items-start gap-2 rounded-lg border bg-card p-3">
							<div className="min-w-0 flex-1 space-y-1">
								<p className="font-medium whitespace-pre-wrap">{card.front}</p>
								<p className="text-sm text-muted-foreground whitespace-pre-wrap">{card.back}</p>
								<p className="text-xs text-muted-foreground">{card.positionSeconds === null ? '' : `${tr('At', 'Em')} ${formatDuration(card.positionSeconds)} · `}{dueLabel(card.nextDue, now)}</p>
							</div>
							<Button variant="ghost" size="icon" onClick={() => setForm({ cardId: card.cardId, front: card.front, back: card.back, position: card.positionSeconds })} aria-label={tr(`Edit card: ${card.front}`, `Editar cartão: ${card.front}`)}><Pencil /></Button>
							<Button variant="ghost" size="icon" onClick={() => remove.mutate(card.cardId)} disabled={remove.isPending} aria-label={tr(`Delete card: ${card.front}`, `Excluir cartão: ${card.front}`)}><Trash2 /></Button>
						</li>
					))}
				</ul>
			)}
		</div>
	);
}
