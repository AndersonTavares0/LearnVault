import { Award, ChevronRight, Flame, FolderPlus, Play, RefreshCw, Search, Sparkles, Star, Target } from 'lucide-react';
import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
import { flushSync } from 'react-dom';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { Motto } from '@/components/brand';
import { CourseCover } from '@/components/course-cover';
import { CourseCard, routerPath } from '@/components/course-card';
import { lessonMediaId } from '@/components/curriculum';
import { LessonSegments } from '@/components/lesson-segments';
import { Button, buttonVariants } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { isLessonMissing } from '@/lib/catalog';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { appearanceStore } from '@/lib/appearance';
import { courseCategoriesStore } from '@/lib/course-categories';
import { buildGamification } from '@/lib/gamification';
import { tr } from '@/lib/i18n';
import { buildLibrary, type CourseEntry, type LibraryFilter } from '@/lib/library';
import { LIBRARY_VIEW_STORAGE_KEY, parseLibraryView, persistLibrarySort, persistLibraryView, readLibrarySort, type LibrarySort, type LibraryView } from '@/lib/library-view';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findLessonLocation, href, type LessonLocation } from '@/lib/router';
import { courseIdOf, isCredentialsChange, transportKind, type Catalog, type CatalogNode, type RewardSummary, type StudyProgress } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { refreshMessage } from '@/lib/library-refresh';
import { useCatalog, useFavorites, useImportCourse, useRecent, useRescan, useRewards, useSession } from '@/app/data';
import { safeStorage } from '@/app/preferences';

// Four views share one filter and sort state (docs/design/parity.md).

const LIBRARY_RETURN_KEY = 'learnvault-library-return';
const COURSE_BATCH_SIZE = 60;

