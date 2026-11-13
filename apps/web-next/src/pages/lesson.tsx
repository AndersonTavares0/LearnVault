import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, CircleCheck, Circle, FileText, PanelRightClose, PanelRightOpen, PenLine, SquarePlus } from 'lucide-react';
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { usePlayerPreferences } from '@/hooks/use-player-preferences';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { courseProgress } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { lessonSequence, neighboringLessons } from '@/lib/navigation';
import { findLessonPdf, formatFileSize } from '@/lib/pdf';
import { formatDuration, selectPlayableMedia, shouldAutoAdvance, studySyncSupported } from '@/lib/player';
import { lessonPreferenceKey } from '@/lib/player-preferences';
import { pomodoro, resetTimer, skipTimer, startTimer } from '@/lib/pomodoro-store';
import { findLesson, findLessonLocation, href } from '@/lib/router';
import { createPositionSaver } from '@/lib/study';
import { transport, type Catalog, type CatalogNode, type FileEntry, type StudyProgress } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { keys, useCatalog, useFavorites, useRecent, useSession, useSetLessonCompleted, useBookmarks } from '@/app/data';

const path = (hash: string) => hash.replace(/^#/, '');
const AUTO_NEXT_SECONDS = 5;

/**
 * Lesson page, laid out as a study workspace: the media on the left; on the right the focus
 * timer and a panel with Notes, Cards, Lessons and Materials, so what you write and the
 * course map stay beside the video. When a focus block ends the lesson pauses and a break
 * screen covers the video (docs/design/redesign.md, "Player").
 */
export function LessonPage({ course, courseId, lessonPath }: { course: CatalogNode; courseId: string; lessonPath: string }) {
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
	const [tab, setTab] = useState('lessons');
	// Cinema gives the picture the whole width; the course content can also be hidden. Both are remembered.
	const [cinema, setCinema] = useStoredFlag('learnvault-cinema', false);
	const [contentOpen, setContentOpen] = useStoredFlag('learnvault-lesson-content', true);
	const wide = cinema || !contentOpen;
	const [noteRequest, setNoteRequest] = useState(0);
	const [cardRequest, setCardRequest] = useState(0);
	const [breakPhase, setBreakPhase] = useState<'break' | 'over' | null>(null);
	const timer = useStore(pomodoro);
	const [seenRound, setSeenRound] = useState(timer.lastCompleted?.at ?? 0);
	const [countdown, setCountdown] = useState<number | null>(null);

	const lesson = findLesson(catalog.data ?? null, courseId, lessonPath);
	// A lesson with nothing to play and a PDF is a PDF lesson: the PDF is read here and drives progress.
	const pdf = lesson ? findLessonPdf(lesson) : null;
	const location = useMemo(() => {
		const media = lesson?.media.find((file) => file.mediaId) ?? pdf;
		return media?.mediaId ? findLessonLocation(catalog.data ?? null, media.mediaId) : null;
	}, [catalog.data, lesson, pdf]);
	const { previous, next } = neighboringLessons([course], lessonPath);
	const mediaId = pdf?.mediaId ?? (lesson ? selectPlayableMedia(lesson.media, preferences.lessons[lessonPreferenceKey(courseId, lessonPath)]?.mediaId)?.mediaId ?? null : null);
	const completed = useMemo(() => new Set((recent.data ?? []).filter((item) => item.completed).map((item) => item.mediaId)), [recent.data]);
	const favoriteMedia = useMemo(() => new Set(favorites.data?.mediaIds ?? []), [favorites.data]);
	const progress = useQuery({ queryKey: ['progress', mediaId], enabled: Boolean(mediaId) && studySyncSupported, queryFn: () => transport.studyProgress(mediaId!) });
	const isDone = Boolean(mediaId && (progress.data?.completed ?? completed.has(mediaId)));
	const overall = courseProgress(course, completed);
	// `?t=` (resume links, note timestamps) starts there; otherwise the saved position is
	// applied automatically. A completed lesson starts from the beginning unless `?t=` says otherwise.
	const queryTime = Number(params.get('t'));
	const requestedPosition = Number.isFinite(queryTime) && queryTime > 0 ? queryTime : null;
	const savedPosition = !requestedPosition && progress.data?.recorded && !progress.data.completed && progress.data.positionSeconds > 5 ? progress.data.positionSeconds : null;
	const resumeAt = requestedPosition ?? savedPosition;
	const pdfResumeAt = requestedPosition ?? (progress.data?.recorded && !progress.data.completed && progress.data.positionSeconds > 1 ? progress.data.positionSeconds : null);
	const bookmarks = useBookmarks(mediaId);

	// One saver per lesson keeps writes ordered; saved positions also refresh "Continue studying".
	const saver = useMemo(() => createPositionSaver((id, seconds) => transport.recordPosition(id, seconds)), []);
	const recordPosition = async (id: string, seconds: number) => {
		if (!studySyncSupported) return;
		const result = await saver.request(id, seconds);
		if (result.kind === 'saved') client.setQueryData<StudyProgress[]>(keys.recent, (items = []) => [result.progress, ...items.filter((item) => item.mediaId !== id)]);
	};

	const noteCount = useLessonNotes(mediaId).data?.length ?? 0;
	const goTo = (target: string) => navigate(path(href('course', courseId, target)));
	const finished = (id: string) => {
		if (studySyncSupported) setCompleted.mutate({ mediaId: id, completed: true }, { onSuccess: (updated) => client.setQueryData(['progress', id], updated) });
		if (shouldAutoAdvance(preferences.autoNext, Boolean(next))) setCountdown(AUTO_NEXT_SECONDS);
	};

	// Reset per-lesson state when the lesson changes.
	const [shownLesson, setShownLesson] = useState(lessonPath);
	if (shownLesson !== lessonPath) {
		setShownLesson(lessonPath);
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
		document.title = lesson ? `${lesson.title} · ${course.title} · LearnVault` : 'LearnVault';
	}, [lesson, course.title]);

	const sequence = useMemo(() => lessonSequence([course]), [course]);
	// One pass over the course, not one catalog walk per segment on every render.
	const donePaths = useMemo(() => new Set(sequence.filter((item) => {
		const other = findLesson(catalog.data ?? null, courseId, item.path);
		const files = other ? [findLessonPdf(other), ...other.media] : [];
		return files.some((file) => file?.mediaId && completed.has(file.mediaId));
	}).map((item) => item.path)), [sequence, catalog.data, courseId, completed]);
	if (!lesson) {
		return (
			<div className="space-y-4">
				<BackLink course={course} courseId={courseId} />
				<h1 className="text-2xl font-semibold">{tr('Lesson not found', 'Aula não encontrada')}</h1>
				<p className="text-muted-foreground">{tr('It may have been renamed or removed. Pick a lesson from the course.', 'Ela pode ter sido renomeada ou removida. Escolha uma aula do curso.')}</p>
			</div>
		);
	}

	const position = sequence.findIndex((item) => item.path === lessonPath);
	const hasCards = studySyncSupported && Boolean(mediaId);
	// Notes are tied to a moment in time, so a PDF lesson has cards but no timed notes.
	const hasNotes = hasCards && !pdf;
	const activeTab = (tab === 'notes' && !hasNotes) || (tab === 'bookmarks' && !hasNotes) || (tab === 'cards' && !hasCards) ? 'lessons' : tab;
	const openNotes = () => { setTab('notes'); setNoteRequest((value) => value + 1); };
	const closeBreak = () => setBreakPhase(null);

	return (
		<div>
			<header className="mb-6 space-y-4">
				<div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-3">
					<div className="min-w-0 space-y-1">
						<Link to={path(href('course', courseId))} className="inline-flex min-h-11 items-center gap-1 text-sm font-medium text-muted-foreground hover:text-foreground">
							<ChevronLeft className="size-4 shrink-0" aria-hidden="true" /><span className="max-w-[28rem] truncate">{location?.moduleTitle ? `${course.title} · ${location.moduleTitle}` : course.title}</span>
						</Link>
						<h1 className="font-heading text-3xl leading-tight font-bold text-balance sm:text-4xl">{lesson.title}</h1>
					</div>
					<div className="flex flex-wrap items-center gap-2">
						{position >= 0 && <span className="mr-2 text-sm text-muted-foreground tabular-nums">{tr(`Lesson ${position + 1} of ${sequence.length}`, `Aula ${position + 1} de ${sequence.length}`)}</span>}
						<Button variant="outline" className="h-11 rounded-full px-5 disabled:opacity-60" disabled={!previous} onClick={() => previous && goTo(previous.path)}><ChevronLeft />{tr('Previous', 'Anterior')}</Button>
						<Button className="h-11 rounded-full px-5" disabled={!next} onClick={() => next && goTo(next.path)}>{tr('Next', 'Próxima')}<ChevronRight /></Button>
						<Button variant="outline" size="icon" className="hidden size-11 rounded-full lg:inline-flex" aria-pressed={contentOpen} onClick={() => setContentOpen(!contentOpen)} aria-label={contentOpen ? tr('Hide course content', 'Ocultar conteúdo do curso') : tr('Show course content', 'Mostrar conteúdo do curso')}>{contentOpen ? <PanelRightClose /> : <PanelRightOpen />}</Button>
					</div>
				</div>
				<LessonRail sequence={sequence} current={position} isDone={(path) => donePaths.has(path)} />
			</header>

			<div className={cn('grid gap-6', !wide && 'lg:grid-cols-[minmax(0,1fr)_22rem] [:root[data-sidebar=left]_&]:lg:grid-cols-[22rem_minmax(0,1fr)]')}>
				<div className="min-w-0 space-y-4">
					<div className={cn('relative', cinema && 'rounded-[2rem] bg-black p-3 sm:p-6')}>
						{/* While the break screen is up, the player under it takes no focus or keys. */}
						<div inert={Boolean(breakPhase)}>
						{pdf ? (
							<PdfLesson file={pdf} courseId={courseId} ready={!studySyncSupported || !progress.isPending} initialPage={pdfResumeAt} onPage={(page) => void recordPosition(pdf.mediaId!, page)} />
						) : (
						<MediaPlayer
							key={`${courseId}/${lessonPath}`}
							lesson={lesson}
							courseId={courseId}
							initialPosition={resumeAt}
							preferences={preferences}
							onPreferencesChange={savePreferences}
							onPosition={recordPosition}
							onEnded={finished}
							onTime={setTime}
							onOpenNotes={openNotes}
							markers={bookmarks.data?.map((bookmark) => ({ id: bookmark.bookmarkId, positionSeconds: bookmark.positionSeconds, label: bookmark.label }))}
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

					{countdown !== null && next && (
						<div role="status" aria-live="polite" className="flex flex-wrap items-center gap-3 rounded-lg border bg-card p-4">
							<p className="min-w-0 flex-1">{tr(`Next lesson in ${countdown} s: ${next.title}`, `Próxima aula em ${countdown} s: ${next.title}`)}</p>
							<Button variant="outline" onClick={() => setCountdown(null)}>{tr('Cancel', 'Cancelar')}</Button>
							<Button onClick={() => goTo(next.path)}>{tr('Play now', 'Reproduzir agora')}</Button>
						</div>
					)}

					<Tabs value={activeTab} onValueChange={(value) => setTab(String(value))}>
						<TabsList variant="line" aria-label={tr('Lesson tools', 'Ferramentas da aula')} className="w-full justify-start border-b">
							<TabsTrigger value="lessons">{tr('Overview', 'Visão geral')}</TabsTrigger>
							{hasNotes && <TabsTrigger value="notes">{tr('Notes', 'Notas')}</TabsTrigger>}
							{hasNotes && <TabsTrigger value="bookmarks">{tr('Timestamps', 'Marcadores')}</TabsTrigger>}
							{hasCards && <TabsTrigger value="cards">{tr('Cards', 'Cartões')}</TabsTrigger>}
							<TabsTrigger value="materials">{tr('Materials', 'Materiais')}</TabsTrigger>
						</TabsList>
						<TabsContent value="lessons" className="space-y-4 pt-5">
							<div className="flex flex-wrap items-center gap-3">
								{hasCards && hasNotes && <Button variant="outline" onClick={() => { player.current?.pause(); openNotes(); }} aria-keyshortcuts="n"><PenLine />{tr('Note', 'Nota')}<kbd aria-hidden="true" className="ml-1 rounded border px-1.5 text-xs text-muted-foreground">N</kbd></Button>}
								{hasCards && <Button variant="outline" onClick={() => { player.current?.pause(); setTab('cards'); setCardRequest((value) => value + 1); }}><SquarePlus />{tr('Card', 'Cartão')}</Button>}
								{studySyncSupported && mediaId && (
									<Button variant={isDone ? 'secondary' : 'default'} aria-pressed={isDone} disabled={setCompleted.isPending} onClick={() => setCompleted.mutate({ mediaId, completed: !isDone }, { onSuccess: (updated) => client.setQueryData(['progress', mediaId], updated) })}>
										{isDone ? <CircleCheck /> : <Circle />}{isDone ? tr('Completed', 'Concluída') : tr('Mark as completed', 'Marcar como concluída')}
									</Button>
								)}
							</div>
							<div className="flex flex-wrap items-center gap-2">
								{!pdf && resumeAt !== null && time < resumeAt && (
									<Button variant="secondary" onClick={() => player.current?.seek(resumeAt)}>{tr(`Resume from ${formatDuration(resumeAt)}`, `Retomar de ${formatDuration(resumeAt)}`)}</Button>
								)}
								<label className="flex min-h-11 items-center gap-3 rounded-xl border bg-card px-3 text-sm hover:bg-muted">
									<input type="checkbox" className="size-5 accent-[var(--brand)]" checked={preferences.autoNext} onChange={(event) => savePreferences({ ...preferences, autoNext: event.target.checked })} />
									{tr('Play the next lesson automatically', 'Reproduzir a próxima aula automaticamente')}
								</label>
							</div>
							<RecentlyWatched items={recent.data ?? []} catalog={catalog.data ?? null} currentMediaId={mediaId} />
						</TabsContent>
						{hasNotes && mediaId && (
							<TabsContent value="notes" keepMounted className="pt-5">
								<StudyNotes mediaId={mediaId} currentTime={time} onSeek={(seconds) => player.current?.seek(seconds)} addRequest={noteRequest} title={lesson.title} trail={location?.moduleTitle ? `${course.title} · ${location.moduleTitle}` : course.title} allNotesTo={`/notes?course=${encodeURIComponent(courseId)}`} />
							</TabsContent>
						)}
						{hasNotes && mediaId && (
							<TabsContent value="bookmarks" keepMounted className="pt-5">
								<Bookmarks mediaId={mediaId} currentTime={time} onSeek={(seconds) => player.current?.seek(seconds)} />
							</TabsContent>
						)}
						{hasCards && mediaId && (
							<TabsContent value="cards" keepMounted className="pt-5">
								<Flashcards mediaId={mediaId} currentTime={time} addRequest={cardRequest} />
							</TabsContent>
						)}
						<TabsContent value="materials" className="pt-5">
							<Materials files={[...lesson.subtitles, ...lesson.materials]} courseId={courseId} />
						</TabsContent>
					</Tabs>
				</div>

				{contentOpen && (
					<aside aria-label={tr('Course content', 'Conteúdo do curso')} className={cn('min-w-0 space-y-3 rounded-[2rem] border bg-card p-4 [:root[data-sidebar=left]_&]:lg:order-first', !wide && 'lg:sticky lg:top-6 lg:max-h-[calc(100dvh-3rem)] lg:self-start lg:overflow-y-auto')}>
						<div className="flex items-start justify-between gap-2 px-1">
							<div>
								<h2 className="font-heading text-lg font-bold">{tr('Course content', 'Conteúdo do curso')}</h2>
								<p className="text-sm text-muted-foreground">{tr(`${overall.done} of ${overall.total} lessons completed`, `${overall.done} de ${overall.total} aulas concluídas`)}</p>
							</div>
							<Button variant="ghost" size="icon" className="hidden size-11 rounded-full lg:inline-flex" onClick={() => setContentOpen(false)} aria-label={tr('Hide course content', 'Ocultar conteúdo do curso')}><PanelRightClose /></Button>
						</div>
						<Curriculum compact nodes={course.children} courseId={courseId} completed={completed} favorites={favoriteMedia} activePath={lessonPath} />
					</aside>
				)}
			</div>
		</div>
	);
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

/** One segment per lesson: done, the current one, or still to come. Decorative; the text names the position. */
function LessonRail({ sequence, current, isDone }: { sequence: { path: string }[]; current: number; isDone: (path: string) => boolean }) {
	if (sequence.length < 2 || sequence.length > 60) return null;
	return (
		<div aria-hidden="true" className="flex gap-1">
			{sequence.map((item, index) => <span key={item.path} className={cn('h-1.5 flex-1 rounded-full', isDone(item.path) ? 'bg-primary' : index === current ? 'bg-[var(--reward-fill)]' : 'bg-border')} />)}
		</div>
	);
}

/** The reader for a PDF lesson, with its type said out loud and the read-only promise. */
function PdfLesson({ file, courseId, ready, initialPage, onPage }: { file: FileEntry; courseId: string; ready: boolean; initialPage: number | null; onPage: (page: number, pages: number) => void }) {
	return (
		<div className="space-y-3">
			<p role="note" className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted-foreground">
				<span className="inline-flex items-center gap-1.5 rounded-full bg-brand-soft px-2.5 py-0.5 font-semibold"><FileText className="size-3.5" aria-hidden="true" />{tr('PDF document', 'Documento PDF')}</span>
				<span>{formatFileSize(file.size)}</span>
				<span>{tr('Read it here; the original file is never changed.', 'Leia aqui; o arquivo original nunca é alterado.')}</span>
			</p>
			{ready ? <PdfReader key={file.mediaId ?? file.path} file={file} courseId={courseId} initialPage={initialPage} onPage={onPage} /> : <p role="status" className="text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>}
		</div>
	);
}

/**
 * Recently watched lessons, other than the one open. It is a compact history, not a second
 * resume card: each row is a plain link that returns to that lesson's saved position.
 */
function RecentlyWatched({ items, catalog, currentMediaId }: { items: StudyProgress[]; catalog: Catalog | null; currentMediaId: string | null }) {
	const seen = new Set<string>();
	const rows = items.flatMap((item) => {
		if (item.mediaId === currentMediaId) return [];
		const found = findLessonLocation(catalog, item.mediaId);
		if (!found || found.lesson.media.length === 0) return [];
		const key = `${found.courseId}/${found.lesson.path}`;
		if (seen.has(key)) return [];
		seen.add(key);
		return [{ item, found, key }];
	}).slice(0, 5);
	if (rows.length === 0) return null;
	return (
		<section aria-labelledby="recently-watched-heading" className="border-t pt-4">
			<div className="flex items-center justify-between gap-3">
				<h2 id="recently-watched-heading" className="font-heading text-lg font-semibold">{tr('Recently watched', 'Assistidas recentemente')}</h2>
				<Link to="/history" className="text-sm font-medium text-primary underline-offset-4 hover:underline focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">{tr('View all', 'Ver tudo')}</Link>
			</div>
			<ul className="mt-3 divide-y">
				{rows.map(({ item, found, key }) => (
					<li key={key}>
						<Link to={path(href('course', found.courseId, found.lesson.path, item.positionSeconds))} className="flex min-h-12 items-center gap-3 py-2 hover:text-primary focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
							<span className="min-w-0 flex-1">
								<span className="block truncate font-medium">{found.lesson.title}</span>
								<span className="block truncate text-sm text-muted-foreground">{found.courseTitle}</span>
							</span>
							<span className="shrink-0 text-sm tabular-nums text-muted-foreground">
								{item.completed ? tr('Completed', 'Concluída') : item.positionSeconds > 0 ? formatDuration(item.positionSeconds) : tr('Not started', 'Não iniciada')}
							</span>
						</Link>
					</li>
				))}
			</ul>
		</section>
	);
}

function BackLink({ course, courseId }: { course: CatalogNode; courseId: string }) {
	return (
		<Link to={path(href('course', courseId))} className="inline-flex min-h-11 min-w-0 items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
			<ChevronLeft className="size-4 shrink-0" aria-hidden="true" /><span className="truncate">{course.title}</span>
		</Link>
	);
}
