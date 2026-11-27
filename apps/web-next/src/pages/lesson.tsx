import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, CircleCheck, Circle, FileText, Focus, Search } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { safeStorage } from '@/app/preferences';
import { Curriculum } from '@/components/curriculum';
import { Materials } from '@/components/materials';
import { BreakOverlay } from '@/components/player/break-overlay';
import { Bookmarks } from '@/components/player/bookmarks';
import { Flashcards } from '@/components/player/flashcards';
import { PdfReader } from '@/components/player/pdf-reader';
import { MediaPlayer, type PlayerHandle } from '@/components/player/media-player';
import { StudyNotes, useLessonNotes } from '@/components/player/study-notes';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { usePlayerPreferences } from '@/hooks/use-player-preferences';
import { useLocale } from '@/hooks/use-locale';
import { usePageTitle } from '@/hooks/use-page-title';
import { useStore } from '@/hooks/use-store';
import { courseProgress, displayLessonTitle, lessonStudyMediaId } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { lessonSequence, neighboringLessons } from '@/lib/navigation';
import { findLessonPdf, formatFileSize } from '@/lib/pdf';
import { findLessonText, loadText } from '@/lib/text-lesson';
import { formatDuration, mediaAvailability, shouldAutoAdvance, studySyncSupported } from '@/lib/player';
import { lessonPreferenceKey, type PlayerPreferences } from '@/lib/player-preferences';
import { normalizeForSearch } from '@/lib/search';
import { pomodoro, resetTimer, skipTimer, startTimer } from '@/lib/pomodoro-store';
import { findLesson, findLessonLocation, href } from '@/lib/router';
import { clearPosition, nextAttemptAt, progressOutbox, queuePosition } from '@/lib/progress-outbox';
import { createPositionSaver } from '@/lib/study';
import { transport, type CatalogNode, type FileEntry, type StudyProgress } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { keys, useCatalog, useFavorites, useRecent, useSession, useSetLessonCompleted, useBookmarks } from '@/app/data';

