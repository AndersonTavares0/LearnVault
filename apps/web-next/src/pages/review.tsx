import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, ArrowUpRight, Download, Layers, Pencil, Search, Trash2, Upload } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type ChangeEvent, type FormEvent, type KeyboardEvent } from 'react';
import { Link, useSearchParams } from 'react-router';
import { ConfirmDelete } from '@/components/confirm-delete';
import { CoursePicker, GONE } from '@/components/course-picker';
import { Eyebrow, Ring } from '@/components/panel';
import { Button, buttonVariants } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { useLocale } from '@/hooks/use-locale';
import { MAX_IMPORT_CARDS, REVIEW_RATINGS, cardsToTsv, parseCards, previewIntervalLabel, ratingLabel } from '@/lib/cards';
import { formatCount } from '@/lib/catalog';
import { getLocale, tr } from '@/lib/i18n';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findCourse, findLessonLocation, href } from '@/lib/router';
import { normalizeForSearch } from '@/lib/search';
import { transport, type CatalogNode, type Flashcard, type ReviewRating } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useCatalog } from '@/app/data';
import { QueryFailure } from '@/components/query-failure';
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
	if (cards.isError || (catalog.isError && !catalog.data)) {
		const failed = cards.isError ? cards : catalog;
		return <QueryFailure message={tr('Could not load your review cards.', 'Não foi possível carregar seus cartões de revisão.')} retry={() => void failed.refetch()} />;
	}
	if (courseId === ALL) return <Deck courseId={ALL} title={tr('All courses', 'Todos os cursos')} cards={all} now={now} />;
	if (courseId) {
		const title = courseId === GONE ? tr('Lessons no longer in the library', 'Aulas que não estão mais na biblioteca') : (catalog.data?.nodes ?? []).find((node) => node.courseId === courseId)?.title ?? courseId;
		return <Deck courseId={courseId} title={title} cards={all.filter((card) => courseOf(card) === courseId)} now={now} />;
	}
	if (all.length === 0) {
		const firstCourse = catalog.data?.nodes.find((node) => node.role === 'course');
		return (
			<div className="space-y-8">
				<PageHeader icon={Layers} title={tr('Cards to review', 'Cartões para revisar')} description={tr('Revisit what you learned in your lessons.', 'Relembre o que você estudou nas aulas.')} />
				<div className="study-empty">
					<span aria-hidden="true" className="mb-1 inline-flex size-11 items-center justify-center rounded-xl border border-primary/20 bg-brand-soft text-brand-ink"><Layers className="size-5" /></span>
					<h2 className="text-lg font-semibold tracking-tight">{tr('No cards yet', 'Nenhum cartão ainda')}</h2>
					<p className="text-sm text-muted-foreground">{tr('Create a card during a lesson. It will appear here when it is time to review.', 'Crie um cartão durante uma aula. Ele aparecerá aqui na hora de revisar.')}</p>
					<div className="flex flex-wrap items-center justify-start gap-2 pt-2">
						{firstCourse?.courseId && <Link to={`/course/${encodeURIComponent(firstCourse.courseId)}`} className={buttonVariants({ variant: 'default' })}>{tr(`Open ${firstCourse.title}`, `Abrir ${firstCourse.title}`)}</Link>}
						<Link to="/" className={buttonVariants({ variant: firstCourse?.courseId ? 'outline' : 'default' })}>{tr('Browse your library', 'Explorar a biblioteca')}</Link>
					</div>
				</div>
			</div>
		);
	}
	// The server count matches the queue; the deck sum is the offline/failed fallback.
	const due = dueCount.data ?? [...decks.values()].reduce((sum, deck) => sum + deck.due, 0);
	return (
		<CoursePicker
			icon={Layers}
			title={tr('Choose a course to review its cards', 'Escolha um curso para revisar os cartões')}
			description={due === 0 ? tr('Nothing is due right now. Cards come back when they are due.', 'Nada vence agora. Os cartões voltam quando vencerem.') : formatCount(due, tr('card is due', 'cartão vence agora'), tr('cards are due', 'cartões vencem agora'))}
			actions={<>
				{due > 0 && <Link to={`/review?course=${ALL}`} className={cn(buttonVariants())}>{tr(`Review all due cards (${due})`, `Revisar todos os cartões a vencer (${due})`)}</Link>}
				{exportUrl && <a href={exportUrl} download="learnvault-cards.tsv" className={cn(buttonVariants({ variant: 'outline' }))}><Download className="size-4" aria-hidden="true" />{tr('Export for Anki', 'Exportar para o Anki')}</a>}
			</>}
			footer={(id) => {
				const deck = decks.get(id);
				const dueCount = deck?.due ?? 0;
				const totalCount = deck?.total ?? 0;
				return `${tr(`${dueCount} due`, `${dueCount} ${dueCount === 1 ? 'vence' : 'vencem'}`)} · ${formatCount(totalCount, tr('card', 'cartão'), tr('cards', 'cartões'))}`;
			}}
			to={(id) => `/review?course=${encodeURIComponent(id)}`}
			hasGone={decks.has(GONE)}
		/>
	);
}