export function HomePage() {
	useLocale();
	const session = useSession();
	const user = session.data?.user;
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const rewards = useRewards();
	const [params, setParams] = useSearchParams();
	// The filter and search stay one click away: the course page's breadcrumb returns to them.
	useEffect(() => { try { safeStorage()?.setItem(LIBRARY_RETURN_KEY, params.toString()); } catch { /* storage is optional */ } }, [params]);
	const look = useStore(appearanceStore);
	const customCategories = useStore(courseCategoriesStore);
	const [view, setView] = useState<LibraryView>(() => {
		try {
			return parseLibraryView(safeStorage()?.getItem(LIBRARY_VIEW_STORAGE_KEY)) ?? (appearanceStore.get().homeLayout === 'list' ? 'list' : 'covers');
		} catch {
			return appearanceStore.get().homeLayout === 'list' ? 'list' : 'covers';
		}
	});
	const [sort, setSort] = useState<LibrarySort>(() => readLibrarySort(safeStorage()));
	const [message, setMessage] = useState('');

	const query = params.get('q') ?? '';
	const filter = (params.get('filter') as LibraryFilter | null) ?? 'all';
	const category = params.get('category');
	const favoriteIds = useMemo(() => new Set(favorites.data?.courseIds ?? []), [favorites.data]);
	const library = useMemo(
		() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: favoriteIds, courseCategories: customCategories, query, filter, category }),
		[catalog.data, recent.data, favoriteIds, customCategories, query, filter, category]
	);

	const setParam = (key: string, value: string | null) => {
		const next = new URLSearchParams(params);
		if (value) next.set(key, value);
		else next.delete(key);
		setParams(next, { replace: true });
	};
	const changeView = (next: LibraryView) => {
		setView(next);
		persistLibraryView(safeStorage(), next);
	};
	const changeSort = (next: LibrarySort) => {
		setSort(next);
		persistLibrarySort(safeStorage(), next);
	};
	const sorted = useMemo(() => {
		const list = [...library.visible];
		if (sort === 'title') list.sort((a, b) => a.course.title.localeCompare(b.course.title));
		else if (sort === 'progress') list.sort((a, b) => (b.total ? b.done / b.total : 0) - (a.total ? a.done / a.total : 0));
		else list.sort((a, b) => b.lastStudiedAt - a.lastStudiedAt || a.course.title.localeCompare(b.course.title));
		return list;
	}, [library.visible, sort]);

	if (catalog.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading your courses…', 'Carregando seus cursos…')}</p>;
	if (catalog.isError) return <p role="alert" className="text-destructive">{catalog.error.message}</p>;

	const profile = user?.userId ?? '';
	const filtered = Boolean(query || filter !== 'all' || category);
	const wide = view === 'list' || view === 'compact';
	const resultsKey = JSON.stringify([view, query, filter, category, sort]);

	const completedCourses = library.entries.filter((entry) => entry.status === 'completed').length;
	const resume = !filtered && studySyncSupported && library.entries.length > 0 ? <ResumeCard next={upNext(catalog.data, recent.data ?? [])} profile={profile} /> : null;
	const pills = library.entries.length > 0 ? <FilterTabs counts={library.counts} filter={filter} onFilter={(value) => setParam('filter', value === 'all' ? null : value)} /> : null;
	const search = (
		<div role="search" className="relative min-w-0 flex-[1_1_15rem]">
			<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
			<Input type="search" value={query} onChange={(event) => setParam('q', event.target.value || null)} placeholder={tr('Search courses and lessons', 'Buscar cursos e aulas')} aria-label={tr('Search courses', 'Buscar cursos')} className="pl-9" />
		</div>
	);
	const tools = <Filters categories={library.categories} category={category} onCategory={(value) => setParam('category', value)} view={view} onView={changeView} sort={sort} onSort={changeSort} />;

	return (
		<div className="space-y-6">
			<div className="flex flex-wrap items-center justify-between gap-3">
				<h1 className="text-2xl font-semibold tracking-tight">{tr('My learning', 'Meu aprendizado')}</h1>
				{user?.isAdmin && library.entries.length > 0 && (
					<div className="flex flex-wrap items-center justify-end gap-2">
						<p role="status" aria-live="polite" className="order-last w-full text-right text-xs font-medium text-muted-foreground empty:hidden sm:order-first sm:w-auto sm:text-left">{message}</p>
						<LibraryActions onMessage={setMessage} />
					</div>
				)}
			</div>
			{(!user?.isAdmin || library.entries.length === 0) && (
				<p role="status" aria-live="polite" className="text-sm text-muted-foreground empty:hidden">{message}</p>
			)}

			{wide && library.entries.length > 0 ? (
				<div className="grid gap-4 empty:hidden">
					{resume}
					<HabitStrip rewards={rewards.data} completedCourses={completedCourses} bookmarks={favorites.data?.mediaIds.length ?? 0} />
				</div>
			) : resume}

			<section aria-labelledby="courses-heading" className="space-y-4">
				<h2 id="courses-heading" className="sr-only">{filtered ? tr('Results', 'Resultados') : tr('Your library', 'Sua biblioteca')}</h2>
				{pills}
				{library.entries.length > 0 && <div className="flex flex-wrap items-center gap-2">{search}{tools}</div>}
				{library.entries.length === 0 ? (
					<EmptyLibrary isAdmin={Boolean(user?.isAdmin)} onMessage={setMessage} />
				) : library.visible.length === 0 ? (
					<EmptyResults filter={query || category ? 'all' : filter} onClear={() => setParams({}, { replace: true })} />
				) : view === 'shelf' ? (
					<Shelf key={resultsKey} entries={sorted} profile={profile} canManage={Boolean(user?.isAdmin)} />
				) : (
					<div>
						{view === 'list' && (
							<div aria-hidden="true" className="hidden items-center gap-4 px-3 pb-2 text-xs font-medium text-muted-foreground sm:flex">
								<div className="w-11 shrink-0" />
								<div className="min-w-0 flex-1">{tr('Course', 'Curso')}</div>
								<div className="w-56 shrink-0">{tr('Progress', 'Progresso')}</div>
								<div className="hidden w-32 shrink-0 md:block">{tr('Last studied', 'Último estudo')}</div>
								<div className="w-16 shrink-0" />
								{user?.isAdmin && <div className="w-11 shrink-0" />}
							</div>
						)}
						<CourseList
							key={resultsKey}
							entries={sorted}
							profile={profile}
							canManage={Boolean(user?.isAdmin)}
							variant={view === 'covers' ? 'grid' : view}
							className={view === 'covers' ? 'grid grid-cols-1 gap-x-5 gap-y-5 sm:grid-cols-2 sm:gap-y-7 lg:grid-cols-3 xl:grid-cols-4' : view === 'compact' ? 'divide-y divide-[var(--study-rule)] border-y border-[var(--study-rule)]' : ''}
						/>
					</div>
				)}
			</section>
			{library.entries.length > 0 && !wide && (
				<aside aria-label={tr('Your habit and bookmarks', 'Seu hábito e favoritos')} className="grid gap-x-10 gap-y-6 border-t border-[var(--study-rule)] pt-6 md:grid-cols-2">
					<Habit rewards={rewards.data} completedCourses={completedCourses} />
					{studySyncSupported && look.showBookmarks && <Bookmarks mediaIds={favorites.data?.mediaIds ?? []} catalog={catalog.data} />}
				</aside>
			)}
		</div>
	);
}

	/** Quiet underline tabs: which courses to show, with tabular counts. */
	function FilterTabs({ counts, filter, onFilter }: { counts: Record<LibraryFilter, number>; filter: LibraryFilter; onFilter: (value: LibraryFilter) => void }) {
		const options: [LibraryFilter, string][] = [
			['all', tr('All courses', 'Todos os cursos')],
			['in_progress', tr('In progress', 'Em andamento')],
			['not_started', tr('Not started', 'Não iniciados')],
			['completed', tr('Completed', 'Concluídos')],
			...(studySyncSupported ? [['favorites', tr('Favorites', 'Favoritos')] as [LibraryFilter, string]] : [])
		];
		return (
			<div role="group" aria-label={tr('Show', 'Mostrar')} className="flex gap-x-4 overflow-x-auto border-b border-[var(--study-rule)] [scrollbar-width:thin] sm:gap-x-6">
				{options.map(([value, label]) => (
					<button key={value} type="button" aria-pressed={filter === value} onClick={() => onFilter(value)} className={cn('-mb-px inline-flex min-h-11 shrink-0 items-center gap-1.5 border-b-2 px-0.5 text-sm whitespace-nowrap transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset focus-visible:outline-none', filter === value ? 'border-foreground font-semibold text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground')}>
						{label}{counts[value] > 0 && <span className="text-xs text-muted-foreground tabular-nums">{counts[value]}</span>}
					</button>
				))}
			</div>
		);
	}

