import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Download, Layers } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { Link, useSearchParams } from 'react-router';
import { CoursePicker, GONE } from '@/components/course-picker';
import { Eyebrow, Hero, Ring } from '@/components/panel';
import { Button, buttonVariants } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { cardsToTsv } from '@/lib/cards';
import { formatCount } from '@/lib/catalog';
import { getLocale, tr } from '@/lib/i18n';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findLessonLocation, href } from '@/lib/router';
import { transport, type Flashcard, type ReviewRating } from '@/lib/transport';
import { useCatalog } from '@/app/data';
import { PageHeader } from '@/app/shell';

const nowSeconds = () => Math.floor(Date.now() / 1000);
const CARDS = ['flashcards', 'all'] as const;
/** The review deck that queues every due card, across courses. */
const ALL = '__all';

/**
 * Review: every card you made in your lessons, due when you are about to forget it. Pick a course
 * (one deck per course), then study its due cards one at a time. Cards are made in a lesson; this
 * page only reviews and exports them.
 */
export function ReviewPage() {
	useLocale();
	const catalog = useCatalog();
	const [params] = useSearchParams();
	const courseId = params.get('course');
	const cards = useQuery({ queryKey: CARDS, enabled: studySyncSupported, queryFn: () => transport.listFlashcards() });
	const dueCount = useQuery({ queryKey: ['flashcards', 'due'], enabled: studySyncSupported, queryFn: () => transport.dueFlashcardCount() });
	const [now] = useState(nowSeconds);
	const all = useMemo(() => cards.data ?? [], [cards.data]);
	const courseOf = (card: Flashcard) => findLessonLocation(catalog.data ?? null, card.mediaId)?.courseId ?? GONE;
	const decks = useMemo(() => {
		const map = new Map<string, { due: number; total: number }>();
		for (const card of all) {
			const id = findLessonLocation(catalog.data ?? null, card.mediaId)?.courseId ?? GONE;
			const deck = map.get(id) ?? { due: 0, total: 0 };
			deck.total += 1;
			if (card.nextDue <= now) deck.due += 1;
			map.set(id, deck);
		}
		return map;
	}, [all, catalog.data, now]);

	// The export is a file the browser saves; the object URL lives only while the link is shown.
	const exportUrl = useMemo(() => (all.length > 0 && typeof URL.createObjectURL === 'function' ? URL.createObjectURL(new Blob([cardsToTsv(all)], { type: 'text/tab-separated-values' })) : null), [all]);
	useEffect(() => () => { if (exportUrl) URL.revokeObjectURL(exportUrl); }, [exportUrl]);

	if (!studySyncSupported) return <PageHeader icon={Layers} title={tr('Review', 'Revisão')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	if (cards.isPending || catalog.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading cards…', 'Carregando cartões…')}</p>;
	if (cards.error) return <p role="alert" className="text-destructive">{cards.error.message}</p>;
	if (courseId === ALL) return <Deck courseId={ALL} title={tr('All courses', 'Todos os cursos')} cards={all} now={now} />;
	if (courseId) {
		const title = courseId === GONE ? tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca') : (catalog.data?.nodes ?? []).find((node) => node.courseId === courseId)?.title ?? courseId;
		return <Deck courseId={courseId} title={title} cards={all.filter((card) => courseOf(card) === courseId)} now={now} />;
	}
	if (all.length === 0) {
		return (
			<div className="space-y-8">
				<PageHeader icon={Layers} title={tr('Cards to review', 'Cartões para revisar')} />
				<div className="rounded-[2rem] border border-dashed p-8">
					<p className="font-heading text-2xl font-bold">{tr('No cards yet', 'Nenhum cartão ainda')}</p>
					<p className="mt-2 max-w-prose text-muted-foreground">{tr('Open a lesson and press “Make card”, or import a file in the Cards tab of a lesson. Each card comes back here when it is due.', 'Abra uma aula e use “Criar cartão”, ou importe um arquivo na aba Cartões de uma aula. Cada cartão volta aqui quando vence.')}</p>
				</div>
			</div>
		);
	}
	// The server count matches the queue; the deck sum is the offline/failed fallback.
	const due = dueCount.data ?? [...decks.values()].reduce((sum, deck) => sum + deck.due, 0);
	return (
		<CoursePicker
			icon={Layers}
			title={tr('Choose a course to review its Cards', 'Escolha um curso para revisar seus Cartões')}
			description={due === 0 ? tr('Nothing is due right now. Cards come back when they are due.', 'Nada vence agora. Os cartões voltam quando vencerem.') : formatCount(due, tr('card is due', 'cartão vence agora'), tr('cards are due', 'cartões vencem agora'))}
			actions={<>
				{due > 0 && <Link to={`/review?course=${ALL}`} className={buttonVariants({ className: 'rounded-full' })}>{tr(`Review all due cards (${due})`, `Revisar todos os cartões a vencer (${due})`)}</Link>}
				{exportUrl && <a href={exportUrl} download="learnvault-cards.tsv" className="inline-flex h-11 items-center gap-2 rounded-full border bg-card px-5 text-sm font-medium hover:bg-brand-tint"><Download className="size-4" aria-hidden="true" />{tr('Export for Anki', 'Exportar para o Anki')}</a>}
			</>}
			footer={(id) => tr(`${decks.get(id)?.due ?? 0} due · ${decks.get(id)?.total ?? 0} cards`, `${decks.get(id)?.due ?? 0} vencem · ${decks.get(id)?.total ?? 0} cartões`)}
			to={(id) => `/review?course=${encodeURIComponent(id)}`}
			hasGone={decks.has(GONE)}
		/>
	);
}

/** One deck: how many cards are due, and the focused session that works through them. */
function Deck({ courseId, title, cards, now }: { courseId: string; title: string; cards: Flashcard[]; now: number }) {
	const client = useQueryClient();
	const catalog = useCatalog();
	const due = useMemo(() => cards.filter((card) => card.nextDue <= now), [cards, now]);
	// The session works through the cards that were due when it started, so its count stays put.
	const [queue, setQueue] = useState<string[] | null>(null);
	const [done, setDone] = useState(0);
	const [revealed, setRevealed] = useState(false);
	const review = useMutation({
		mutationFn: ({ cardId, rating }: { cardId: string; rating: ReviewRating }) => transport.reviewFlashcard(cardId, rating),
		onSuccess: (card) => {
			client.setQueryData<Flashcard[]>(CARDS, (list = []) => list.map((item) => (item.cardId === card.cardId ? card : item)));
			void client.invalidateQueries({ queryKey: ['flashcards', 'due'] });
			void client.invalidateQueries({ queryKey: ['flashcard-reviews', card.cardId] });
			setRevealed(false);
			setDone((value) => value + 1);
		}
	});
	// A card deleted elsewhere while the session runs is skipped instead of ending the session early.
	const live = useMemo(() => (queue ?? []).filter((id) => cards.some((card) => card.cardId === id)), [queue, cards]);
	const current = queue && done < live.length ? cards.find((card) => card.cardId === live[done]) ?? null : null;
	const wrapper = useRef<HTMLDivElement>(null);
	// The button that was focused disappears when the answer shows, so focus moves to the session itself and 1 and 2 keep working.
	useEffect(() => { if (revealed) wrapper.current?.focus(); }, [revealed]);
	const location = current ? findLessonLocation(catalog.data ?? null, current.mediaId) : null;
	const leave = () => { setQueue(null); setDone(0); setRevealed(false); };
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
			<div ref={wrapper} tabIndex={-1} className="mx-auto max-w-3xl space-y-6 outline-none" onKeyDown={onKeyDown}>
				<div className="flex flex-wrap items-center gap-4">
					<Button variant="outline" size="icon" className="size-11 rounded-full" onClick={leave} aria-label={tr('Close', 'Fechar')}><ArrowLeft /></Button>
					<div className="min-w-0 flex-1">
						<p className="truncate font-heading text-lg font-bold">{title}</p>
						{location && <p className="truncate text-sm text-muted-foreground">{courseId === ALL && `${location.courseTitle} · `}{location.lesson.title}{current?.positionSeconds == null ? '' : ` · ${formatDuration(current.positionSeconds)}`}</p>}
					</div>
					<div className="flex items-center gap-3 rounded-2xl border bg-card px-4 py-2">
						<Ring size={52} value={live.length ? done / live.length : 0} label={tr(`${done} of ${live.length} cards reviewed`, `${done} de ${live.length} cartões revisados`)} center={<span className="text-[0.7rem] font-bold tabular-nums">{Math.min(done + 1, live.length)}/{live.length}</span>} />
						<p className="text-sm"><span className="font-bold tabular-nums">{live.length - done}</span> <span className="text-muted-foreground">{tr('left', 'restantes')}</span></p>
					</div>
				</div>
				{review.error && <p role="alert" className="text-sm text-destructive">{review.error.message}</p>}
				{current ? (
					<>
						<div className="relative flex min-h-[22rem] flex-col rounded-[2rem] border bg-card p-6 sm:p-10" aria-live="polite">
							<div className="flex items-center justify-between">
								<span className="rounded-full bg-brand-soft px-3 py-1 text-xs font-bold tabular-nums"># {done + 1}</span>
								<Eyebrow>{revealed ? tr('Answer', 'Resposta') : tr('Question', 'Pergunta')}</Eyebrow>
							</div>
							<div className="my-auto space-y-6 py-8 text-center">
								<h1 className="font-heading text-2xl leading-snug font-bold whitespace-pre-wrap sm:text-3xl">{current.front}</h1>
								{revealed && (
									<>
										<p className="mx-auto max-w-prose border-t pt-6 text-lg leading-relaxed whitespace-pre-wrap">{current.back}</p>
										{location && <Link to={href('course', location.courseId, location.lesson.path, current.positionSeconds).replace(/^#/, '')} className="inline-flex min-h-11 items-center font-semibold text-primary hover:underline">{current.positionSeconds === null ? tr('Open the lesson', 'Abrir a aula') : tr(`Open the lesson at ${formatDuration(current.positionSeconds)}`, `Abrir a aula em ${formatDuration(current.positionSeconds)}`)}</Link>}
									</>
								)}
							</div>
							{!revealed && <p className="text-center text-sm text-muted-foreground">{tr('Press Space, or use the button, to show the answer.', 'Aperte Espaço, ou use o botão, para mostrar a resposta.')}</p>}
						</div>
						<ReviewHistory cardId={current.cardId} nextDue={current.nextDue} now={now} />
						{revealed ? (
							<div className="mx-auto grid max-w-md grid-cols-2 gap-3">
								<Button variant="outline" size="lg" className="h-14 rounded-full" disabled={review.isPending} onClick={() => rate('again')} aria-keyshortcuts="1">{tr('Again', 'De novo')}</Button>
								<Button size="lg" className="h-14 rounded-full" disabled={review.isPending} onClick={() => rate('good')} aria-keyshortcuts="2">{tr('Good', 'Bom')}</Button>
							</div>
						) : (
							<div className="flex justify-center">
								{/* eslint-disable-next-line jsx-a11y/no-autofocus -- the session is keyboard-first; focus starts on its one action */}
								<Button size="lg" className="h-14 rounded-full px-10" autoFocus onClick={() => setRevealed(true)} aria-keyshortcuts="Space Enter">{tr('Show answer', 'Mostrar resposta')}</Button>
							</div>
						)}
						{revealed && <p className="text-center text-sm text-muted-foreground">{tr('1 for again, 2 for good.', '1 para de novo, 2 para bom.')}</p>}
					</>
				) : (
					<div className="space-y-4 rounded-[2rem] border bg-card p-10 text-center">
						<h1 className="font-heading text-3xl font-bold">{tr('Session done', 'Sessão concluída')}</h1>
						<p className="text-muted-foreground">{formatCount(done, tr('card reviewed.', 'cartão revisado.'), tr('cards reviewed.', 'cartões revisados.'))}</p>
						<Button className="rounded-full" onClick={leave}>{tr('Back to the deck', 'Voltar ao baralho')}</Button>
					</div>
				)}
			</div>
		);
	}

	return (
		<div className="space-y-8">
			<Link to="/review" className="inline-flex min-h-11 items-center gap-1.5 text-sm font-medium text-muted-foreground hover:text-foreground"><ArrowLeft className="size-4" aria-hidden="true" />{tr('All courses', 'Todos os cursos')}</Link>
			<PageHeader icon={Layers} title={courseId === ALL ? tr('All due cards', 'Todos os cartões a vencer') : tr(`${title} cards`, `Cartões de ${title}`)} description={formatCount(cards.length, courseId === ALL ? tr('card across all courses.', 'cartão em todos os cursos.') : tr('card in this deck.', 'cartão neste baralho.'), courseId === ALL ? tr('cards across all courses.', 'cartões em todos os cursos.') : tr('cards in this deck.', 'cartões neste baralho.'))} />
			<section aria-labelledby="due-heading">
				<Hero className="flex flex-wrap items-center justify-between gap-6">
					<div>
						<h2 id="due-heading" className="font-heading text-3xl font-bold">{due.length === 0 ? tr('Nothing is due right now', 'Nada vence agora') : formatCount(due.length, tr('card is due', 'cartão vence agora'), tr('cards are due', 'cartões vencem agora'))}</h2>
						<p className="mt-1 text-muted-foreground">{due.length === 0 ? tr('Cards come back when they are due.', 'Os cartões voltam quando vencerem.') : tr(`About ${Math.max(1, Math.round(due.length / 3))} min.`, `Cerca de ${Math.max(1, Math.round(due.length / 3))} min.`)}</p>
					</div>
					{due.length > 0 && <Button size="lg" className="h-14 rounded-full px-8" onClick={() => { setQueue(due.map((card) => card.cardId)); setDone(0); setRevealed(false); }}>{tr('Start review', 'Começar revisão')}</Button>}
				</Hero>
			</section>
			{courseId !== GONE && courseId !== ALL && <Link to={`/course/${encodeURIComponent(courseId)}`} className="inline-flex min-h-11 items-center font-semibold text-primary hover:underline">{tr('Open the course', 'Abrir o curso')}</Link>}
		</div>
	);
}

/** Compact history for the card on screen: rating, interval before it, when it happened. */
function ReviewHistory({ cardId, nextDue, now }: { cardId: string; nextDue: number; now: number }) {
	const history = useQuery({ queryKey: ['flashcard-reviews', cardId], queryFn: () => transport.listFlashcardReviews(cardId, 8) });
	const date = (seconds: number) => new Date(seconds * 1000).toLocaleDateString(getLocale(), { dateStyle: 'medium' });

	if (history.isPending) return <p role="status" className="text-sm text-muted-foreground">{tr('Loading review history…', 'Carregando histórico de revisões…')}</p>;
	if (history.error) return <p role="alert" className="text-sm text-destructive">{history.error.message}</p>;
	const reviews = history.data?.items ?? [];
	const total = history.data?.total ?? 0;
	if (total === 0) return <p className="text-sm text-muted-foreground">{tr('This card has not been reviewed yet.', 'Este cartão ainda não foi revisado.')}</p>;

	return (
		<section aria-labelledby={`reviews-${cardId}`} className="rounded-2xl border bg-card px-4 py-3 text-sm">
			<div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
				<h2 id={`reviews-${cardId}`} className="font-semibold">{formatCount(total, tr('review', 'revisão'), tr('reviews', 'revisões'))}</h2>
				<p className="text-xs text-muted-foreground">
					{tr(`Last ${date(reviews[0]?.reviewedAt ?? 0)} · next ${nextDue <= now ? 'now' : date(nextDue)}`, `Última em ${date(reviews[0]?.reviewedAt ?? 0)} · próxima ${nextDue <= now ? 'agora' : date(nextDue)}`)}
				</p>
			</div>
			<ol className="mt-2 space-y-1">
				{reviews.map((review) => (
					<li key={review.reviewId} className="flex flex-wrap items-baseline gap-x-2 text-xs">
						<span className="font-medium">{review.rating === 'again' ? tr('Again', 'De novo') : tr('Good', 'Bom')}</span>
						<span className="text-muted-foreground">{tr(`${review.previousIntervalDays} d before`, `${review.previousIntervalDays} d antes`)}</span>
						<time dateTime={new Date(review.reviewedAt * 1000).toISOString()} className="ml-auto text-muted-foreground">{date(review.reviewedAt)}</time>
					</li>
				))}
			</ol>
		</section>
	);
}