const path = (hash: string) => hash.replace(/^#/, '');
const AUTO_NEXT_SECONDS = 5;

/**
 * Lesson page, laid out as a study workspace: the player and its title on the left; a rail on
 * the right with Notes (and timestamps), Cards and Contents (course map and materials), so what
 * you write and the course map stay beside the video. Focus mode dims everything but the player. When a focus block ends the lesson pauses and a break
 * screen covers the video (docs/design/redesign.md, "Player").
 */
export function LessonPage({ course, courseId, lessonPath: requestedPath }: { course: CatalogNode; courseId: string; lessonPath: string }) {
	useLocale();
	const session = useSession();
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const client = useQueryClient();
	const navigate = useNavigate();
	const [params] = useSearchParams();
	const [preferences, savePreferences] = usePlayerPreferences(session.data?.user?.userId);
	const setCompleted = useSetLessonCompleted();
	const player = useRef<PlayerHandle | null>(null);
	const [time, setTime] = useState(0);
	const [tab, setTab] = useState('notes');
	const [focus, setFocus] = useState(false);
	// Cinema gives the picture the whole width; the course content can also be hidden. Both are remembered.
	const [cinema, setCinema] = useStoredFlag('learnvault-cinema', false);
	const [contentOpen, setContentOpen] = useStoredFlag('learnvault-lesson-content', true);
	const [find, setFind] = useState('');
	const needle = normalizeForSearch(find.trim());
	const found = useMemo(() => (needle ? filterLessons(course.children, needle) : course.children), [course.children, needle]);
	const wide = cinema || !contentOpen;
	const [noteRequest, setNoteRequest] = useState(0);
	const [cardSeed, setCardSeed] = useState<{ key: number; back: string; position: number | null } | null>(null);
	const [breakPhase, setBreakPhase] = useState<'break' | 'over' | null>(null);
	const timer = useStore(pomodoro);
	const [seenRound, setSeenRound] = useState(timer.lastCompleted?.at ?? 0);
	const [countdown, setCountdown] = useState<number | null>(null);

	const lesson = findLesson(catalog.data ?? null, courseId, requestedPath);
	const lessonPath = lesson?.path ?? requestedPath;
	const linkedMediaId = requestedPath !== lessonPath
		? lesson?.media.find((file) => file.path === requestedPath)?.mediaId
		: undefined;
	// A lesson with nothing to play and a PDF is a PDF lesson: the PDF is read here and drives progress.
	const pdf = lesson ? findLessonPdf(lesson) : null;
	const text = lesson ? findLessonText(lesson, Boolean(pdf)) : null;
	const location = useMemo(() => {
		const media = lesson?.media.find((file) => file.mediaId) ?? pdf;
		return media?.mediaId ? findLessonLocation(catalog.data ?? null, media.mediaId) : null;
	}, [catalog.data, lesson, pdf]);
	const { previous, next } = neighboringLessons([course], lessonPath);
	const playableMedia = lesson ? mediaAvailability(lesson.media) === 'playable' : false;
	const preferredMediaId = linkedMediaId ?? preferences.lessons[lessonPreferenceKey(courseId, lessonPath)]?.mediaId;
	const mediaId = pdf?.mediaId ?? (lesson ? lessonStudyMediaId(lesson.media, preferredMediaId) : null);
	const completed = useMemo(() => new Set((recent.data ?? []).filter((item) => item.completed).map((item) => item.mediaId)), [recent.data]);
	const favoriteMedia = useMemo(() => new Set(favorites.data?.mediaIds ?? []), [favorites.data]);
	const progress = useQuery({ queryKey: ['progress', mediaId], enabled: Boolean(mediaId) && studySyncSupported, queryFn: () => transport.studyProgress(mediaId!) });
	const isDone = Boolean(mediaId && (progress.data?.completed ?? completed.has(mediaId)));
	const overall = courseProgress(course, completed);
	// A position the server rejected or never received still beats an older saved one.
	const pending = useStore(progressOutbox);
	const queued = pending.find((entry) => entry.mediaId === mediaId) ?? null;
	const localPosition = queued && !progress.data?.completed &&
		queued.positionSeconds > (progress.data?.recorded ? progress.data.positionSeconds : 0)
		? queued.positionSeconds
		: null;
	// `?t=` (resume links, note timestamps) starts there; otherwise the saved position is
	// applied automatically. A completed lesson starts from the beginning unless `?t=` says otherwise.
	const rawT = params.get('t');
	const queryTime = rawT === null ? Number.NaN : Number(rawT);
	const requestedPosition = Number.isFinite(queryTime) && queryTime >= 0 ? queryTime : null;
	const savedPosition = !requestedPosition && progress.data?.recorded && !progress.data.completed && progress.data.positionSeconds > 5 ? progress.data.positionSeconds : null;
	const resumeAt = requestedPosition ?? (localPosition !== null && localPosition > 5 ? localPosition : null) ?? savedPosition;
	const pdfResumeAt = requestedPosition ?? (localPosition !== null && localPosition > 1 ? localPosition : null) ?? (progress.data?.recorded && !progress.data.completed && progress.data.positionSeconds > 1 ? progress.data.positionSeconds : null);
	const bookmarks = useBookmarks(mediaId);

	// One saver per lesson keeps writes ordered; a failed or coalesced position stays in the
	// durable outbox until the server has it, so leaving the lesson never loses progress.
	const saver = useMemo(() => createPositionSaver(async (id, seconds) => {
		const at = nextAttemptAt();
		try {
			const saved = await transport.recordPosition(id, seconds);
			clearPosition(id, at);
			return saved;
		} catch (cause) {
			queuePosition({ mediaId: id, positionSeconds: seconds, at });
			throw cause;
		}
	}), []);
	const recordPosition = async (id: string, seconds: number) => {
		if (!studySyncSupported) return;
		const result = await saver.request(id, seconds);
		if (result.kind === 'saved') {
			client.setQueryData(['progress', id], result.progress);
			client.setQueryData<StudyProgress[]>(keys.recent, (items = []) => [result.progress, ...items.filter((item) => item.mediaId !== id)]);
		}
	};

	// Retry an unsaved position when the lesson opens and when the tab comes back. A newer
	// server position wins over the queue; an older one leaves the local position as resume point.
	useEffect(() => {
		if (!mediaId || progress.isPending) return;
		const flush = () => {
			// A save in flight already carries a newer position; retrying now would only
			// queue an older one behind it and let it land last.
			if (saver.saving) return;
			const entry = progressOutbox.get().find((item) => item.mediaId === mediaId);
			if (!entry) return;
			const remote = progress.data;
			if (remote?.completed || (remote?.recorded && remote.positionSeconds >= entry.positionSeconds)) {
				clearPosition(mediaId, entry.at);
				return;
			}
			void recordPosition(mediaId, entry.positionSeconds);
		};
		flush();
		const onVisibility = () => { if (document.visibilityState === 'visible') flush(); };
		document.addEventListener('visibilitychange', onVisibility);
		return () => document.removeEventListener('visibilitychange', onVisibility);
	}, [mediaId, progress.isPending, progress.data]); // eslint-disable-line react-hooks/exhaustive-deps -- recordPosition is recreated each render; the effect follows the lesson and its saved progress

	const lessonNotes = useLessonNotes(mediaId);
	const lessonCards = useQuery({ queryKey: ['flashcards', mediaId], enabled: Boolean(mediaId) && studySyncSupported, queryFn: () => transport.listFlashcards(mediaId!) });
	const noteCount = lessonNotes.data?.length ?? 0;
	const cardCount = lessonCards.data?.length ?? 0;
	const goTo = (target: string) => navigate(path(href('course', courseId, target)));
	const finished = (id: string) => {
		if (studySyncSupported) setCompleted.mutate({ mediaId: id, completed: true }, { onSuccess: (updated) => client.setQueryData(['progress', id], updated) });
		if (shouldAutoAdvance(preferences.autoNext, Boolean(next))) setCountdown(AUTO_NEXT_SECONDS);
	};

	// Reset per-lesson state when the lesson changes.
	const [shownLesson, setShownLesson] = useState(requestedPath);
	if (shownLesson !== requestedPath) {
		setShownLesson(requestedPath);
		setCountdown(null);
		setTime(0);
	}

	useEffect(() => {
		if (countdown === null) return;
		if (countdown === 0) {
			if (next) goTo(next.path);
			return;
		}
		const timer = setTimeout(() => setCountdown((value) => (value === null ? null : value - 1)), 1000);
		return () => clearTimeout(timer);
	});

	// A finished focus block pauses the lesson (if the setting is on); a finished break offers to resume.
	// Derived while rendering, like the per-lesson reset above; the effect below only pauses the media.
	const finishedRound = timer.lastCompleted;
	if (finishedRound && finishedRound.at !== seenRound) {
		setSeenRound(finishedRound.at);
		if (finishedRound.mode === 'focus') {
			if (timer.settings.pauseVideoOnRoundEnd) setBreakPhase('break');
		} else if (breakPhase === 'break') {
			setBreakPhase('over');
		}
	}
	// Starting the next focus round from the timer card or the top bar ends the break screen.
	if (breakPhase && timer.mode === 'focus' && timer.status !== 'idle') setBreakPhase(null);
	useEffect(() => {
		if (breakPhase === 'break') player.current?.pause();
	}, [breakPhase]);

	useEffect(() => {
		if (!focus) return;
		const leave = (event: globalThis.KeyboardEvent) => { if (event.key === 'Escape') setFocus(false); };
		document.addEventListener('keydown', leave);
		return () => document.removeEventListener('keydown', leave);
	}, [focus]);

	const missingTitle = tr('Lesson not found', 'Aula não encontrada');
	const displayTitle = lesson ? displayLessonTitle(lesson.title, location?.moduleTitle) : missingTitle;
	usePageTitle(lesson ? `${displayTitle} · ${course.title}` : missingTitle);

	const sequence = useMemo(() => lessonSequence([course]), [course]);
	if (!lesson) {
		return (
			<div className="space-y-4">
				<BackLink course={course} courseId={courseId} />
				<h1 className="text-2xl font-semibold">{missingTitle}</h1>
				<p className="text-muted-foreground">{tr('It may have been renamed or removed. Pick a lesson from the course.', 'Ela pode ter sido renomeada ou removida. Escolha uma aula do curso.')}</p>
			</div>
		);
	}

	const position = sequence.findIndex((item) => item.path === lessonPath);
	const hasCards = studySyncSupported && Boolean(mediaId);
	const hasNotes = hasCards;
	const activeTab = (tab === 'notes' && !hasNotes) || (tab === 'cards' && !hasCards) ? 'contents' : tab;
	const openNotes = () => {
		if (hasNotes) {
			setTab('notes');
			setNoteRequest((value) => value + 1);
		}
	};
	const closeBreak = () => setBreakPhase(null);

	const moduleTitle = location?.moduleTitle === displayTitle ? null : location?.moduleTitle;
	const materialCount = lesson.materials.length + lesson.subtitles.length;
	return (
		<div data-lesson-focus={focus ? 'on' : undefined} className="space-y-3.5 lg:space-y-5">
			<header className="flex items-center justify-between gap-3 lg:flex-wrap lg:gap-x-6 lg:gap-y-3">
				<nav aria-label={tr('Breadcrumb', 'Caminho')} className="flex min-w-0 flex-1 items-center gap-1.5 text-sm lg:flex-none">
					<Link to={path(href('course', courseId))} className="inline-flex min-h-11 min-w-0 items-center gap-1 font-medium text-muted-foreground hover:text-foreground">
						<ChevronLeft className="size-4 shrink-0" aria-hidden="true" /><span className="truncate">{course.title}</span>
					</Link>
					{moduleTitle && <><span aria-hidden="true" className="text-muted-foreground/60">/</span><span className="truncate text-muted-foreground">{moduleTitle}</span></>}
				</nav>
				<div className="flex shrink-0 items-center gap-2">
					<Button variant="outline" className="hidden lg:inline-flex" aria-pressed={focus} onClick={() => setFocus(!focus)}><Focus />{tr('Focus', 'Foco')}</Button>
					<Button variant="outline" className="max-lg:size-11 max-lg:px-0" title={tr('Previous', 'Anterior')} disabled={!previous} onClick={() => previous && goTo(previous.path)}><ChevronLeft /><span className="max-lg:sr-only">{tr('Previous', 'Anterior')}</span></Button>
					<Button variant="outline" className="max-lg:size-11 max-lg:px-0" disabled={!next} title={tr('Jump to the next lesson without marking this one completed', 'Ir para a próxima aula sem marcar esta como concluída')} onClick={() => next && goTo(next.path)}><span className="max-lg:sr-only">{tr('Next', 'Próxima')}</span><ChevronRight /></Button>
					{cinema && (
						<Button variant="secondary" onClick={() => setCinema(false)}>{tr('Exit cinema', 'Sair do cinema')}</Button>
					)}
					<Button variant="outline" className="hidden lg:inline-flex" aria-pressed={contentOpen} onClick={() => setContentOpen(!contentOpen)} aria-label={contentOpen ? tr('Hide contents', 'Ocultar conteúdo') : tr('Show contents', 'Mostrar conteúdo')}>{tr('Contents', 'Conteúdo')}</Button>
				</div>
			</header>

			<div className={cn('grid gap-x-8 gap-y-6', !wide && 'lg:grid-cols-[minmax(0,1fr)_372px]')}>
				{/* The lamp sits behind the player: the one thing to study now. Phones get the picture edge to edge. */}
				<div className="min-w-0 space-y-5">
					<div className={cn('relative max-sm:-mx-5', cinema && 'rounded-[var(--radius-lg)] bg-[#141210] p-2 sm:p-5')}>
						{/* While the break screen is up, the player under it takes no focus or keys. */}
						<div inert={Boolean(breakPhase)}>
						{pdf ? (
							<PdfLesson file={pdf} courseId={courseId} ready={!studySyncSupported || !progress.isPending} initialPage={pdfResumeAt} onPage={(page) => void recordPosition(pdf.mediaId!, page)} preferences={preferences} onPreferencesChange={savePreferences} />
						) : text ? (
							<TextLesson file={text} courseId={courseId} />
						) : (
						<MediaPlayer
							key={`${courseId}/${lessonPath}`}
							lesson={lesson}
							courseId={courseId}
							initialPosition={resumeAt}
							preferredMediaId={preferredMediaId}
							onSelectMedia={linkedMediaId ? () => navigate(path(href('course', courseId, lessonPath)), { replace: true }) : undefined}
							preferences={preferences}
							onPreferencesChange={savePreferences}
							onPosition={recordPosition}
							onEnded={finished}
							onTime={setTime}
							onOpenNotes={openNotes}
							bookmarks={bookmarks.data?.map((bookmark) => ({ id: bookmark.bookmarkId, positionSeconds: bookmark.positionSeconds, label: bookmark.label }))}
							markers={hasNotes ? lessonNotes.data?.map((note) => note.positionSeconds) : undefined}
							cinema={cinema}
							onCinema={() => setCinema(!cinema)}
							handle={(value) => { player.current = value; }}
						/>
						)}
						</div>
						{breakPhase && (
							<BreakOverlay
								phase={breakPhase}
								mode={timer.mode}
								remainingSeconds={timer.remainingSeconds}
								running={timer.status === 'running'}
								notes={noteCount}
								onStart={startTimer}
								onOpenNotes={() => { openNotes(); closeBreak(); }}
								onSkip={() => { skipTimer(); closeBreak(); player.current?.play(); }}
								onEnd={() => { resetTimer(); closeBreak(); }}
								onResume={() => { closeBreak(); player.current?.play(); }}
							/>
						)}
					</div>

					{localPosition !== null && (
						<p role="status" aria-live="polite" className="text-sm text-muted-foreground">
							{tr('Progress is not saved yet. It will retry when the connection is back.', 'O progresso ainda não foi salvo. Vamos tentar de novo quando a conexão voltar.')}
						</p>
					)}

					{countdown !== null && next && (
						<div role="status" aria-live="polite" className="flex flex-wrap items-center gap-3 rounded-lg border bg-card p-4">
							<p className="min-w-0 flex-1">{tr(`Next lesson in ${countdown} s: ${next.title}`, `Próxima aula em ${countdown} s: ${next.title}`)}</p>
							<Button variant="outline" onClick={() => setCountdown(null)}>{tr('Cancel', 'Cancelar')}</Button>
							<Button onClick={() => goTo(next.path)}>{tr('Play now', 'Reproduzir agora')}</Button>
						</div>
					)}

					<div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-2.5">
						<div className="min-w-0 space-y-0.5">
							{position >= 0 && <p className="text-[13px] text-muted-foreground tabular-nums">{tr(`Lesson ${position + 1} of ${sequence.length}`, `Aula ${position + 1} de ${sequence.length}`)}</p>}
							<h1 className="text-xl leading-tight font-semibold tracking-tight text-balance [overflow-wrap:anywhere] sm:text-[26px]">{displayTitle}</h1>
						</div>
						{studySyncSupported && mediaId && (
							<Button variant="outline" aria-pressed={isDone} disabled={setCompleted.isPending} title={tr('Mark or unmark this lesson completed regardless of playback position', 'Marcar ou desmarcar esta aula como concluída, independente da posição do vídeo')} onClick={() => setCompleted.mutate({ mediaId, completed: !isDone }, { onSuccess: (updated) => client.setQueryData(['progress', mediaId], updated) })}>
								{isDone ? <CircleCheck className="text-primary" /> : <Circle />}{isDone ? tr('Completed', 'Concluída') : tr('Mark as completed', 'Marcar como concluída')}
							</Button>
						)}
					</div>

					<div className="flex flex-wrap items-center gap-x-4 gap-y-1.5 border-t border-[var(--study-rule)] pt-2.5 text-sm lg:pt-3">
						{!pdf && playableMedia && resumeAt !== null && time < resumeAt && (
							<Button variant="secondary" onClick={() => player.current?.seek(resumeAt)}>{tr(`Resume from ${formatDuration(resumeAt)}`, `Retomar de ${formatDuration(resumeAt)}`)}</Button>
						)}
						{next && <Button variant="ghost" onClick={() => goTo(next.path)} className="min-w-0 max-w-full px-2.5"><span className="text-muted-foreground">{tr('Up next', 'A seguir')}:</span><span className="truncate font-medium">{next.title}</span><ChevronRight className="size-4 text-muted-foreground" aria-hidden="true" /></Button>}
						{materialCount > 0 && <Button variant="ghost" onClick={() => { setContentOpen(true); setTab('contents'); }}>{tr(`Materials (${materialCount})`, `Materiais (${materialCount})`)}</Button>}
						<label className="ml-auto inline-flex min-h-11 cursor-pointer items-center gap-2.5 px-2 font-medium text-foreground/85 hover:text-foreground has-[:focus-visible]:ring-3 has-[:focus-visible]:ring-ring/50" title={tr('When playback ends, mark completed and advance after 5 s', 'Ao chegar ao fim, marca como concluída e avança em 5 s')}>
							<input type="checkbox" className="peer sr-only" checked={preferences.autoNext} onChange={(event) => savePreferences({ ...preferences, autoNext: event.target.checked })} />
							<span aria-hidden="true" className="relative inline-flex h-5 w-9 shrink-0 items-center rounded-full bg-border-strong/45 transition-colors duration-150 ease-[cubic-bezier(0.23,1,0.32,1)] peer-checked:bg-primary">
								<span className={cn('size-3.5 translate-x-0.5 rounded-full bg-card shadow-xs transition-transform duration-150 ease-[cubic-bezier(0.23,1,0.32,1)]', preferences.autoNext && 'translate-x-[1.125rem]')} />
							</span>
							{tr('Autoplay next', 'Reproduzir a próxima')}
						</label>
					</div>
				</div>

				{(
					// Desktop: a full-height rail with tabs. Phones: rounded card above bottom navigation bar.
					<aside className={cn('lesson-rail min-w-0 rounded-xl border border-[var(--study-rule)] bg-card lg:rounded-none lg:border-y-0 lg:border-r-0 lg:border-l [:root[data-sidebar=left]_&]:lg:order-first [:root[data-sidebar=left]_&]:lg:border-x-0 [:root[data-sidebar=left]_&]:lg:border-r', !contentOpen && 'hidden max-lg:block')}>
						<Tabs value={activeTab} onValueChange={(value) => setTab(String(value))} className="gap-0">
							<TabsList variant="line" aria-label={tr('Lesson tools', 'Ferramentas da aula')} className="w-full justify-start gap-1 border-b px-2 group-data-horizontal/tabs:h-auto [&>[role=tab]]:min-h-11 [&>[role=tab]]:px-2 sm:px-3 sm:[&>[role=tab]]:px-3">
								{hasNotes && <TabsTrigger value="notes">{tr('Notes', 'Notas')}{noteCount > 0 && ` (${noteCount})`}</TabsTrigger>}
								{hasCards && <TabsTrigger value="cards">{tr('Cards', 'Cartões')}{cardCount > 0 && ` (${cardCount})`}</TabsTrigger>}
								<TabsTrigger value="contents">{tr('Contents', 'Conteúdo')}</TabsTrigger>
							</TabsList>
							{hasNotes && mediaId && (
								<TabsContent value="notes" keepMounted className="space-y-6 p-4">
									<StudyNotes
										mediaId={mediaId}
										currentTime={time}
										onSeek={!pdf && !text ? (seconds) => player.current?.seek(seconds) : undefined}
										addRequest={noteRequest}
										onMakeCard={hasCards ? (note) => {
											player.current?.pause();
											setTab('cards');
											setCardSeed({ key: (cardSeed?.key ?? 0) + 1, back: note.content, position: !pdf && !text ? note.positionSeconds : null });
										} : undefined}
										allNotesTo={`/notes?course=${encodeURIComponent(courseId)}`}
									/>
									{!pdf && !text && (
										<section aria-label={tr('Timestamps', 'Marcadores')} className="space-y-3 border-t border-[var(--study-rule)] pt-4">
											<h2 className="text-sm font-semibold">{tr('Timestamps', 'Marcadores')}</h2>
											<Bookmarks mediaId={mediaId} currentTime={time} onSeek={(seconds) => player.current?.seek(seconds)} />
										</section>
									)}
								</TabsContent>
							)}
							{hasCards && mediaId && (
								<TabsContent value="cards" keepMounted className="p-4">
									<Flashcards mediaId={mediaId} currentTime={time} cardSeed={cardSeed} />
								</TabsContent>
							)}
							<TabsContent value="contents" className="space-y-4 p-4">
								<div>
									<h2 className="text-sm font-semibold">{tr('Course content', 'Conteúdo do curso')}</h2>
									<p className="text-sm text-muted-foreground">{tr(`${overall.done} of ${overall.total} lessons completed`, `${overall.done} de ${overall.total} aulas concluídas`)}</p>
								</div>
								<div role="search" className="relative">
									<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
									<Input type="search" value={find} onChange={(event) => setFind(event.target.value)} placeholder={tr('Find a lesson', 'Buscar aula')} aria-label={tr('Find a lesson', 'Buscar aula')} className="pl-9" />
								</div>
								{found.length === 0 ? <p role="status" className="py-3 text-sm text-muted-foreground">{tr('No lesson matches.', 'Nenhuma aula corresponde.')}</p> : <Curriculum compact nodes={found} courseId={courseId} completed={completed} favorites={favoriteMedia} activePath={lessonPath} signal={needle ? { open: true, n: needle.length } : undefined} />}
								{materialCount > 0 && (
									<section aria-label={tr('Materials', 'Materiais')} className="space-y-3 border-t border-[var(--study-rule)] pt-4">
										<h2 className="text-sm font-semibold">{tr('Materials', 'Materiais')}</h2>
										<Materials files={[...lesson.subtitles, ...lesson.materials]} courseId={courseId} />
									</section>
								)}
							</TabsContent>
						</Tabs>
					</aside>
				)}
			</div>
		</div>
	);
}

/** The modules and lessons whose titles contain the text; a module is kept when one of its lessons matches. */
function filterLessons(nodes: CatalogNode[], needle: string): CatalogNode[] {
	return nodes.flatMap((node) => {
		if (node.role === 'lesson') return normalizeForSearch(node.title).includes(needle) ? [node] : [];
		const children = filterLessons(node.children, needle);
		return children.length > 0 ? [{ ...node, children }] : [];
	});
}

/** A yes/no choice kept in this browser; unreadable storage just means the default. */
function useStoredFlag(key: string, fallback: boolean) {
	const [value, setValue] = useState(() => {
		try {
			const saved = safeStorage()?.getItem(key);
			return saved === null || saved === undefined ? fallback : saved === '1';
		} catch {
			return fallback;
		}
	});
	const set = (next: boolean) => {
		setValue(next);
		try {
			safeStorage()?.setItem(key, next ? '1' : '0');
		} catch {
			// The choice just is not kept.
		}
	};
	return [value, set] as const;
}

/** The reader for a PDF lesson, with its type said out loud and the read-only promise. */
function PdfLesson({ file, courseId, ready, initialPage, onPage, preferences, onPreferencesChange }: { file: FileEntry; courseId: string; ready: boolean; initialPage: number | null; onPage: (page: number, pages: number) => void; preferences: PlayerPreferences; onPreferencesChange: (next: PlayerPreferences) => void }) {
	return (
		<div className="space-y-3">
			<p role="note" className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted-foreground">
				<span className="inline-flex items-center gap-1.5 rounded-full bg-brand-soft px-2.5 py-0.5 font-semibold text-brand-ink"><FileText className="size-3.5" aria-hidden="true" />{tr('PDF document', 'Documento PDF')}</span>
				<span>{formatFileSize(file.size)}</span>
				<span>{tr('Read it here; the original file is never changed.', 'Leia aqui; o arquivo original nunca é alterado.')}</span>
			</p>
			{ready ? <PdfReader key={file.mediaId ?? file.path} file={file} courseId={courseId} initialPage={initialPage} onPage={onPage} preferences={preferences} onPreferencesChange={onPreferencesChange} /> : <p role="status" className="text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>}
		</div>
	);
}

/** A lesson that is a text file: shown as plain text, never as HTML, and the original is never changed. */
function TextLesson({ file, courseId }: { file: FileEntry; courseId: string }) {
	const url = transport.mediaUrl(file, courseId);
	const content = useQuery({ queryKey: ['lesson-text', courseId, file.mediaId ?? file.path], enabled: Boolean(url), staleTime: Infinity, queryFn: () => loadText(file, courseId) });
	return (
		<div className="space-y-3">
			<p role="note" className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted-foreground">
				<span className="inline-flex items-center gap-1.5 rounded-full bg-brand-soft px-2.5 py-0.5 font-semibold text-brand-ink"><FileText className="size-3.5" aria-hidden="true" />{tr('Text', 'Texto')}</span>
				<span>{formatFileSize(file.size)}</span>
				<span>{tr('Read it here; the original file is never changed.', 'Leia aqui; o arquivo original nunca é alterado.')}</span>
			</p>
			{content.isPending ? <p role="status" className="text-muted-foreground">{tr('Loading…', 'Carregando…')}</p> : content.isError ? (
				<p role="alert" className="rounded-lg border bg-card p-4">{tr('Could not load this text.', 'Não foi possível carregar este texto.')}</p>
			) : (
				<article aria-label={file.name} className="max-h-[70vh] overflow-auto rounded-lg border bg-card p-5">
					<pre className="font-sans text-[15px] leading-relaxed break-words whitespace-pre-wrap">{content.data.text}</pre>
					{content.data.truncated && <p className="mt-4 text-sm text-muted-foreground">{tr('This file is long; only the beginning is shown. Open it from Materials to read all of it.', 'Este arquivo é longo; só o começo é mostrado. Abra-o em Materiais para ler tudo.')}</p>}
				</article>
			)}
		</div>
	);
}

function BackLink({ course, courseId }: { course: CatalogNode; courseId: string }) {
	return (
		<Link to={path(href('course', courseId))} className="inline-flex min-h-11 min-w-0 items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
			<ChevronLeft className="size-4 shrink-0" aria-hidden="true" /><span className="truncate">{course.title}</span>
		</Link>
	);
}