/** Collects playable lesson `(mediaId, title)` options within a course for card import. */
function collectCourseLessons(nodes: CatalogNode[]): { mediaId: string; title: string }[] {
	const out: { mediaId: string; title: string }[] = [];
	for (const node of nodes) {
		if (node.role === 'lesson') {
			const media = [...node.files, ...node.looseFiles].find((file) => file.kind === 'media' && file.mediaId)?.mediaId;
			if (media) out.push({ mediaId: media, title: node.title });
		} else {
			out.push(...collectCourseLessons(node.children));
		}
	}
	return out;
}

/** One deck: due SRS review, cram/practice mode, and card browser/editor. */
function Deck({ courseId, title, cards, now }: { courseId: string; title: string; cards: Flashcard[]; now: number }) {
	const client = useQueryClient();
	const catalog = useCatalog();
	const due = useMemo(() => cards.filter((card) => card.nextDue <= now), [cards, now]);
	// The session works through the queued cards when it started, so its count stays put.
	const [queue, setQueue] = useState<string[] | null>(null);
	const [practice, setPractice] = useState(false);
	const [done, setDone] = useState(0);
	const [revealed, setRevealed] = useState(false);
	const [filter, setFilter] = useState<'all' | 'due' | 'scheduled'>('all');
	const [query, setQuery] = useState('');
	const [editing, setEditing] = useState<{ cardId: string; front: string; back: string } | null>(null);
	const [deleting, setDeleting] = useState<Flashcard | null>(null);
	const [importMessage, setImportMessage] = useState('');
	const courseLessons = useMemo(() => {
		if (courseId === ALL || courseId === GONE) return [];
		const course = findCourse(catalog.data ?? null, courseId);
		return course ? collectCourseLessons(course.children) : [];
	}, [catalog.data, courseId]);
	const [importTargetMedia, setImportTargetMedia] = useState('');
	const targetMediaId = importTargetMedia && courseLessons.some((l) => l.mediaId === importTargetMedia) ? importTargetMedia : (courseLessons[0]?.mediaId ?? '');
	const importFileRef = useRef<HTMLInputElement>(null);

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
	const saveEdit = useMutation({
		mutationFn: ({ cardId, front, back }: { cardId: string; front: string; back: string }) => transport.updateFlashcard(cardId, front, back),
		onSuccess: (updated) => {
			client.setQueryData<Flashcard[]>(CARDS, (list = []) => list.map((item) => (item.cardId === updated.cardId ? updated : item)));
			setEditing(null);
		}
	});
	const remove = useMutation({
		mutationFn: (cardId: string) => transport.deleteFlashcard(cardId),
		onSuccess: (_, cardId) => {
			client.setQueryData<Flashcard[]>(CARDS, (list = []) => list.filter((item) => item.cardId !== cardId));
			void client.invalidateQueries({ queryKey: ['flashcards', 'due'] });
			setDeleting(null);
		}
	});
	const importCards = useMutation({
		mutationFn: async (file: File) => {
			const parsed = parseCards(await file.text());
			const batch = parsed.slice(0, MAX_IMPORT_CARDS);
			const addedCards: Flashcard[] = [];
			for (const item of batch) addedCards.push(await transport.createFlashcard(targetMediaId, item.front, item.back, null));
			return { addedCards, skipped: parsed.length - batch.length };
		},
		onSuccess: ({ addedCards, skipped }) => {
			if (addedCards.length > 0) {
				client.setQueryData<Flashcard[]>(CARDS, (list = []) => [...addedCards, ...list]);
				void client.invalidateQueries({ queryKey: ['flashcards', 'due'] });
			}
			setImportMessage(addedCards.length === 0
				? tr('No cards found in that file. Use one card per line: front, a tab, then back.', 'Nenhum cartão encontrado no arquivo. Use um cartão por linha: frente, uma tabulação e o verso.')
				: tr(`${addedCards.length} cards imported.${skipped ? ` ${skipped} more were left out.` : ''}`, `${addedCards.length} cartões importados.${skipped ? ` Outros ${skipped} ficaram de fora.` : ''}`));
		},
		onError: () => setImportMessage('')
	});

	// A card deleted elsewhere while the session runs is skipped instead of ending the session early.
	const live = useMemo(() => (queue ?? []).filter((id) => cards.some((card) => card.cardId === id)), [queue, cards]);
	const current = queue && done < live.length ? cards.find((card) => card.cardId === live[done]) ?? null : null;
	const wrapper = useRef<HTMLDivElement>(null);
	// The button that was focused disappears when the answer shows, so focus moves to the session itself and 1-4 keep working.
	useEffect(() => { if (revealed) wrapper.current?.focus(); }, [revealed]);
	const location = current ? findLessonLocation(catalog.data ?? null, current.mediaId) : null;
	const leave = () => { setQueue(null); setPractice(false); setDone(0); setRevealed(false); };
	const startSession = (list: Flashcard[], isPractice: boolean) => {
		setPractice(isPractice);
		setQueue(list.map((card) => card.cardId));
		setDone(0);
		setRevealed(false);
	};
	const rate = (rating: ReviewRating) => {
		if (!current || review.isPending) return;
		if (practice) {
			if (rating === 'again') setQueue((q) => (q ? [...q, current.cardId] : q));
			setRevealed(false);
			setDone((value) => value + 1);
			return;
		}
		review.mutate({ cardId: current.cardId, rating });
	};
	const onKeyDown = (event: KeyboardEvent) => {
		if (!current || review.isPending) return;
		if (!revealed && (event.key === ' ' || event.key === 'Enter')) { event.preventDefault(); setRevealed(true); }
		else if (revealed && event.key === '1') { event.preventDefault(); rate('again'); }
		else if (revealed && event.key === '2') { event.preventDefault(); rate('hard'); }
		else if (revealed && event.key === '3') { event.preventDefault(); rate('good'); }
		else if (revealed && event.key === '4') { event.preventDefault(); rate('easy'); }
	};

	const needle = normalizeForSearch(query);
	const visibleCards = useMemo(() => cards.filter((card) => {
		if (filter === 'due' && card.nextDue > now) return false;
		if (filter === 'scheduled' && card.nextDue <= now) return false;
		if (needle && !normalizeForSearch(`${card.front} ${card.back}`).includes(needle)) return false;
		return true;
	}), [cards, filter, needle, now]);

	const onImportFile = (event: ChangeEvent<HTMLInputElement>) => {
		const file = event.target.files?.[0];
		event.target.value = '';
		if (file && targetMediaId) importCards.mutate(file);
	};
	const submitEdit = (event: FormEvent) => {
		event.preventDefault();
		if (editing && editing.front.trim() && editing.back.trim() && !saveEdit.isPending) {
			saveEdit.mutate({ cardId: editing.cardId, front: editing.front.trim(), back: editing.back.trim() });
		}
	};

	if (queue) {
		return (
			// eslint-disable-next-line jsx-a11y/no-static-element-interactions -- keyboard shortcuts for the review controls inside
			<div ref={wrapper} tabIndex={-1} className="mx-auto max-w-3xl space-y-6 outline-none" onKeyDown={onKeyDown}>
				<div className="flex flex-wrap items-center gap-4">
					<Button variant="outline" size="icon" className="size-11 rounded-full" onClick={leave} aria-label={tr('Close', 'Fechar')}><ArrowLeft /></Button>
					<div className="min-w-0 flex-1">
						<div className="flex flex-wrap items-center gap-2">
							<p className="truncate font-heading text-lg font-bold">{title}</p>
							{practice && <span className="rounded-full border border-[var(--study-rule)] bg-panel px-2.5 py-0.5 text-xs font-medium text-muted-foreground">{tr('Practice · schedule unchanged', 'Prática · agenda mantida')}</span>}
						</div>
						{location && <p className="truncate text-sm text-muted-foreground">{courseId === ALL && `${location.courseTitle} · `}{location.lesson.title}{current?.positionSeconds == null ? '' : ` · ${formatDuration(current.positionSeconds)}`}</p>}
					</div>
					<div className="flex items-center gap-3">
						<Ring size={52} value={live.length ? done / live.length : 0} label={tr(`${done} of ${live.length} cards reviewed`, `${done} de ${live.length} cartões revisados`)} center={<span className="text-[0.7rem] font-bold tabular-nums">{Math.min(done + 1, live.length)}/{live.length}</span>} />
						<p className="text-sm"><span className="font-bold tabular-nums">{live.length - done}</span> <span className="text-muted-foreground">{tr('left', 'restantes')}</span></p>
					</div>
				</div>
				{review.error && <p role="alert" className="text-sm text-destructive">{review.error.message}</p>}
				{current ? (
					<>
						<div className="relative isolate">
							<div className="study-surface relative flex min-h-[20rem] flex-col p-6 sm:p-10" aria-live="polite">
								<div className="flex items-center justify-between border-b border-[var(--study-rule)] pb-4">
									<span className="study-timecode text-muted-foreground"># {done + 1}</span>
									<Eyebrow>{revealed ? tr('Answer', 'Resposta') : tr('Question', 'Pergunta')}</Eyebrow>
								</div>
								<div className="my-auto space-y-6 py-8 text-center">
									<h1 className="font-heading text-xl leading-snug font-semibold whitespace-pre-wrap sm:text-2xl">{current.front}</h1>
									{revealed && (
										<>
											<p className="mx-auto max-w-prose border-t border-[var(--study-rule)] pt-6 text-xl leading-relaxed whitespace-pre-wrap">{current.back}</p>
											{location && <Link to={href('course', location.courseId, location.lesson.path, current.positionSeconds).replace(/^#/, '')} className={buttonVariants({ variant: 'ghost', size: 'sm' })}>{current.positionSeconds === null ? tr('Open the lesson', 'Abrir a aula') : tr(`Open the lesson at ${formatDuration(current.positionSeconds)}`, `Abrir a aula em ${formatDuration(current.positionSeconds)}`)}<ArrowUpRight aria-hidden="true" /></Link>}
										</>
									)}
								</div>
								{!revealed && <p className="text-center text-sm text-muted-foreground">{tr('Press Space, or use the button, to show the answer.', 'Aperte Espaço, ou use o botão, para mostrar a resposta.')}</p>}
							</div>
						</div>
						{!practice && <ReviewHistory cardId={current.cardId} nextDue={current.nextDue} now={now} />}
						{revealed ? (
							practice ? (
								<div className="mx-auto grid max-w-md grid-cols-2 gap-3">
									<Button variant="outline" size="lg" onClick={() => rate('again')} aria-keyshortcuts="1">{tr('Review again', 'Revisar de novo')}</Button>
									<Button size="lg" onClick={() => rate('good')} aria-keyshortcuts="3">{tr('Got it', 'Acertei')}</Button>
								</div>
							) : (
								<div className="mx-auto grid max-w-xl grid-cols-2 gap-2.5 sm:grid-cols-4">
									{REVIEW_RATINGS.map((rating, index) => (
										<Button
											key={rating}
											variant={rating === 'good' ? 'default' : 'outline'}
											size="lg"
											className="h-auto flex-col gap-0.5 py-2.5"
											disabled={review.isPending}
											onClick={() => rate(rating)}
											aria-keyshortcuts={String(index + 1)}
										>
											<span>{ratingLabel(rating)}</span>
											<span className="text-[11px] font-normal opacity-80">{previewIntervalLabel(current.intervalDays, rating)}</span>
										</Button>
									))}
								</div>
							)
						) : (
							<div className="flex justify-center">
								{/* eslint-disable-next-line jsx-a11y/no-autofocus -- the session is keyboard-first; focus starts on its one action */}
								<Button size="lg" className="px-10" autoFocus onClick={() => setRevealed(true)} aria-keyshortcuts="Space Enter">{tr('Show answer', 'Mostrar resposta')}</Button>
							</div>
						)}
						{revealed && !practice && <p className="text-center text-sm text-muted-foreground">{tr('1 Again · 2 Hard · 3 Good · 4 Easy', '1 De novo · 2 Difícil · 3 Bom · 4 Fácil')}</p>}
					</>
				) : (
					<div className="space-y-4 py-10 text-center">
						<h1 className="font-heading text-3xl font-semibold">{tr('Session done', 'Sessão concluída')}</h1>
						<p className="text-muted-foreground">{formatCount(done, tr('card reviewed.', 'cartão revisado.'), tr('cards reviewed.', 'cartões revisados.'))}</p>
						<div className="flex flex-wrap justify-center gap-2">
							<Button onClick={leave}>{tr('Back to the deck', 'Voltar ao baralho')}</Button>
							{cards.length > 0 && <Button variant="outline" onClick={() => startSession(cards, true)}>{tr('Practice all cards', 'Praticar todos')}</Button>}
						</div>
					</div>
				)}
			</div>
		);
	}

	return (
		<div className="space-y-8">
			<Link to="/review" className="inline-flex min-h-11 items-center gap-1.5 text-sm font-medium text-muted-foreground hover:text-foreground"><ArrowLeft className="size-4" aria-hidden="true" />{tr('All courses', 'Todos os cursos')}</Link>
			<PageHeader icon={Layers} title={courseId === ALL ? tr('All due cards', 'Todos os cartões a vencer') : tr(`${title} cards`, `Cartões de ${title}`)} description={formatCount(cards.length, courseId === ALL ? tr('card across all courses.', 'cartão em todos os cursos.') : tr('card in this deck.', 'cartão neste baralho.'), courseId === ALL ? tr('cards across all courses.', 'cartões em todos os cursos.') : tr('cards in this deck.', 'cartões neste baralho.'))} />

			<Tabs defaultValue="study">
				<TabsList variant="line" aria-label={tr('Deck sections', 'Seções do baralho')}>
					<TabsTrigger value="study">{tr('Study', 'Estudar')}</TabsTrigger>
					<TabsTrigger value="browse">{tr('Browse cards', 'Gerenciar cartões')} ({cards.length})</TabsTrigger>
				</TabsList>
				<TabsContent value="study" className="space-y-6 pt-5">
					<section aria-labelledby="due-heading" className="flex flex-wrap items-center justify-between gap-6 border-t border-[var(--study-rule)] pt-6">
						<div>
							<h2 id="due-heading" className="text-[15px] font-semibold">{due.length === 0 ? tr('Nothing is due right now', 'Nada vence agora') : formatCount(due.length, tr('card is due', 'cartão vence agora'), tr('cards are due', 'cartões vencem agora'))}</h2>
							<p className="mt-1 text-sm text-muted-foreground">{due.length === 0 ? tr('Cards come back when they are due, or practice the whole deck anytime.', 'Os cartões voltam quando vencerem, ou pratique o baralho todo a qualquer hora.') : tr(`About ${Math.max(1, Math.round(due.length / 3))} min.`, `Cerca de ${Math.max(1, Math.round(due.length / 3))} min.`)}</p>
						</div>
						<div className="flex flex-wrap gap-2">
							{due.length > 0 && <Button size="lg" className="px-8" onClick={() => startSession(due, false)}>{tr('Start review', 'Começar revisão')}</Button>}
							{cards.length > 0 && <Button size="lg" variant={due.length === 0 ? 'default' : 'outline'} onClick={() => startSession(cards, true)}>{tr(`Practice all (${cards.length})`, `Praticar todos (${cards.length})`)}</Button>}
						</div>
					</section>
					{courseId !== GONE && courseId !== ALL && <Link to={`/course/${encodeURIComponent(courseId)}`} className={buttonVariants({ variant: 'outline' })}>{tr('Open the course', 'Abrir o curso')}<ArrowUpRight aria-hidden="true" /></Link>}
				</TabsContent>
				<TabsContent value="browse" className="space-y-4 pt-5">
					<div className="flex flex-wrap items-center justify-between gap-3">
						<div className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
							<div role="search" className="relative min-w-0 flex-1 sm:max-w-xs">
								<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
								<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder={tr('Search cards', 'Buscar cartões')} aria-label={tr('Search cards', 'Buscar cartões')} className="pl-9" />
							</div>
							<div role="group" aria-label={tr('Card filter', 'Filtro de cartões')} className="inline-flex rounded-[var(--radius)] border border-[var(--study-rule)] bg-panel p-0.5">
								{([['all', tr('All', 'Todos')], ['due', tr('Due', 'A vencer')], ['scheduled', tr('Scheduled', 'Agendados')]] as const).map(([value, label]) => (
									<button key={value} type="button" aria-pressed={filter === value} onClick={() => setFilter(value)} className={cn('min-h-9 rounded-[calc(var(--radius)-2px)] px-2.5 text-xs font-medium transition-colors', filter === value ? 'bg-card font-semibold text-foreground shadow-2xs' : 'text-muted-foreground hover:text-foreground')}>{label}</button>
								))}
							</div>
						</div>
						{courseLessons.length > 0 && (
							<div className="flex flex-wrap items-center gap-2">
								{courseLessons.length > 1 && (
									<select aria-label={tr('Lesson for imported cards', 'Aula para cartões importados')} value={targetMediaId} onChange={(event) => setImportTargetMedia(event.target.value)} className="h-9 max-w-[13rem] truncate rounded-[var(--radius)] border border-input bg-background px-2 text-xs">
										{courseLessons.map((lesson) => <option key={lesson.mediaId} value={lesson.mediaId}>{lesson.title}</option>)}
									</select>
								)}
								<input ref={importFileRef} type="file" accept=".txt,.tsv,.csv,text/plain,text/csv,text/tab-separated-values" className="sr-only" tabIndex={-1} aria-label={tr('Card file to import', 'Arquivo de cartões para importar')} onChange={onImportFile} />
								<Button type="button" variant="outline" size="sm" disabled={importCards.isPending || !targetMediaId} onClick={() => importFileRef.current?.click()}>
									<Upload className="size-3.5" aria-hidden="true" />{importCards.isPending ? tr('Importing…', 'Importando…') : tr('Import cards', 'Importar cartões')}
								</Button>
							</div>
						)}
					</div>
					{importMessage && <p role="status" className="text-sm text-muted-foreground">{importMessage}</p>}
					{(importCards.error || saveEdit.error || remove.error) && (
						<p role="alert" className="text-sm text-destructive">{importCards.error?.message || saveEdit.error?.message || remove.error?.message}</p>
					)}
					{editing && (
						<form onSubmit={submitEdit} className="space-y-3 rounded-lg border bg-card p-4" aria-label={tr('Edit card', 'Editar cartão')}>
							<div className="space-y-1.5">
								<Label htmlFor="review-edit-front">{tr('Front', 'Frente')}</Label>
								<textarea id="review-edit-front" rows={2} maxLength={4000} required value={editing.front} onChange={(event) => setEditing({ ...editing, front: event.target.value })} className="w-full rounded-lg border border-input bg-background p-2.5 text-sm" />
							</div>
							<div className="space-y-1.5">
								<Label htmlFor="review-edit-back">{tr('Back', 'Verso')}</Label>
								<textarea id="review-edit-back" rows={3} maxLength={4000} required value={editing.back} onChange={(event) => setEditing({ ...editing, back: event.target.value })} className="w-full rounded-lg border border-input bg-background p-2.5 text-sm" />
							</div>
							<div className="flex justify-end gap-2">
								<Button type="button" variant="outline" size="sm" onClick={() => setEditing(null)}>{tr('Cancel', 'Cancelar')}</Button>
								<Button type="submit" size="sm" disabled={saveEdit.isPending || !editing.front.trim() || !editing.back.trim()}>{saveEdit.isPending ? tr('Saving…', 'Salvando…') : tr('Save card', 'Salvar cartão')}</Button>
							</div>
						</form>
					)}
					{visibleCards.length === 0 ? (
						<p className="text-sm text-muted-foreground">{tr('No cards match this filter.', 'Nenhum cartão corresponde a este filtro.')}</p>
					) : (
						<ul className="divide-y divide-[var(--study-rule)] rounded-lg border bg-card">
							{visibleCards.map((card) => {
								const loc = findLessonLocation(catalog.data ?? null, card.mediaId);
								const isDue = card.nextDue <= now;
								return (
									<li key={card.cardId} className="flex items-start gap-3 p-4">
										<div className="min-w-0 flex-1 space-y-1">
											<p className="font-medium whitespace-pre-wrap">{card.front}</p>
											<p className="text-sm text-muted-foreground whitespace-pre-wrap">{card.back}</p>
											<div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
												<span className={isDue ? 'font-medium text-primary' : undefined}>{isDue ? tr('Due now', 'Vence agora') : tr(`Due ${new Date(card.nextDue * 1000).toLocaleDateString(getLocale(), { dateStyle: 'medium' })}`, `Vence em ${new Date(card.nextDue * 1000).toLocaleDateString(getLocale(), { dateStyle: 'medium' })}`)}</span>
												{loc && <Link to={href('course', loc.courseId, loc.lesson.path, card.positionSeconds).replace(/^#/, '')} className="hover:underline">{loc.lesson.title}{card.positionSeconds !== null ? ` · ${formatDuration(card.positionSeconds)}` : ''}</Link>}
											</div>
										</div>
										<Button type="button" variant="ghost" size="icon" onClick={() => setEditing({ cardId: card.cardId, front: card.front, back: card.back })} aria-label={tr(`Edit card: ${card.front}`, `Editar cartão: ${card.front}`)}><Pencil className="size-4" aria-hidden="true" /></Button>
										<Button type="button" variant="ghost" size="icon" disabled={remove.isPending} onClick={() => setDeleting(card)} aria-label={tr(`Delete card: ${card.front}`, `Excluir cartão: ${card.front}`)}><Trash2 className="size-4" aria-hidden="true" /></Button>
									</li>
								);
							})}
						</ul>
					)}
				</TabsContent>
			</Tabs>
			<ConfirmDelete
				open={deleting !== null}
				onOpenChange={(open) => !open && setDeleting(null)}
				title={tr('Delete this card?', 'Excluir este cartão?')}
				description={tr('The card is removed for good. The lesson and your progress stay as they are.', 'O cartão será removido para sempre. A aula e o progresso continuam como estão.')}
				label={tr('Delete card', 'Excluir cartão')}
				pending={remove.isPending}
				onConfirm={() => { if (deleting) remove.mutate(deleting.cardId); }}
			/>
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
		<section aria-labelledby={`reviews-${cardId}`} className="border-t border-[var(--study-rule)] pt-3 text-sm">
			<div className="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
				<h2 id={`reviews-${cardId}`} className="font-semibold">{formatCount(total, tr('review', 'revisão'), tr('reviews', 'revisões'))}</h2>
				<p className="text-xs text-muted-foreground">
					{tr(`Last ${date(reviews[0]?.reviewedAt ?? 0)} · next ${nextDue <= now ? 'now' : date(nextDue)}`, `Última em ${date(reviews[0]?.reviewedAt ?? 0)} · próxima ${nextDue <= now ? 'agora' : date(nextDue)}`)}
				</p>
			</div>
			<ol className="mt-2 space-y-1">
				{reviews.map((review) => (
					<li key={review.reviewId} className="flex flex-wrap items-baseline gap-x-2 text-xs">
						<span className="font-medium">{ratingLabel(review.rating)}</span>
						<span className="text-muted-foreground">{tr(`${review.previousIntervalDays} d before`, `${review.previousIntervalDays} d antes`)}</span>
						<time dateTime={new Date(review.reviewedAt * 1000).toISOString()} className="ml-auto text-muted-foreground">{date(review.reviewedAt)}</time>
					</li>
				))}
			</ol>
		</section>
	);
}