/** Lessons of a course in order, as the course page counts them: playable lessons only. */
const courseLessons = (course: CatalogNode): CatalogNode[] => {
	const walk = (node: CatalogNode): CatalogNode[] => (node.role === 'lesson' ? [node] : node.children.flatMap(walk));
	return walk(course).filter((node) => !isLessonMissing(node));
};

type UpNext = { courseId: string; course: CatalogNode; lesson: CatalogNode; index: number; lessons: CatalogNode[]; done: number; position: number | null };

/**
 * The lesson to study next. The newest study that is not finished resumes where it stopped; a
 * finished lesson hands over to the next unfinished lesson of its course, as the course page does.
 * Null when every studied course is complete, so the hero hides.
 */
function upNext(catalog: Catalog | undefined, recent: StudyProgress[]): UpNext | null {
	if (!catalog) return null;
	const completed = new Set(recent.filter((item) => item.completed).map((item) => item.mediaId));
	const isDone = (node: CatalogNode) => completed.has(lessonMediaId(node) ?? '');
	for (const item of recent) {
		const location = findLessonLocation(catalog, item.mediaId);
		const course = catalog.nodes.find((node) => courseIdOf(node) === location?.courseId);
		if (!location || !course) continue;
		const lessons = courseLessons(course);
		const studied = lessons.findIndex((node) => node.path === location.lesson.path);
		if (studied < 0) continue;
		const done = lessons.filter(isDone).length;
		const index = !item.completed ? studied : [...lessons.keys()].find((i) => i > studied && !isDone(lessons[i])) ?? lessons.findIndex((node) => !isDone(node));
		if (index < 0) continue;
		return { courseId: courseIdOf(course), course, lesson: lessons[index], index, lessons, done, position: !item.completed ? item.positionSeconds : null };
	}
	return null;
}

/** Up next: the lesson to study next, lit like a reading lamp over its 16:9 frame. */
function ResumeCard({ next, profile }: { next: UpNext | null; profile: string }) {
	const look = useStore(appearanceStore);
	if (!next || !look.showContinue) return null;
	const { courseId, course, lesson, index, lessons, done, position } = next;
	return (
		<section aria-label={tr('Continue learning', 'Continuar estudando')} className="lamp-pool grid gap-6 rounded-[18px] p-4 sm:grid-cols-[minmax(0,520px)_minmax(0,1fr)] sm:items-center sm:gap-9 sm:p-7">
			<div className="relative aspect-video overflow-hidden rounded-xl border border-[var(--study-rule)]" aria-hidden="true">
				<CourseCover course={course} profile={profile} eager done={done} total={lessons.length} className="size-full rounded-none ring-0" />
			</div>
			<div className="min-w-0">
				<p className="flex flex-wrap gap-x-3.5 text-[13px] text-muted-foreground">
					<span>{tr('Up next', 'A seguir')}</span>
					<span>{course.title}</span>
					<span className="tabular-nums">{tr(`Lesson ${index + 1} of ${lessons.length}`, `Aula ${index + 1} de ${lessons.length}`)}</span>
				</p>
				<h2 className="mt-2.5 text-3xl font-semibold tracking-tight text-balance sm:text-[38px] sm:leading-[1.05]">{lesson.title}</h2>
				{position !== null && position > 0 && <p className="mt-3 max-w-[42ch] text-sm text-muted-foreground">{tr(`You stopped at ${formatDuration(position)}.`, `Você parou em ${formatDuration(position)}.`)}</p>}
				<div className="mt-5 flex flex-wrap gap-2.5">
					<Link to={routerPath(href('course', courseId, lesson.path, position))} className={buttonVariants({ className: 'font-semibold' })}>
						<Play className="size-4 fill-current" aria-hidden="true" />{position ? tr(`Resume at ${formatDuration(position)}`, `Retomar em ${formatDuration(position)}`) : tr('Start lesson', 'Começar aula')}
					</Link>
					<Link to={routerPath(href('course', courseId))} className={buttonVariants({ variant: 'outline' })}>{tr('Open course', 'Abrir curso')}</Link>
				</div>
				<LessonSegments className="mt-6 max-w-[300px]" done={done} total={lessons.length} current={index} />
			</div>
		</section>
	);
}

