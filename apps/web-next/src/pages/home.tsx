import { Award, ChevronRight, Flame, FolderPlus, Play, RefreshCw, Search, Sparkles, Star, Target } from 'lucide-react';
import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { BrandMark, Motto } from '@/components/brand';
import { CourseCover } from '@/components/course-cover';
import { Stat } from '@/components/panel';
import { CourseCard, ProgressBar, routerPath } from '@/components/course-card';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { appearanceStore } from '@/lib/appearance';
import { courseCategoriesStore } from '@/lib/course-categories';
import { buildGamification } from '@/lib/gamification';
import { getLocale, tr } from '@/lib/i18n';
import { buildLibrary, type CourseEntry, type LibraryFilter, type ResumeEntry } from '@/lib/library';
import { LIBRARY_VIEW_STORAGE_KEY, parseLibraryView, persistLibrarySort, persistLibraryView, readLibrarySort, type LibrarySort, type LibraryView } from '@/lib/library-view';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findLessonLocation, href, type LessonLocation } from '@/lib/router';
import { isCredentialsChange, transportKind, type Catalog, type RewardSummary } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { refreshMessage } from '@/lib/library-refresh';
import { useCatalog, useFavorites, useImportCourse, useRecent, useRescan, useRewards, useSession } from '@/app/data';
import { safeStorage } from '@/app/preferences';
import { PageHeader } from '@/app/shell';

// Four views share one filter and sort state (docs/design/parity.md).

const LIBRARY_RETURN_KEY = 'learnvault-library-return';

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

	const completedCourses = library.entries.filter((entry) => entry.status === 'completed').length;
	const resume = !filtered && studySyncSupported && library.entries.length > 0 ? <ResumeCard items={library.resume} entries={library.entries} profile={profile} compact={wide} /> : null;
	const pills = library.entries.length > 0 ? <FilterTabs counts={library.counts} filter={filter} onFilter={(value) => setParam('filter', value === 'all' ? null : value)} /> : null;
	const search = (
		<div role="search" className={cn('relative min-w-44 flex-1', wide && 'min-w-60 sm:max-w-xs')}>
			<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
			<Input type="search" value={query} onChange={(event) => setParam('q', event.target.value || null)} placeholder={tr('Search courses and lessons', 'Buscar cursos e aulas')} aria-label={tr('Search courses', 'Buscar cursos')} className="pl-9" />
		</div>
	);
	const tools = <Filters categories={library.categories} category={category} onCategory={(value) => setParam('category', value)} view={view} onView={changeView} sort={sort} onSort={changeSort} />;

	return (
		<div className="space-y-6">
			<PageHeader title={tr('My learning', 'Meu aprendizado')} actions={user?.isAdmin && library.entries.length > 0 ? <LibraryActions onMessage={setMessage} /> : undefined} />
			<p role="status" aria-live="polite" className="text-sm text-muted-foreground empty:hidden">{message}</p>

			<div className={cn('grid items-start gap-6', wide || library.entries.length === 0 ? 'grid-cols-1' : 'lg:grid-cols-[minmax(0,1fr)_18.5rem] xl:grid-cols-[minmax(0,1fr)_19.5rem]')}>
				<div className="min-w-0 space-y-5">
					{wide && library.entries.length > 0 ? (
						<div className="grid items-stretch gap-4 xl:grid-cols-[1.08fr_1fr]">
							{resume}
							<HabitStrip rewards={rewards.data} completedCourses={completedCourses} bookmarks={favorites.data?.mediaIds.length ?? 0} />
						</div>
					) : resume}

					<section aria-labelledby="courses-heading" className="space-y-4">
						<h2 id="courses-heading" className="sr-only">{filtered ? tr('Results', 'Resultados') : tr('Your library', 'Sua biblioteca')}</h2>
						{wide ? (
							<div className="flex flex-wrap items-center gap-2">{pills}<div className="ml-auto flex flex-wrap items-center gap-2">{library.entries.length > 0 && search}{library.entries.length > 0 && tools}</div></div>
						) : (
							<>
								{pills}
								{library.entries.length > 0 && <div className="flex flex-wrap items-center gap-2">{search}{tools}</div>}
							</>
						)}
						{library.entries.length === 0 ? (
							<EmptyLibrary isAdmin={Boolean(user?.isAdmin)} onMessage={setMessage} />
						) : library.visible.length === 0 ? (
							<EmptyResults filter={query || category ? 'all' : filter} onClear={() => setParams({}, { replace: true })} />
						) : view === 'shelf' ? (
							<Shelf entries={sorted} profile={profile} canManage={Boolean(user?.isAdmin)} />
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
								<ul className={view === 'covers' ? 'grid grid-cols-1 gap-3.5 sm:grid-cols-2 xl:grid-cols-3' : view === 'compact' ? 'divide-y border-y' : ''}>
									{sorted.map((entry) => <CourseCard key={entry.courseId} entry={entry} profile={profile} canManage={Boolean(user?.isAdmin)} variant={view === 'covers' ? 'grid' : view} />)}
								</ul>
							</div>
						)}
					</section>
				</div>
				{library.entries.length > 0 && !wide && (
					<aside aria-label={tr('Your habit and bookmarks', 'Seu hábito e favoritos')} className="space-y-2.5">
						<Habit rewards={rewards.data} completedCourses={completedCourses} />
						{studySyncSupported && look.showBookmarks && <Bookmarks mediaIds={favorites.data?.mediaIds ?? []} catalog={catalog.data} />}
					</aside>
				)}
			</div>
		</div>
	);
}

