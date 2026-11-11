import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Download, Layers } from 'lucide-react';
import { useEffect, useMemo, useState, type KeyboardEvent } from 'react';
import { Link } from 'react-router';
import { Button } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { cardsToTsv } from '@/lib/cards';
import { formatCount } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { studySyncSupported } from '@/lib/player';
import { findLessonLocation, href } from '@/lib/router';
import { transport, type Flashcard, type ReviewRating } from '@/lib/transport';
import { useCatalog } from '@/app/data';
import { PageHeader } from '@/app/shell';

const nowSeconds = () => Math.floor(Date.now() / 1000);
const CARDS = ['flashcards', 'all'] as const;

/**
 * Review: every card you made in your lessons, due when you are about to forget it. The page
 * says how many are due, runs the session one card at a time, and lists the decks (one per
 * course). Cards are created in a lesson; this page only reviews and exports them.
 */
export function ReviewPage() {
	useLocale();
	const client = useQueryClient();
	const catalog = useCatalog();
	const cards = useQuery({ queryKey: CARDS, enabled: studySyncSupported, queryFn: () => transport.listFlashcards() });
	// The session works through the cards that were due when it started, so its count stays put.
	const [queue, setQueue] = useState<string[] | null>(null);
	const [done, setDone] = useState(0);
	const [revealed, setRevealed] = useState(false);
	const [now] = useState(nowSeconds);
	const review = useMutation({
		mutationFn: ({ cardId, rating }: { cardId: string; rating: ReviewRating }) => transport.reviewFlashcard(cardId, rating),
		onSuccess: (card) => {
			client.setQueryData<Flashcard[]>(CARDS, (list = []) => list.map((item) => (item.cardId === card.cardId ? card : item)));
			setRevealed(false);
			setDone((value) => value + 1);
		}
	});

	const all = useMemo(() => cards.data ?? [], [cards.data]);
	const due = useMemo(() => all.filter((card) => card.nextDue <= now), [all, now]);
	const decks = useMemo(() => {
		const byCourse = new Map<string, { title: string; courseId: string | null; due: number; total: number }>();
		for (const card of all) {
			const location = findLessonLocation(catalog.data ?? null, card.mediaId);
			const key = location?.courseId ?? '';
			const deck = byCourse.get(key) ?? { title: location?.courseTitle ?? tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca'), courseId: location?.courseId ?? null, due: 0, total: 0 };
			deck.total += 1;
			if (card.nextDue <= now) deck.due += 1;
			byCourse.set(key, deck);
		}
		return [...byCourse.values()].sort((a, b) => b.due - a.due || a.title.localeCompare(b.title));
	}, [all, catalog.data, now]);

	const current = queue && done < queue.length ? all.find((card) => card.cardId === queue[done]) ?? null : null;
	const location = current ? findLessonLocation(catalog.data ?? null, current.mediaId) : null;

	// The export is a file the browser saves; the object URL lives only while the link is shown.
	const exportUrl = useMemo(() => (all.length > 0 && typeof URL.createObjectURL === 'function' ? URL.createObjectURL(new Blob([cardsToTsv(all)], { type: 'text/tab-separated-values' })) : null), [all]);
	useEffect(() => () => { if (exportUrl) URL.revokeObjectURL(exportUrl); }, [exportUrl]);

	if (!studySyncSupported) return <PageHeader title={tr('Review', 'Revisão')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;

	const rate = (rating: ReviewRating) => { if (current && !review.isPending) review.mutate({ cardId: current.cardId, rating }); };
	const onKeyDown = (event: KeyboardEvent) => {
		if (!current || review.isPending) return;
		if (!revealed && (event.key === ' ' || event.key === 'Enter')) { event.preventDefault(); setRevealed(true); }
		else if (revealed && event.key === '1') { event.preventDefault(); rate('again'); }
		else if (revealed && event.key === '2') { event.preventDefault(); rate('good'); }
	};

	if (queue) {
		return (
			// eslint-disable-next-line jsx-a11y/no-static-element-interactions -- keyboard shortcuts for the review controls inside
			<div className="mx-auto max-w-2xl space-y-6" onKeyDown={onKeyDown}>
				<div className="flex items-center gap-4">
					<Button variant="ghost" onClick={() => { setQueue(null); setDone(0); setRevealed(false); }}>{tr('Close', 'Fechar')}</Button>
					<div role="progressbar" aria-valuemin={0} aria-valuemax={queue.length} aria-valuenow={done} aria-label={tr('Cards reviewed', 'Cartões revisados')} className="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
						<div className="h-full rounded-full bg-primary" style={{ width: `${(done / queue.length) * 100}%` }} />
					</div>
					<span className="text-sm text-muted-foreground tabular-nums">{done} / {queue.length}</span>
				</div>
				{review.error && <p role="alert" className="text-sm text-destructive">{review.error.message}</p>}
				{current ? (
					<>
						{location && <p className="text-sm text-muted-foreground">{location.courseTitle} · {location.lesson.title}</p>}
						<div className="space-y-6 rounded-2xl border bg-card p-6 sm:p-10" aria-live="polite">
							<h1 className="font-serif text-2xl leading-snug whitespace-pre-wrap sm:text-3xl">{current.front}</h1>
							{revealed && (
								<>
									<p className="border-t pt-6 font-serif text-lg leading-relaxed whitespace-pre-wrap">{current.back}</p>
									{location && <Link to={href('course', location.courseId, location.lesson.path).replace(/^#/, '')} className="inline-flex min-h-11 items-center font-semibold text-primary hover:underline">{tr('Open the lesson', 'Abrir a aula')}</Link>}
								</>
							)}
						</div>
						{revealed ? (
							<div className="grid grid-cols-2 gap-3">
								<Button variant="outline" size="lg" disabled={review.isPending} onClick={() => rate('again')} aria-keyshortcuts="1">{tr('Again', 'De novo')}</Button>
								<Button size="lg" disabled={review.isPending} onClick={() => rate('good')} aria-keyshortcuts="2">{tr('Good', 'Bom')}</Button>
							</div>
						) : (
							// eslint-disable-next-line jsx-a11y/no-autofocus -- the session is keyboard-first; focus starts on its one action
							<Button size="lg" className="w-full" autoFocus onClick={() => setRevealed(true)} aria-keyshortcuts="Space Enter">{tr('Show answer', 'Mostrar resposta')}</Button>
						)}
						<p className="text-center text-sm text-muted-foreground">{tr('Space shows the answer, then 1 for again or 2 for good.', 'Espaço mostra a resposta; depois 1 para de novo ou 2 para bom.')}</p>
					</>
				) : (
					<div className="space-y-4 rounded-2xl border bg-card p-8 text-center">
						<h1 className="font-serif text-2xl font-bold">{tr('Session done', 'Sessão concluída')}</h1>
						<p className="text-muted-foreground">{formatCount(done, tr('card reviewed.', 'cartão revisado.'), tr('cards reviewed.', 'cartões revisados.'))}</p>
						<Button onClick={() => { setQueue(null); setDone(0); }}>{tr('Back to Review', 'Voltar à Revisão')}</Button>
					</div>
				)}
			</div>
		);
	}

	return (
		<div className="space-y-10">
			<PageHeader
				icon={Layers}
				title={tr('Review', 'Revisão')}
				description={tr('Cards you make in your lessons, brought back just before you forget them.', 'Cartões que você cria nas aulas, trazidos de volta pouco antes de você esquecer.')}
				actions={exportUrl && <a href={exportUrl} download="learnvault-cards.tsv" className="inline-flex h-11 items-center gap-2 rounded-lg border bg-background px-4 text-sm font-medium hover:bg-muted"><Download className="size-4" aria-hidden="true" />{tr('Export for Anki', 'Exportar para o Anki')}</a>}
			/>
			{cards.error && <p role="alert" className="text-destructive">{cards.error.message}</p>}
			{cards.isPending ? (
				<p role="status" className="text-muted-foreground">{tr('Loading cards…', 'Carregando cartões…')}</p>
			) : all.length === 0 ? (
				<div className="rounded-2xl border border-dashed p-8">
					<p className="font-serif text-2xl font-bold">{tr('No cards yet', 'Nenhum cartão ainda')}</p>
					<p className="mt-2 max-w-prose text-muted-foreground">{tr('Open a lesson and press “Make card”, or import a file in the Cards tab of a lesson. Each card comes back here when it is due.', 'Abra uma aula e use “Criar cartão”, ou importe um arquivo na aba Cartões de uma aula. Cada cartão volta aqui quando vence.')}</p>
				</div>
			) : (
				<>
					<section aria-labelledby="due-heading" className="flex flex-wrap items-center justify-between gap-4 rounded-2xl border bg-card p-6 sm:p-8">
						<div>
							<h2 id="due-heading" className="font-serif text-2xl font-bold sm:text-3xl">{due.length === 0 ? tr('Nothing is due right now', 'Nada vence agora') : formatCount(due.length, tr('card is due', 'cartão vence agora'), tr('cards are due', 'cartões vencem agora'))}</h2>
							<p className="mt-1 text-muted-foreground">{due.length === 0 ? tr('Cards come back when they are due.', 'Os cartões voltam quando vencerem.') : tr(`About ${Math.max(1, Math.round(due.length / 3))} min.`, `Cerca de ${Math.max(1, Math.round(due.length / 3))} min.`)}</p>
						</div>
						{due.length > 0 && <Button size="lg" onClick={() => { setQueue(due.map((card) => card.cardId)); setDone(0); setRevealed(false); }}>{tr('Start review', 'Começar revisão')}</Button>}
					</section>
					<section aria-labelledby="decks-heading">
						<h2 id="decks-heading" className="mb-1 text-xl font-semibold">{tr('Decks', 'Baralhos')}</h2>
						<table className="w-full text-left">
							<thead className="text-sm text-muted-foreground">
								<tr><th scope="col" className="py-2 font-normal">{tr('One deck per course', 'Um baralho por curso')}</th><th scope="col" className="w-20 py-2 text-right font-normal">{tr('Due', 'Vencem')}</th><th scope="col" className="w-20 py-2 text-right font-normal">{tr('Total', 'Total')}</th></tr>
							</thead>
							<tbody>
								{decks.map((deck) => (
									<tr key={deck.courseId ?? ''} className="border-t">
										<th scope="row" className="py-3 font-semibold">{deck.courseId ? <Link to={href('course', deck.courseId).replace(/^#/, '')} className="inline-flex min-h-11 items-center hover:underline">{deck.title}</Link> : deck.title}</th>
										<td className={deck.due > 0 ? 'py-3 text-right font-semibold text-primary tabular-nums' : 'py-3 text-right text-muted-foreground tabular-nums'}>{deck.due}</td>
										<td className="py-3 text-right text-muted-foreground tabular-nums">{deck.total}</td>
									</tr>
								))}
							</tbody>
						</table>
					</section>
				</>
			)}
		</div>
	);
}