function CourseList({ entries, profile, canManage, variant, className }: {
	entries: CourseEntry[];
	profile: string;
	canManage: boolean;
	variant: 'grid' | 'list' | 'compact' | 'shelf';
	className: string;
}) {
	const [limit, setLimit] = useState(COURSE_BATCH_SIZE);
	const list = useRef<HTMLUListElement>(null);

	const showMore = () => {
		flushSync(() => setLimit(limit + COURSE_BATCH_SIZE));
		list.current?.children[limit]?.querySelector('a')?.focus();
	};

	return (
		<>
			<ul ref={list} className={className}>
				{entries.slice(0, limit).map((entry) => <CourseCard key={entry.courseId} entry={entry} profile={profile} canManage={canManage} variant={variant} />)}
			</ul>
			{entries.length > limit && (
				<Button type="button" variant="outline" className="mt-3 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-foreground" onClick={showMore}>
					{tr('Show more courses', 'Mostrar mais cursos')}
				</Button>
			)}
		</>
	);
}

/** Courses as horizontal shelves, grouped by how far the reader has got. */
function Shelf({ entries, profile, canManage }: { entries: CourseEntry[]; profile: string; canManage: boolean }) {
	const groups = [
		{ status: 'in_progress', label: tr('In progress', 'Em andamento') },
		{ status: 'not_started', label: tr('Not started', 'Não iniciados') },
		{ status: 'completed', label: tr('Completed', 'Concluídos') }
	] as const;
	return (
		<div className="space-y-8">
			{groups.map(({ status, label }) => {
				const items = entries.filter((entry) => entry.status === status);
				if (items.length === 0) return null;
				return (
					<section key={status} aria-label={label} className="space-y-3">
						<div className="flex items-center justify-between gap-3">
							<h3 className="text-lg font-bold">{label}</h3>
							<span className="text-sm text-muted-foreground tabular-nums">{items.length}</span>
						</div>
						<CourseList entries={items} profile={profile} canManage={canManage} variant="shelf" className="flex snap-x gap-5 overflow-x-auto pb-2" />
					</section>
				);
			})}
		</div>
	);
}

/** The lessons starred in a course, tucked cleanly under the lateral habit card. */
function Bookmarks({ mediaIds, catalog }: { mediaIds: string[]; catalog: Catalog | undefined }) {
	const items = mediaIds.map((id) => findLessonLocation(catalog ?? null, id)).filter((location): location is LessonLocation => location !== null).slice(0, 5);
	return (
		<section aria-labelledby="bookmarks-heading" className="min-w-0">
			<div className="flex items-center justify-between gap-2">
				<h2 id="bookmarks-heading" className="text-sm font-semibold">{tr('Bookmarks', 'Favoritos')}</h2>
				{items.length > 0 && <span className="study-timecode text-muted-foreground">{items.length}</span>}
			</div>
			{items.length === 0 ? (
				<p className="mt-1.5 text-xs leading-relaxed text-muted-foreground">
					<Star className="mr-1.5 inline size-3.5 align-text-bottom text-muted-foreground/80" aria-hidden="true" />
					<span className="font-medium text-foreground/80">{tr('Nothing saved yet.', 'Nada salvo ainda.')}</span> {tr('Star a lesson to keep it here.', 'Marque uma aula com estrela para mantê-la aqui.')}
				</p>
			) : (
				<ul className="mt-2 divide-y divide-[var(--study-rule)]">
					{items.map((location) => (
						<li key={`${location.courseId}/${location.lesson.path}`}>
							<Link to={routerPath(href('course', location.courseId, location.lesson.path))} className="group/bm flex min-h-11 items-center gap-2 py-1.5 text-xs transition-colors duration-150 hover:text-primary focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
								<span className="min-w-0 flex-1"><span className="block truncate font-semibold">{location.lesson.title}</span><span className="block truncate text-xs text-muted-foreground">{location.courseTitle}</span></span>
								<ChevronRight className="size-3.5 shrink-0 text-muted-foreground transition-transform duration-150 group-hover/bm:translate-x-0.5" aria-hidden="true" />
							</Link>
						</li>
					))}
				</ul>
			)}
		</section>
	);
}