/** Outlined pills: which courses to show, with how many in each. */
function FilterTabs({ counts, filter, onFilter }: { counts: Record<LibraryFilter, number>; filter: LibraryFilter; onFilter: (value: LibraryFilter) => void }) {
	const options: [LibraryFilter, string][] = [
		['all', tr('All courses', 'Todos os cursos')],
		['in_progress', tr('In progress', 'Em andamento')],
		['not_started', tr('Not started', 'Não iniciados')],
		['completed', tr('Completed', 'Concluídos')],
		...(studySyncSupported ? [['favorites', tr('Favorites', 'Favoritos')] as [LibraryFilter, string]] : [])
	];
	return (
		<div role="group" aria-label={tr('Show', 'Mostrar')} className="flex flex-wrap gap-2 sm:gap-3">
			{options.map(([value, label]) => (
				<button key={value} type="button" aria-pressed={filter === value} onClick={() => onFilter(value)} className={cn('min-h-11 shrink-0 rounded-full border border-border bg-card px-4 text-[0.9375rem] font-medium transition-[background-color,border-color,color,transform] duration-150 ease-[cubic-bezier(0.23,1,0.32,1)] hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none active:scale-[0.97]', filter === value && 'border-primary bg-brand-soft font-semibold text-brand-ink')}>
					{label}{counts[value] > 0 && <span className={cn('ml-1.5 text-sm font-normal', filter === value ? 'text-brand-ink' : 'text-muted-foreground')}>{counts[value]}</span>}
				</button>
			))}
		</div>
	);
}

