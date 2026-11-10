import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, CircleCheck, Circle, FileText, PanelRightClose, PanelRightOpen, PenLine, SquarePlus } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { Curriculum } from '@/components/curriculum';
import { Materials } from '@/components/materials';
import { BreakOverlay } from '@/components/player/break-overlay';
import { FocusCard } from '@/components/player/focus-card';
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
import { transport, type CatalogNode, type FileEntry, type StudyProgress } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { keys, useCatalog, useFavorites, useRecent, useSession, useSetLessonCompleted } from '@/app/data';

const path = (hash: string) => hash.replace(/^#/, '');
const AUTO_NEXT_SECONDS = 5;
const THEATER_KEY = 'learnvault-theater';
const readTheater = () => { try { return localStorage.getItem(THEATER_KEY) === '1'; } catch { return false; } };
const saveTheater = (on: boolean) => { try { localStorage.setItem(THEATER_KEY, on ? '1' : '0'); } catch { /* storage can be blocked; the choice just does not persist */ } };

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
	const [tab, setTab] = useState(studySyncSupported ? 'notes' : 'lessons');
	const [noteRequest, setNoteRequest] = useState(0);
	const [cardRequest, setCardRequest] = useState(0);
	const [theater, setTheater] = useState(readTheater);
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
	// `?t=` (resume links, note timestamps) starts there; otherwise offer the saved position.
	const queryTime = Number(params.get('t'));
	const initialPosition = Number.isFinite(queryTime) && queryTime > 0 ? queryTime : null;
	const savedPosition = !initialPosition && progress.data?.recorded && !progress.data.completed && progress.data.positionSeconds > 5 ? progress.data.positionSeconds : null;

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
	const activeTab = (tab === 'notes' && !hasNotes) || (tab === 'cards' && !hasCards) ? 'lessons' : tab;
	const openNotes = () => { setTab('notes'); setNoteRequest((value) => value + 1); };
	const closeBreak = () => setBreakPhase(null);

	return (
		<div className={cn('grid gap-6', !theater && 'lg:grid-cols-[minmax(0,1fr)_24rem]')}>
			<div className="min-w-0 space-y-4">
				<div className="flex flex-wrap items-center justify-between gap-2">
					<BackLink course={course} courseId={courseId} />
					<div className="flex items-center gap-2">
						<Button variant="outline" className="hidden lg:inline-flex" aria-pressed={theater} onClick={() => { setTheater(!theater); saveTheater(!theater); }}>
							{theater ? <PanelRightOpen /> : <PanelRightClose />}{theater ? tr('Show side panel', 'Mostrar painel lateral') : tr('Theater mode', 'Modo teatro')}
						</Button>
						<Button variant="outline" disabled={!previous} onClick={() => previous && goTo(previous.path)}><ChevronLeft />{tr('Previous', 'Anterior')}</Button>
						{position >= 0 && <span className="text-sm text-muted-foreground tabular-nums">{tr(`Lesson ${position + 1} of ${sequence.length}`, `Aula ${position + 1} de ${sequence.length}`)}</span>}
						<Button variant="outline" disabled={!next} onClick={() => next && goTo(next.path)}>{tr('Next', 'Próxima')}<ChevronRight /></Button>
					</div>
				</div>
				<LessonRail sequence={sequence} current={position} isDone={(path) => donePaths.has(path)} />

				<div className="relative">
					{/* While the break screen is up, the player under it takes no focus or keys. */}
					<div inert={Boolean(breakPhase)}>
					{pdf ? (
						<PdfLesson file={pdf} courseId={courseId} ready={!studySyncSupported || !progress.isPending} initialPage={initialPosition ?? (progress.data?.recorded && !progress.data.completed && progress.data.positionSeconds > 1 ? progress.data.positionSeconds : null)} onPage={(page) => void recordPosition(pdf.mediaId!, page)} />
					) : (
					<MediaPlayer
						key={`${courseId}/${lessonPath}`}
						lesson={lesson}
						courseId={courseId}
						initialPosition={initialPosition}
						preferences={preferences}
						onPreferencesChange={savePreferences}
						onPosition={recordPosition}
						onEnded={finished}
						onTime={setTime}
						onOpenNotes={openNotes}
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

				{hasCards && (
					<div className="flex flex-wrap gap-2">
						{hasNotes && <Button variant="outline" onClick={() => { player.current?.pause(); openNotes(); }} aria-keyshortcuts="n"><PenLine />{tr('Pause and note', 'Pausar e anotar')}<kbd aria-hidden="true" className="ml-1 rounded border px-1.5 text-xs text-muted-foreground">N</kbd></Button>}
						<Button variant="outline" onClick={() => { player.current?.pause(); setTab('cards'); setCardRequest((value) => value + 1); }}><SquarePlus />{tr('Make card', 'Criar cartão')}</Button>
					</div>
				)}

				<div className="flex flex-wrap items-center gap-2">
					<div className="mr-auto min-w-0">
						<h1 className="text-2xl font-semibold tracking-tight text-balance">{lesson.title}</h1>
						<p className="text-sm text-muted-foreground">{location?.moduleTitle ? `${course.title} · ${location.moduleTitle}` : course.title}</p>
					</div>
					{studySyncSupported && mediaId && (
						<Button variant={isDone ? 'secondary' : 'default'} aria-pressed={isDone} disabled={setCompleted.isPending} onClick={() => setCompleted.mutate({ mediaId, completed: !isDone }, { onSuccess: (updated) => client.setQueryData(['progress', mediaId], updated) })}>
							{isDone ? <CircleCheck /> : <Circle />}{isDone ? tr('Completed', 'Concluída') : tr('Mark as completed', 'Marcar como concluída')}
						</Button>
					)}
				</div>
				<div className="flex flex-wrap items-center gap-2">
					{!pdf && savedPosition !== null && time < savedPosition && (
						<Button variant="secondary" onClick={() => player.current?.seek(savedPosition)}>{tr(`Resume from ${formatDuration(savedPosition)}`, `Retomar de ${formatDuration(savedPosition)}`)}</Button>
					)}
					<label className="flex min-h-11 items-center gap-2 text-sm">
						<input type="checkbox" className="size-5 accent-[var(--primary)]" checked={preferences.autoNext} onChange={(event) => savePreferences({ ...preferences, autoNext: event.target.checked })} />
						{tr('Play the next lesson automatically', 'Reproduzir a próxima aula automaticamente')}
					</label>
				</div>
			</div>

			<aside aria-label={tr('Study panel', 'Painel de estudo')} className={cn('min-w-0 space-y-4', !theater && 'lg:sticky lg:top-20 lg:max-h-[calc(100dvh-6rem)] lg:self-start lg:overflow-y-auto')}>
				<FocusCard />
				<Tabs value={activeTab} onValueChange={(value) => setTab(String(value))}>
					<TabsList aria-label={tr('Lesson tools', 'Ferramentas da aula')}>
						{hasNotes && <TabsTrigger value="notes">{tr('Notes', 'Notas')}</TabsTrigger>}
						{hasCards && <TabsTrigger value="cards">{tr('Cards', 'Cartões')}</TabsTrigger>}
						<TabsTrigger value="lessons">{tr('Lessons', 'Aulas')}</TabsTrigger>
						<TabsTrigger value="materials">{tr('Materials', 'Materiais')}</TabsTrigger>
					</TabsList>
					{hasNotes && mediaId && (
						<TabsContent value="notes" keepMounted className="pt-4">
							<StudyNotes mediaId={mediaId} currentTime={time} onSeek={(seconds) => player.current?.seek(seconds)} addRequest={noteRequest} />
						</TabsContent>
					)}
					{hasCards && mediaId && (
						<TabsContent value="cards" keepMounted className="pt-4">
							<Flashcards mediaId={mediaId} addRequest={cardRequest} />
						</TabsContent>
					)}
					<TabsContent value="lessons" className="space-y-3 pt-4">
						<p className="text-sm text-muted-foreground">{tr(`${overall.done} of ${overall.total} lessons of this course completed.`, `${overall.done} de ${overall.total} aulas deste curso concluídas.`)}</p>
						<Curriculum nodes={course.children} courseId={courseId} completed={completed} favorites={favoriteMedia} activePath={lessonPath} />
					</TabsContent>
					<TabsContent value="materials" className="pt-4">
						<Materials files={[...lesson.subtitles, ...lesson.materials]} courseId={courseId} />
					</TabsContent>
				</Tabs>
			</aside>
		</div>
	);
}

/** One segment per lesson: done, the current one, or still to come. Decorative; the text names the position. */
function LessonRail({ sequence, current, isDone }: { sequence: { path: string }[]; current: number; isDone: (path: string) => boolean }) {
	if (sequence.length < 2 || sequence.length > 60) return null;
	return (
		<div aria-hidden="true" className="flex gap-1">
			{sequence.map((item, index) => <span key={item.path} className={cn('h-1.5 flex-1 rounded-full', isDone(item.path) ? 'bg-primary' : index === current ? 'bg-amber-500' : 'bg-muted')} />)}
		</div>
	);
}

/** The reader for a PDF lesson, with its type said out loud and the read-only promise. */
function PdfLesson({ file, courseId, ready, initialPage, onPage }: { file: FileEntry; courseId: string; ready: boolean; initialPage: number | null; onPage: (page: number, pages: number) => void }) {
	return (
		<div className="space-y-3">
			<p className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted-foreground">
				<span className="inline-flex items-center gap-1.5 rounded-full bg-destructive/10 px-2.5 py-0.5 font-semibold text-destructive"><FileText className="size-3.5" aria-hidden="true" />{tr('PDF document', 'Documento PDF')}</span>
				<span>{formatFileSize(file.size)}</span>
			</p>
			<p role="note" className="rounded-xl border border-blue-200 bg-blue-50 p-3 text-sm text-blue-950 dark:border-blue-900 dark:bg-blue-950/40 dark:text-blue-100">
				{tr('This lesson is a PDF document. Read it here; the original file is never changed.', 'Esta aula é um documento PDF. Leia aqui; o arquivo original nunca é alterado.')}
			</p>
			{ready ? <PdfReader file={file} courseId={courseId} initialPage={initialPage} onPage={onPage} /> : <p role="status" className="text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>}
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
