import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ChevronLeft, ChevronRight, CircleCheck, Circle, PanelRightClose, PanelRightOpen } from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { Curriculum } from '@/components/curriculum';
import { Materials } from '@/components/materials';
import { Flashcards } from '@/components/player/flashcards';
import { MediaPlayer, type PlayerHandle } from '@/components/player/media-player';
import { StudyNotes } from '@/components/player/study-notes';
import { Button } from '@/components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { usePlayerPreferences } from '@/hooks/use-player-preferences';
import { useLocale } from '@/hooks/use-locale';
import { courseProgress } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { neighboringLessons } from '@/lib/navigation';
import { formatDuration, selectPlayableMedia, shouldAutoAdvance, studySyncSupported } from '@/lib/player';
import { lessonPreferenceKey } from '@/lib/player-preferences';
import { findLesson, findLessonLocation, href } from '@/lib/router';
import { createPositionSaver } from '@/lib/study';
import { transport, type CatalogNode, type StudyProgress } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { keys, useCatalog, useFavorites, useRecent, useSession, useSetLessonCompleted } from '@/app/data';

const path = (hash: string) => hash.replace(/^#/, '');
const AUTO_NEXT_SECONDS = 5;

/**
 * Lesson player page (docs/design/redesign.md, "Player"): media first, the curriculum in a
 * collapsible sidebar, completion and lesson navigation, then Overview, Notes, Flashcards
 * and Materials.
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
	const [tab, setTab] = useState('overview');
	const [noteRequest, setNoteRequest] = useState(0);
	const [sidebar, setSidebar] = useState(true);
	const [countdown, setCountdown] = useState<number | null>(null);

	const lesson = findLesson(catalog.data ?? null, courseId, lessonPath);
	const location = useMemo(() => {
		const media = lesson?.media.find((file) => file.mediaId);
		return media?.mediaId ? findLessonLocation(catalog.data ?? null, media.mediaId) : null;
	}, [catalog.data, lesson]);
	const { previous, next } = neighboringLessons([course], lessonPath);
	const mediaId = lesson ? selectPlayableMedia(lesson.media, preferences.lessons[lessonPreferenceKey(courseId, lessonPath)]?.mediaId)?.mediaId ?? null : null;
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

	useEffect(() => {
		document.title = lesson ? `${lesson.title} · ${course.title} · LearnVault` : 'LearnVault';
	}, [lesson, course.title]);

	if (!lesson) {
		return (
			<div className="space-y-4">
				<BackLink course={course} courseId={courseId} />
				<h1 className="text-2xl font-semibold">{tr('Lesson not found', 'Aula não encontrada')}</h1>
				<p className="text-muted-foreground">{tr('It may have been renamed or removed. Pick a lesson from the course.', 'Ela pode ter sido renomeada ou removida. Escolha uma aula do curso.')}</p>
			</div>
		);
	}

	return (
		<div className={cn('grid gap-6', sidebar && 'lg:grid-cols-[minmax(0,1fr)_22rem]')}>
			<div className="min-w-0 space-y-4">
				<div className="flex items-center justify-between gap-2">
					<BackLink course={course} courseId={courseId} />
					<Button variant="ghost" size="sm" className="hidden lg:inline-flex" aria-expanded={sidebar} aria-controls="lesson-curriculum" onClick={() => setSidebar((open) => !open)}>
						{sidebar ? <PanelRightClose /> : <PanelRightOpen />}{sidebar ? tr('Hide course content', 'Ocultar conteúdo do curso') : tr('Show course content', 'Mostrar conteúdo do curso')}
					</Button>
				</div>

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
					onOpenNotes={() => { setTab('notes'); setNoteRequest((value) => value + 1); }}
					handle={(value) => { player.current = value; }}
				/>

				{countdown !== null && next && (
					<div role="status" aria-live="polite" className="flex flex-wrap items-center gap-3 rounded-lg border bg-card p-4">
						<p className="min-w-0 flex-1">{tr(`Next lesson in ${countdown} s: ${next.title}`, `Próxima aula em ${countdown} s: ${next.title}`)}</p>
						<Button variant="outline" onClick={() => setCountdown(null)}>{tr('Cancel', 'Cancelar')}</Button>
						<Button onClick={() => goTo(next.path)}>{tr('Play now', 'Reproduzir agora')}</Button>
					</div>
				)}

				<div className="flex flex-wrap items-center gap-2">
					<h1 className="mr-auto min-w-0 text-2xl font-semibold tracking-tight text-balance">{lesson.title}</h1>
					{studySyncSupported && mediaId && (
						<Button variant={isDone ? 'secondary' : 'default'} aria-pressed={isDone} disabled={setCompleted.isPending} onClick={() => setCompleted.mutate({ mediaId, completed: !isDone }, { onSuccess: (updated) => client.setQueryData(['progress', mediaId], updated) })}>
							{isDone ? <CircleCheck /> : <Circle />}{isDone ? tr('Completed', 'Concluída') : tr('Mark as completed', 'Marcar como concluída')}
						</Button>
					)}
				</div>
				<div className="flex flex-wrap items-center gap-2">
					<Button variant="outline" disabled={!previous} onClick={() => previous && goTo(previous.path)}><ChevronLeft />{tr('Previous', 'Anterior')}</Button>
					<Button variant="outline" disabled={!next} onClick={() => next && goTo(next.path)}>{tr('Next', 'Próxima')}<ChevronRight /></Button>
					{savedPosition !== null && time < savedPosition && (
						<Button variant="secondary" onClick={() => player.current?.seek(savedPosition)}>{tr(`Resume from ${formatDuration(savedPosition)}`, `Retomar de ${formatDuration(savedPosition)}`)}</Button>
					)}
					<label className="ml-auto flex min-h-11 items-center gap-2 text-sm">
						<input type="checkbox" className="size-5 accent-[var(--primary)]" checked={preferences.autoNext} onChange={(event) => savePreferences({ ...preferences, autoNext: event.target.checked })} />
						{tr('Play the next lesson automatically', 'Reproduzir a próxima aula automaticamente')}
					</label>
				</div>

				<Tabs value={tab} onValueChange={(value) => setTab(String(value))}>
					<TabsList aria-label={tr('Lesson tools', 'Ferramentas da aula')}>
						<TabsTrigger value="overview">{tr('Overview', 'Visão geral')}</TabsTrigger>
						{studySyncSupported && mediaId && <TabsTrigger value="notes">{tr('Notes', 'Notas')}</TabsTrigger>}
						{studySyncSupported && mediaId && <TabsTrigger value="flashcards">{tr('Flashcards', 'Flashcards')}</TabsTrigger>}
						<TabsTrigger value="materials">{tr('Materials', 'Materiais')}</TabsTrigger>
					</TabsList>
					<TabsContent value="overview" className="space-y-2 pt-4">
						<p>{location?.moduleTitle ? `${course.title} · ${location.moduleTitle}` : course.title}</p>
						<p className="text-muted-foreground">{tr(`${overall.done} of ${overall.total} lessons of this course completed.`, `${overall.done} de ${overall.total} aulas deste curso concluídas.`)}</p>
					</TabsContent>
					{studySyncSupported && mediaId && (
						<TabsContent value="notes" keepMounted className="pt-4">
							<StudyNotes mediaId={mediaId} currentTime={time} onSeek={(seconds) => player.current?.seek(seconds)} addRequest={noteRequest} />
						</TabsContent>
					)}
					{studySyncSupported && mediaId && (
						<TabsContent value="flashcards" className="pt-4">
							<Flashcards mediaId={mediaId} />
						</TabsContent>
					)}
					<TabsContent value="materials" className="pt-4">
						<Materials files={[...lesson.subtitles, ...lesson.materials]} courseId={courseId} />
					</TabsContent>
				</Tabs>
			</div>

			{sidebar && (
				<aside id="lesson-curriculum" aria-labelledby="curriculum-heading" className="min-w-0 space-y-3 lg:sticky lg:top-20 lg:max-h-[calc(100dvh-6rem)] lg:overflow-y-auto">
					<h2 id="curriculum-heading" className="font-semibold">{tr('Course content', 'Conteúdo do curso')}</h2>
					<p className="text-sm text-muted-foreground">{tr(`${overall.done} of ${overall.total} completed`, `${overall.done} de ${overall.total} concluídas`)}</p>
					<Curriculum nodes={course.children} courseId={courseId} completed={completed} favorites={favoriteMedia} activePath={lessonPath} />
				</aside>
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