/** The one thing to do first: the lesson to go back to, with how far the course is. */
function ResumeCard({ items, entries, profile, compact = false }: { items: ResumeEntry[]; entries: CourseEntry[]; profile: string; compact?: boolean }) {
	const look = useStore(appearanceStore);
	const first = items[0];
	if (!first || !look.showContinue) return null;
	const entry = entries.find((course) => course.courseId === first.location.courseId);
	const value = entry && entry.total ? entry.done / entry.total : 0;
	const remaining = entry ? Math.max(0, entry.total - entry.done) : 0;
	const studied = entry?.lastStudiedAt ? new Date(entry.lastStudiedAt * 1000).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric' }) : null;
	const position = first.progress.positionSeconds;
	return (
		<section aria-label={tr('Continue learning', 'Continuar estudando')} className={compact ? 'h-full' : undefined}>
			<div data-card className={cn('group/resume flex flex-col items-stretch gap-4 rounded-[1.5rem] border border-primary/25 bg-panel p-5 sm:flex-row sm:items-center sm:gap-5', compact ? 'h-full' : 'sm:gap-6 sm:p-6')}>
				{entry ? (
					<span className="relative hidden size-16 shrink-0 overflow-hidden rounded-lg ring-1 ring-border/75 sm:block" aria-hidden="true">
						<CourseCover course={entry.course} profile={profile} eager done={entry.done} total={entry.total} compact className="size-full rounded-none" />
					</span>
				) : (
					<span className="hidden size-16 shrink-0 place-items-center rounded-lg bg-foreground text-background sm:grid" aria-hidden="true"><Play className="size-6 fill-current" /></span>
				)}
				<div className="min-w-0 flex-1 space-y-1.5">
					<h2 className="truncate font-sans text-lg font-semibold text-balance sm:text-xl">{first.location.lesson.title}</h2>
					<p className="truncate text-sm text-muted-foreground">{first.location.courseTitle}{position > 0 && ` · ${tr('stopped at', 'parou em')} ${formatDuration(position)}`}{!compact && studied && ` · ${tr('studied', 'estudado')} ${studied}`}</p>
					{!compact && entry && entry.total > 0 && <div className="max-w-sm"><ProgressBar value={value} label={remaining === 0 ? tr('All lessons done', 'Todas as aulas concluídas') : tr(`${entry.done} of ${entry.total} lessons`, `${entry.done} de ${entry.total} aulas`)} /></div>}
				</div>
				<div className={cn('flex flex-wrap gap-2', !compact && 'sm:flex-col')}>
					<Link to={routerPath(href('course', first.location.courseId, first.location.lesson.path, first.progress.positionSeconds))} className="inline-flex h-11 items-center justify-center gap-2 rounded-xl bg-primary px-6 text-sm font-semibold whitespace-nowrap text-primary-foreground hover:bg-brand-deep focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
						<Play className="size-4 fill-current" aria-hidden="true" />{tr('Resume', 'Retomar')}
					</Link>
					{!compact && (
						<Link to={routerPath(href('course', first.location.courseId))} className="inline-flex h-11 items-center justify-center rounded-xl border bg-card px-6 text-sm font-medium whitespace-nowrap hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">{tr('Open course', 'Abrir curso')}</Link>
					)}
				</div>
			</div>
		</section>
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
						<ul className="flex snap-x gap-5 overflow-x-auto pb-2">
							{items.map((entry) => <CourseCard key={entry.courseId} entry={entry} profile={profile} canManage={canManage} variant="shelf" />)}
						</ul>
					</section>
				);
			})}
		</div>
	);
}