/** Streak, weekly goal, level and badges in a compact 2x2 lateral panel beside the library. */
function Habit({ rewards, completedCourses }: { rewards: RewardSummary | undefined; completedCourses: number }) {
	const look = useStore(appearanceStore);
	const study = rewards?.study;
	if (!rewards || !study) return null;
	const game = buildGamification({ rewards, completedCourses });
	const days = Math.min(study.weekActiveDays, rewards.weeklyGoal);
	const remaining = Math.max(0, rewards.weeklyGoal - study.weekActiveDays);
	const earned = game.badges.filter((badge) => badge.earned).length;
	if (!(look.showStreak || look.showXp || look.showGoal || look.showBadges)) return null;
	if (!study.longestStreak && !study.weekActiveDays && !game.level.xp && !earned) return (
		<section aria-labelledby="habit-heading" className="space-y-1">
			<h2 id="habit-heading" className="text-sm font-semibold">{tr('Your habit', 'Seu hábito')}</h2>
			<p className="text-sm text-muted-foreground">{tr('Start a lesson to see your study activity here.', 'Comece uma aula para acompanhar sua atividade aqui.')}</p>
		</section>
	);
	const cards = [
		look.showStreak && (
			<li key="s" className="flex flex-col justify-between gap-2 border-t border-[var(--study-rule)] pt-3">
				<div className="flex items-center justify-between text-xs font-medium text-muted-foreground">
					<span>{tr('Streak', 'Sequência')}</span>
					<Flame className="size-3.5 text-[var(--reward)]" aria-hidden="true" />
				</div>
				<p className="flex items-baseline gap-1 tabular-nums">
					<span className="font-heading text-xl leading-none font-semibold text-foreground">{study.currentStreak}</span>
					<span className="text-xs text-muted-foreground">{study.currentStreak === 1 ? tr('day', 'dia') : tr('days', 'dias')}</span>
				</p>
				<div role="img" aria-label={tr(`${study.weekActiveDays} of 7 days studied this week`, `${study.weekActiveDays} de 7 dias estudados nesta semana`)} className="grid grid-cols-7 gap-1 pt-0.5">
					{study.days.map((day) => (
						<span key={day.day} className={cn('h-1 rounded-full', day.active ? 'bg-reward-fill' : 'bg-border-strong/30')} />
					))}
				</div>
			</li>
		),
		look.showXp && (
			<li key="x" className="flex flex-col justify-between gap-2 border-t border-[var(--study-rule)] pt-3">
				<div className="flex items-center justify-between text-xs font-medium text-muted-foreground">
					<span>{tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)}</span>
					<Sparkles className="size-3.5 text-primary/80" aria-hidden="true" />
				</div>
				<p className="flex items-baseline gap-1 tabular-nums">
					<span className="font-heading text-xl leading-none font-semibold text-foreground">{game.level.xp}</span>
					<span className="text-xs text-muted-foreground">XP</span>
				</p>
				<div className="h-1 overflow-hidden rounded-full bg-border-strong/30" aria-hidden="true">
					<div className="h-full rounded-full bg-primary" style={{ width: `${Math.round(game.level.progress * 100)}%` }} />
				</div>
			</li>
		),
		look.showGoal && (
			<li key="g" className="flex flex-col justify-between gap-2 border-t border-[var(--study-rule)] pt-3">
				<div className="flex items-center justify-between text-xs font-medium text-muted-foreground">
					<span>{tr('Goal', 'Meta')}</span>
					<Target className="size-3.5 text-[var(--stat-green-ink)]" aria-hidden="true" />
				</div>
				<p className="flex items-baseline gap-1 tabular-nums">
					<span className="font-heading text-xl leading-none font-semibold text-foreground">{days}/{rewards.weeklyGoal}</span>
					<span className="text-xs text-muted-foreground">{remaining === 0 ? tr('done', 'ok') : tr('days', 'dias')}</span>
				</p>
				<div className="grid gap-1" style={{ gridTemplateColumns: `repeat(${Math.max(1, rewards.weeklyGoal)}, minmax(0, 1fr))` }} aria-hidden="true">
					{Array.from({ length: Math.max(1, rewards.weeklyGoal) }, (_, i) => (
						<span key={i} className={cn('h-1 rounded-full', i < days ? 'bg-[var(--stat-green-ink)]' : 'bg-border-strong/30')} />
					))}
				</div>
			</li>
		),
		look.showBadges && (
			<li key="b">
				<Link to="/progress" className="flex h-full flex-col justify-between gap-2 border-t border-[var(--study-rule)] pt-3 transition-colors hover:border-[var(--border-strong)] hover:bg-brand-tint/45 hover:text-primary focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
					<div className="flex items-center justify-between text-xs font-medium text-muted-foreground">
						<span>{tr('Badges', 'Emblemas')}</span>
						<Award className="size-3.5 text-[var(--reward)]" aria-hidden="true" />
					</div>
					<p className="flex items-baseline gap-1 tabular-nums">
						<span className="font-heading text-xl leading-none font-semibold text-foreground">{earned}/{game.badges.length}</span>
						<span className="text-xs text-muted-foreground">{tr('earned', 'conq.')}</span>
					</p>
					<span className="text-xs text-muted-foreground">{tr('View progress →', 'Ver progresso →')}</span>
				</Link>
			</li>
		)
	].filter(Boolean);
	if (cards.length === 0) return null;
	return (
		<section aria-labelledby="habit-heading" className="space-y-2.5">
			<h2 id="habit-heading" className="text-sm font-semibold">{tr('Your habit', 'Seu hábito')}</h2>
			<ul aria-labelledby="habit-heading" className="grid grid-cols-2 gap-x-6 gap-y-4">{cards}</ul>
		</section>
	);
}