/** The lessons starred in a course, one click from the home page. */
function Bookmarks({ mediaIds, catalog }: { mediaIds: string[]; catalog: Catalog | undefined }) {
	const items = mediaIds.map((id) => findLessonLocation(catalog ?? null, id)).filter((location): location is LessonLocation => location !== null).slice(0, 5);
	return (
		<section aria-labelledby="bookmarks-heading" data-card className="rounded-2xl border bg-card p-3.5">
			<div className="flex items-center justify-between gap-2">
				<h2 id="bookmarks-heading" className="text-xs font-semibold text-foreground/85">{tr('Bookmarks', 'Favoritos')}</h2>
				{items.length > 0 && <span className="text-xs text-muted-foreground tabular-nums">{items.length}</span>}
			</div>
			{items.length === 0 ? (
				<div className="mt-2 flex items-center gap-2.5 text-xs text-muted-foreground">
					<Star className="size-4 shrink-0 text-muted-foreground/80" aria-hidden="true" />
					<p className="min-w-0"><span className="font-medium text-foreground/80">{tr('Nothing saved yet.', 'Nada salvo ainda.')}</span> {tr('Star a lesson to keep it here.', 'Marque uma aula com estrela para mantê-la aqui.')}</p>
				</div>
			) : (
				<ul className="mt-2.5 space-y-1.5">
					{items.map((location) => (
						<li key={`${location.courseId}/${location.lesson.path}`}>
							<Link to={routerPath(href('course', location.courseId, location.lesson.path))} className="group/bm flex min-h-11 items-center gap-2.5 rounded-xl bg-panel px-3 py-1.5 text-sm transition-colors duration-150 hover:bg-brand-soft focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
								<span className="min-w-0 flex-1"><span className="block truncate font-semibold">{location.lesson.title}</span><span className="block truncate text-xs text-muted-foreground">{location.courseTitle}</span></span>
								<ChevronRight className="size-4 shrink-0 text-muted-foreground transition-transform duration-150 ease-[cubic-bezier(0.23,1,0.32,1)] group-hover/bm:translate-x-0.5" aria-hidden="true" />
							</Link>
						</li>
					))}
				</ul>
			)}
		</section>
	);
}

/** Streak, this week's goal and level as small cards beside the library. Each can be hidden in Settings. */
function Habit({ rewards, completedCourses }: { rewards: RewardSummary | undefined; completedCourses: number }) {
	const look = useStore(appearanceStore);
	const study = rewards?.study;
	if (!rewards || !study) return null;
	const game = buildGamification({ rewards, completedCourses });
	const days = Math.min(study.weekActiveDays, rewards.weeklyGoal);
	const remaining = Math.max(0, rewards.weeklyGoal - study.weekActiveDays);
	const todayActive = study.days.find((day) => day.day === study.today)?.active ?? false;
	const dayInitial = (day: string) => new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { weekday: 'narrow' });
	const cards = [
		look.showStreak && <li key="s"><Stat compact tone="amber" icon={<Flame />} label={tr('Streak', 'Sequência')} value={study.currentStreak} unit={study.currentStreak === 1 ? tr('day', 'dia') : tr('days', 'dias')} note={todayActive ? tr('Today counted', 'Hoje já conta') : tr('Today counts', 'Hoje conta')} className="h-full">
			<div role="img" aria-label={tr(`${study.weekActiveDays} of 7 days studied this week`, `${study.weekActiveDays} de 7 dias estudados nesta semana`)} className="flex justify-between gap-0.5">
				{study.days.map((day) => (
					<span key={day.day} className="flex flex-col items-center gap-0.5">
						<span className={cn('size-2 rounded-full', day.active ? 'bg-reward-fill' : 'border border-border-strong')} />
						<span className="text-[10px] leading-none text-muted-foreground">{dayInitial(day.day)}</span>
					</span>
				))}
			</div>
		</Stat></li>,
		look.showXp && <li key="x"><Stat compact tone="violet" icon={<Sparkles />} label={tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)} value={game.level.xp} unit="XP" progress={game.level.progress} note={tr(`${game.level.xpToNext} XP to Level ${game.level.level + 1}`, `${game.level.xpToNext} XP para o Nível ${game.level.level + 1}`)} className="h-full" /></li>,
		look.showGoal && <li key="g"><Stat compact tone="green" icon={<Target />} label={tr('Goal', 'Meta')} value={`${days}/${rewards.weeklyGoal}`} unit={tr('days', 'dias')} bar={{ value: days, max: rewards.weeklyGoal }} note={remaining === 0 ? tr('Goal reached', 'Meta atingida') : remaining === 1 ? tr('1 more day', 'Mais 1 dia') : tr(`${remaining} more days`, `Mais ${remaining} dias`)} className="h-full" /></li>,
		look.showBadges && <li key="b"><Link to="/progress" className="block h-full rounded-2xl focus-visible:ring-3 focus-visible:ring-ring focus-visible:outline-none"><Stat compact tone="amber" icon={<Award />} label={tr('Badges', 'Emblemas')} value={game.badges.filter((badge) => badge.earned).length} unit={tr('earned', 'conq.')} note={tr(`${game.badges.length} milestones`, `${game.badges.length} marcos`)} className="h-full" /></Link></li>
	].filter(Boolean);
	if (cards.length === 0) return null;
	return <ul aria-label={tr('Your habit', 'Seu hábito')} className="grid grid-cols-2 gap-2.5">{cards}</ul>;
}