/** The habit in one row, for the list views that use the whole width. */
function HabitStrip({ rewards, completedCourses, bookmarks }: { rewards: RewardSummary | undefined; completedCourses: number; bookmarks: number }) {
	const look = useStore(appearanceStore);
	const study = rewards?.study;
	if (!rewards || !study) return null;
	const game = buildGamification({ rewards, completedCourses });
	const earned = game.badges.filter((badge) => badge.earned).length;
	const days = Math.min(study.weekActiveDays, rewards.weeklyGoal);
	if (!study.longestStreak && !days && !game.level.xp && !earned && !bookmarks) return null;
	const items = [
		look.showStreak && { key: 'streak', label: tr('Streak', 'Sequência'), value: `${study.currentStreak} ${study.currentStreak === 1 ? tr('day', 'dia') : tr('days', 'dias')}` },
		look.showXp && { key: 'level', label: tr(`Level ${game.level.level}`, `Nível ${game.level.level}`), value: `${game.level.xp} XP` },
		look.showGoal && { key: 'goal', label: tr('Goal', 'Meta'), value: `${days}/${rewards.weeklyGoal} ${tr('days', 'dias')}` },
		look.showBadges && { key: 'badges', label: tr('Badges', 'Emblemas'), value: tr(`${earned} earned`, `${earned} conquistados`) },
		look.showBookmarks && { key: 'bookmarks', label: tr('Bookmarks', 'Favoritos'), value: String(bookmarks) }
	].filter((item): item is { key: string; label: string; value: string } => Boolean(item));
	if (items.length === 0) return null;
	return (
		<dl aria-label={tr('Your habit', 'Seu hábito')} data-card className="grid grid-cols-2 items-center divide-border rounded-[1.5rem] border bg-card px-2 py-4 sm:grid-cols-5 sm:divide-x lg:grid-cols-5">
			{items.map((item) => (
				<div key={item.key} className="min-w-0 space-y-1 px-3 py-1">
					<dt className="truncate text-xs font-medium text-muted-foreground">{item.label}</dt>
					<dd className="truncate font-heading text-xl leading-tight font-bold tabular-nums">{item.value}</dd>
				</div>
			))}
		</dl>
	);
}

/** Why a filter shows nothing: say what the tab means and how to fill it, instead of a bare "no results". */
function EmptyResults({ filter, onClear }: { filter: LibraryFilter; onClear: () => void }) {
	const copy: Partial<Record<LibraryFilter, [string, string]>> = {
		completed: [tr('No completed courses yet', 'Nenhum curso concluído ainda'), tr('Finish every lesson of a course and it shows up here.', 'Conclua todas as aulas de um curso e ele aparece aqui.')],
		in_progress: [tr('Nothing in progress', 'Nada em andamento'), tr('Start a lesson and its course shows up here.', 'Comece uma aula e o curso dela aparece aqui.')],
		favorites: [tr('No favorite courses yet', 'Nenhum curso favorito ainda'), tr('Star a course to keep it here.', 'Marque um curso com estrela para mantê-lo aqui.')],
		not_started: [tr('Every course has been started', 'Todos os cursos já foram iniciados'), tr('Courses you have not opened yet would be listed here.', 'Os cursos que você ainda não abriu apareceriam aqui.')]
	};
	const [title, hint] = copy[filter] ?? [tr('No results found', 'Nenhum resultado encontrado'), ''];
	return (
		<div className="study-surface border-dashed p-8 text-center">
			<p className="font-medium">{title}</p>
			{hint && <p className="mt-1 text-sm text-muted-foreground">{hint}</p>}
			<Button variant="outline" className="mt-4" onClick={onClear}>{filter === 'all' ? tr('Clear filters and search', 'Limpar filtros e busca') : tr('Show all courses', 'Mostrar todos os cursos')}</Button>
		</div>
	);
}

function Filters(props: { categories: string[]; category: string | null; onCategory: (value: string | null) => void; view: LibraryView; onView: (view: LibraryView) => void; sort: LibrarySort; onSort: (sort: LibrarySort) => void }) {
	const views: [LibraryView, string][] = [
		['covers', tr('Grid', 'Grade')],
		['list', tr('List', 'Lista')],
		['compact', tr('Compact', 'Compacta')],
		['shelf', tr('Shelf', 'Estante')]
	];
	return (
		<>
			{props.categories.length > 1 && (
				<div className="min-w-0 max-w-full">
					<Label htmlFor="category" className="sr-only">{tr('Category', 'Categoria')}</Label>
					<select id="category" value={props.category ?? ''} onChange={(event) => props.onCategory(event.target.value || null)} className="h-11 max-w-full rounded-[var(--radius)] border border-[var(--study-rule)] bg-transparent px-2 text-xs focus-visible:outline-2 focus-visible:outline-ring sm:h-8 sm:px-2.5 sm:text-sm">
						<option value="">{tr('All categories', 'Todas as categorias')}</option>
						{props.categories.map((name) => <option key={name} value={name}>{name}</option>)}
					</select>
				</div>
			)}
			<Label htmlFor="sort" className="sr-only">{tr('Sort', 'Ordenar')}</Label>
			<select id="sort" value={props.sort} onChange={(event) => props.onSort(event.target.value as LibrarySort)} className="h-11 max-w-full rounded-[var(--radius)] border border-[var(--study-rule)] bg-transparent px-2 text-xs focus-visible:outline-2 focus-visible:outline-ring sm:h-8 sm:px-2.5 sm:text-sm">
				<option value="recent">{tr('Sort: Recent', 'Ordem: Recentes')}</option>
				<option value="title">{tr('Sort: Title A–Z', 'Ordem: Título A–Z')}</option>
				<option value="progress">{tr('Sort: Progress', 'Ordem: Progresso')}</option>
			</select>
			<div role="group" aria-label={tr('View', 'Visualização')} className="ml-auto flex max-w-full flex-wrap items-center gap-0.5 rounded-[var(--radius)] border border-[var(--study-rule)] p-0.5 sm:ml-0">
				{views.map(([value, label]) => (
					<button key={value} type="button" aria-pressed={props.view === value} onClick={() => props.onView(value)} className={cn('inline-flex min-h-11 min-w-11 items-center justify-center rounded-[calc(var(--radius)-2px)] px-2 text-xs transition-colors focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset focus-visible:outline-none sm:min-h-7 sm:px-2.5 sm:text-[13px]', props.view === value ? 'bg-muted font-semibold text-foreground' : 'text-muted-foreground hover:text-foreground')}>{label}</button>
				))}
			</div>
		</>
	);
}