/** The habit in one row, for the list views that use the whole width. */
function HabitStrip({ rewards, completedCourses, bookmarks }: { rewards: RewardSummary | undefined; completedCourses: number; bookmarks: number }) {
	const look = useStore(appearanceStore);
	const study = rewards?.study;
	if (!rewards || !study) return null;
	const game = buildGamification({ rewards, completedCourses });
	const earned = game.badges.filter((badge) => badge.earned).length;
	const days = Math.min(study.weekActiveDays, rewards.weeklyGoal);
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
		<div className="rounded-[2rem] border border-dashed p-8 text-center">
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
		<div className="flex flex-wrap items-center gap-2 sm:ml-auto">
			{props.categories.length > 0 && (
				<div className="flex items-center gap-2">
					<Label htmlFor="category" className="sr-only">{tr('Category', 'Categoria')}</Label>
					<select id="category" value={props.category ?? ''} onChange={(event) => props.onCategory(event.target.value || null)} className="h-11 rounded-lg border border-input bg-background px-3 text-sm">
						<option value="">{tr('All categories', 'Todas as categorias')}</option>
						{props.categories.map((name) => <option key={name} value={name}>{name}</option>)}
					</select>
				</div>
			)}
			<div className="flex flex-wrap items-center gap-2">
				<Label htmlFor="sort" className="sr-only">{tr('Sort', 'Ordenar')}</Label>
				<select id="sort" value={props.sort} onChange={(event) => props.onSort(event.target.value as LibrarySort)} className="h-11 rounded-lg border border-input bg-background px-3 text-sm">
					<option value="recent">{tr('Sort: Recent', 'Ordem: Recentes')}</option>
					<option value="title">{tr('Sort: Title A–Z', 'Ordem: Título A–Z')}</option>
					<option value="progress">{tr('Sort: Progress', 'Ordem: Progresso')}</option>
				</select>
				<div role="group" aria-label={tr('View', 'Visualização')} className="flex overflow-hidden rounded-xl border">
					{views.map(([value, label]) => (
						<button key={value} type="button" aria-pressed={props.view === value} onClick={() => props.onView(value)} className={cn('inline-flex h-11 items-center border-l px-2.5 text-sm first:border-l-0 sm:px-3 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', props.view === value ? 'bg-brand-soft font-bold text-brand-ink' : 'font-medium hover:bg-muted')}>{label}</button>
					))}
				</div>
			</div>
		</div>
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
			<Button variant="outline" disabled={rescan.isPending} onClick={() => { onMessage(tr('Reading courses…', 'Lendo os cursos…')); rescan.mutate({ onProgress: onMessage }, { onSuccess: (outcome) => onMessage(refreshMessage(outcome)), onError: (error) => onMessage(failure(error)) }); }}>
				<RefreshCw className={cn(rescan.isPending && 'motion-safe:animate-spin')} />{tr('Refresh courses', 'Atualizar cursos')}
			</Button>
			<Button disabled={importCourse.isPending} aria-haspopup={transportKind === 'http' ? 'dialog' : undefined} onClick={() => (transportKind === 'desktop' ? runImport() : setImportOpen(true))}>
				<FolderPlus />{tr('Import course', 'Importar curso')}
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
			<BrandMark className="mx-auto size-12 text-foreground" />
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