export function LibraryActions({ onMessage }: { onMessage: (message: string) => void }) {
	const rescan = useRescan();
	const importCourse = useImportCourse();
	const navigate = useNavigate();
	const [importOpen, setImportOpen] = useState(false);
	const [importPath, setImportPath] = useState('');
	const [importError, setImportError] = useState('');
	const failure = (error: unknown) => {
		if (isCredentialsChange(error)) return tr('Change your password on the server before managing courses.', 'Troque sua senha no servidor antes de gerenciar cursos.');
		// The transport already worded these two for the user; any other error stays generic.
		if (error instanceof Error && (error.name === 'NoCoursesError' || error.name === 'RescanBusyError')) return error.message;
		return tr('The library action failed. Try again.', 'A ação da biblioteca falhou. Tente novamente.');
	};

	const runImport = (folder?: string) =>
		importCourse.mutate(folder, {
			onSuccess: () => {
				onMessage(tr('Course imported. Existing files are read in place; unsupported files are skipped without changing your folder.', 'Curso importado. Os arquivos são lidos no lugar; arquivos não suportados são ignorados sem alterar sua pasta.'));
				setImportOpen(false);
				setImportPath('');
				navigate('/');
			},
			onError: (error) => { if (transportKind === 'http') setImportError(failure(error)); else onMessage(failure(error)); }
		});
	const submit = (event: FormEvent) => {
		event.preventDefault();
		if (importPath.trim()) runImport(importPath);
	};

	return (
		<>
			<Button variant="outline" aria-label={tr('Refresh courses', 'Atualizar cursos')} className="px-3 sm:px-4" disabled={rescan.isPending} onClick={() => { onMessage(tr('Reading courses…', 'Lendo os cursos…')); rescan.mutate({ onProgress: onMessage }, { onSuccess: (outcome) => onMessage(refreshMessage(outcome)), onError: (error) => onMessage(failure(error)) }); }}>
				<RefreshCw className={cn(rescan.isPending && 'motion-safe:animate-spin')} />
				<span className="hidden sm:inline">{tr('Refresh courses', 'Atualizar cursos')}</span>
				<span className="sm:hidden" aria-hidden="true">{tr('Refresh', 'Atualizar')}</span>
			</Button>
			<Button variant="outline" aria-label={tr('Import course', 'Importar curso')} className="px-3 sm:px-4" disabled={importCourse.isPending} aria-haspopup={transportKind === 'http' ? 'dialog' : undefined} onClick={() => (transportKind === 'desktop' ? runImport() : setImportOpen(true))}>
				<FolderPlus />
				<span className="hidden sm:inline">{tr('Import course', 'Importar curso')}</span>
				<span className="sm:hidden" aria-hidden="true">{tr('Import', 'Importar')}</span>
			</Button>
			<Dialog open={importOpen} onOpenChange={(open) => { setImportOpen(open); if (!open) { setImportError(''); importCourse.reset(); } }}>
				<DialogContent className="sm:max-w-lg">
					<DialogHeader>
						<DialogTitle className="font-serif text-2xl">{tr('Import a course', 'Importar um curso')}</DialogTitle>
						<DialogDescription>{tr('Point LearnVault at a folder you already own. Your files are read, never moved, renamed or changed.', 'Aponte o LearnVault para uma pasta que você já tem. Seus arquivos são lidos, nunca movidos, renomeados ou alterados.')}</DialogDescription>
						<Motto name="adFontes" />
					</DialogHeader>
					<form onSubmit={submit} className="space-y-4">
						<div className="space-y-2">
							<Label htmlFor="import-path">{tr('Course folder on the server', 'Pasta do curso no servidor')}</Label>
							<Input id="import-path" value={importPath} onChange={(event) => { setImportPath(event.target.value); setImportError(''); }} placeholder="/courses/rust-from-zero" required autoComplete="off" spellCheck={false} aria-describedby="import-help import-error" aria-invalid={importError ? true : undefined} />
							<p id="import-help" className="text-sm text-muted-foreground">{tr('Type a path the server can read. The course name is taken from the folder name.', 'Digite um caminho que o servidor consiga ler. O nome do curso vem do nome da pasta.')}</p>
							<p id="import-error" role="alert" className="text-sm font-medium text-destructive empty:hidden">{importError}</p>
						</div>
						<DialogFooter>
							<DialogClose render={<Button type="button" variant="outline" />}>{tr('Cancel', 'Cancelar')}</DialogClose>
							<Button type="submit" disabled={importCourse.isPending}>{tr('Import course', 'Importar curso')}</Button>
						</DialogFooter>
					</form>
				</DialogContent>
			</Dialog>
		</>
	);
}

/** First run: say what the app does, and put the one action that matters right here. */
function EmptyLibrary({ isAdmin, onMessage }: { isAdmin: boolean; onMessage: (message: string) => void }) {
	return (
		<div className="mx-auto max-w-xl space-y-5 rounded-2xl border border-dashed p-8 text-center sm:p-12">
			<p className="font-serif text-2xl font-semibold text-balance">{tr('Bring in the courses you already have', 'Traga os cursos que você já tem')}</p>
			<p className="text-muted-foreground">
				{isAdmin
					? tr('Pick a folder of videos and documents. LearnVault builds the lessons, remembers where you stopped and never touches your files.', 'Escolha uma pasta de vídeos e documentos. O LearnVault monta as aulas, lembra onde você parou e nunca mexe nos seus arquivos.')
					: tr('Ask an administrator to give you access to a course.', 'Peça a um administrador acesso a um curso.')}
			</p>
			{isAdmin && <div className="flex flex-wrap justify-center gap-2"><LibraryActions onMessage={onMessage} /></div>}
			<Motto name="archeHemisy" />
		</div>
	);
}
