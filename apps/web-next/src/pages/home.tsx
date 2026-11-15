import { ChevronRight, FolderPlus, Play, RefreshCw, Search, Star } from 'lucide-react';
import { useEffect, useMemo, useState, type FormEvent } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { BrandMark } from '@/components/brand';
import { Stat } from '@/components/panel';
import { CourseCard, ProgressBar, routerPath } from '@/components/course-card';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { appearanceStore } from '@/lib/appearance';
import { buildGamification } from '@/lib/gamification';
import { getLocale, tr } from '@/lib/i18n';
import { buildLibrary, type CourseEntry, type LibraryFilter, type ResumeEntry } from '@/lib/library';
import { persistLibrarySort, persistLibraryView, readLibrarySort, readLibraryView, type LibrarySort, type LibraryView } from '@/lib/library-view';
import { formatDuration, studySyncSupported } from '@/lib/player';
import { findLessonLocation, href, type LessonLocation } from '@/lib/router';
import { transportKind, type Catalog, type RewardSummary } from '@/lib/transport';
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
	const [view, setView] = useState<LibraryView>(() => readLibraryView(safeStorage()));
	const [sort, setSort] = useState<LibrarySort>(() => readLibrarySort(safeStorage()));
	const [message, setMessage] = useState('');

	const query = params.get('q') ?? '';
	const filter = (params.get('filter') as LibraryFilter | null) ?? 'all';
	const category = params.get('category');
	const favoriteIds = useMemo(() => new Set(favorites.data?.courseIds ?? []), [favorites.data]);
	const library = useMemo(
		() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: favoriteIds, query, filter, category }),
		[catalog.data, recent.data, favoriteIds, query, filter, category]
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
	const resume = !filtered && studySyncSupported && library.entries.length > 0 ? <ResumeCard items={library.resume} entries={library.entries} /> : null;
	const pills = library.entries.length > 0 ? <FilterTabs counts={library.counts} filter={filter} onFilter={(value) => setParam('filter', value === 'all' ? null : value)} /> : null;
	const search = (
		<div role="search" className="relative min-w-56 flex-1 sm:max-w-sm">
			<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
			<Input type="search" value={query} onChange={(event) => setParam('q', event.target.value || null)} placeholder={tr('Search courses and lessons', 'Buscar cursos e aulas')} aria-label={tr('Search courses', 'Buscar cursos')} className="pl-9" />
		</div>
	);
	const tools = <Filters categories={library.categories} category={category} onCategory={(value) => setParam('category', value)} view={view} onView={changeView} sort={sort} onSort={changeSort} />;

	return (
		<div className="space-y-6">
			<PageHeader title={tr('My learning', 'Meu aprendizado')} actions={user?.isAdmin && library.entries.length > 0 ? <LibraryActions onMessage={setMessage} /> : undefined} />
			<p role="status" aria-live="polite" className="text-sm text-muted-foreground empty:hidden">{message}</p>

			<div className="grid items-start gap-6 lg:grid-cols-12">
				<div className={cn('min-w-0 space-y-5', wide ? 'lg:col-span-12' : 'lg:col-span-8')}>
					{wide && library.entries.length > 0 ? (
						<div className="grid gap-4 lg:grid-cols-[1.1fr_1fr]">
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
							<ul className={view === 'covers' ? 'grid grid-cols-1 gap-3.5 sm:grid-cols-2 xl:grid-cols-3' : view === 'compact' ? 'divide-y border-y' : ''}>
								{sorted.map((entry) => <CourseCard key={entry.courseId} entry={entry} profile={profile} canManage={Boolean(user?.isAdmin)} variant={view === 'covers' ? 'grid' : view} />)}
							</ul>
						)}
					</section>
				</div>
				{library.entries.length > 0 && !wide && (
					<aside aria-label={tr('Your habit and bookmarks', 'Seu hábito e favoritos')} className="space-y-3 lg:col-span-4">
						<Habit rewards={rewards.data} completedCourses={completedCourses} />
						{studySyncSupported && <Bookmarks mediaIds={favorites.data?.mediaIds ?? []} catalog={catalog.data} />}
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
				<button key={value} type="button" aria-pressed={filter === value} onClick={() => onFilter(value)} className={cn('min-h-11 shrink-0 rounded-full border border-border bg-card px-4 text-[0.9375rem] font-medium hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', filter === value && 'border-primary bg-brand-tint font-semibold')}>
					{label}{counts[value] > 0 && <span className="ml-1.5 text-sm font-normal text-muted-foreground">{counts[value]}</span>}
				</button>
			))}
		</div>
	);
}

/** The one thing to do first: the lesson to go back to, with how far the course is. */
function ResumeCard({ items, entries }: { items: ResumeEntry[]; entries: CourseEntry[] }) {
	const look = useStore(appearanceStore);
	const first = items[0];
	if (!first || !look.showContinue) return null;
	const entry = entries.find((course) => course.courseId === first.location.courseId);
	const value = entry && entry.total ? entry.done / entry.total : 0;
	const remaining = entry ? Math.max(0, entry.total - entry.done) : 0;
	const studied = entry?.lastStudiedAt ? new Date(entry.lastStudiedAt * 1000).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric' }) : null;
	const position = first.progress.positionSeconds;
	return (
		<section aria-label={tr('Continue learning', 'Continuar estudando')}>
			<div className="flex flex-wrap items-center gap-4 rounded-[1.5rem] bg-panel p-5 sm:flex-nowrap sm:gap-6">
				<span className="hidden h-[4.75rem] w-[7.5rem] shrink-0 place-items-center rounded-xl bg-foreground text-background sm:grid" aria-hidden="true"><Play className="size-7 fill-current" /></span>
				<div className="min-w-0 flex-1 space-y-1.5">
					<h2 className="font-sans text-xl font-semibold text-balance">{first.location.lesson.title}</h2>
					<p className="text-sm text-muted-foreground">{first.location.courseTitle}{position > 0 && ` · ${tr('stopped at', 'parou em')} ${formatDuration(position)}`}{studied && ` · ${tr('studied', 'estudado')} ${studied}`}</p>
					{entry && entry.total > 0 && <div className="max-w-sm"><ProgressBar value={value} label={remaining === 0 ? tr('All lessons done', 'Todas as aulas concluídas') : tr(`${entry.done} of ${entry.total} lessons`, `${entry.done} de ${entry.total} aulas`)} /></div>}
				</div>
				<div className="flex flex-wrap gap-2 sm:flex-col">
					<Link to={routerPath(href('course', first.location.courseId, first.location.lesson.path, first.progress.positionSeconds))} className="inline-flex h-11 items-center justify-center gap-2 rounded-xl bg-primary px-6 text-sm font-semibold whitespace-nowrap text-primary-foreground hover:bg-brand-deep focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
						<Play className="size-4 fill-current" aria-hidden="true" />{tr('Resume', 'Retomar')}
					</Link>
					<Link to={routerPath(href('course', first.location.courseId))} className="inline-flex h-11 items-center justify-center rounded-xl border bg-card px-6 text-sm font-medium whitespace-nowrap hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">{tr('Open course', 'Abrir curso')}</Link>
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
		<section aria-labelledby="bookmarks-heading" className="rounded-2xl border bg-card p-4">
			<h2 id="bookmarks-heading" className="text-[0.9375rem] font-bold">{tr('Bookmarks', 'Favoritos')}</h2>
			{items.length === 0 ? (
				<div className="mt-3 flex items-start gap-3 rounded-2xl border border-dashed p-4">
					<Star className="mt-0.5 size-5 shrink-0 text-muted-foreground" aria-hidden="true" />
					<p className="text-sm text-muted-foreground">{tr('Nothing saved yet.', 'Nada salvo ainda.')}<span className="mt-0.5 block text-xs">{tr('Star a lesson to keep it here.', 'Marque uma aula com estrela para mantê-la aqui.')}</span></p>
				</div>
			) : (
				<ul className="mt-4 space-y-2">
					{items.map((location) => (
						<li key={`${location.courseId}/${location.lesson.path}`}>
							<Link to={routerPath(href('course', location.courseId, location.lesson.path))} className="flex min-h-14 items-center gap-3 rounded-2xl bg-panel px-4 py-2 hover:bg-brand-soft focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
								<span className="min-w-0 flex-1"><span className="block truncate font-semibold">{location.lesson.title}</span><span className="block truncate text-sm text-muted-foreground">{location.courseTitle}</span></span>
								<ChevronRight className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
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
		look.showStreak && <li key="s"><Stat tone="amber" label={tr('Streak', 'Sequência')} value={study.currentStreak} unit={study.currentStreak === 1 ? tr('day', 'dia') : tr('days', 'dias')} note={todayActive ? tr('Today counted', 'Hoje já conta') : tr('Today counts', 'Hoje conta')} className="h-full p-4 xl:p-5">
			<div role="img" aria-label={tr(`${study.weekActiveDays} of 7 days studied this week`, `${study.weekActiveDays} de 7 dias estudados nesta semana`)} className="flex justify-between gap-1">
				{study.days.map((day) => (
					<span key={day.day} className="flex flex-col items-center gap-1">
						<span className={cn('size-2.5 rounded-full', day.active ? 'bg-reward-fill' : 'border border-border-strong')} />
						<span className="text-xs text-muted-foreground">{dayInitial(day.day)}</span>
					</span>
				))}
			</div>
		</Stat></li>,
		look.showXp && <li key="x"><Stat tone="violet" label={tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)} value={game.level.xp} unit="XP" progress={game.level.progress} note={tr(`${game.level.xpToNext} XP to Level ${game.level.level + 1}`, `${game.level.xpToNext} XP para o Nível ${game.level.level + 1}`)} className="h-full p-4 xl:p-5" /></li>,
		look.showGoal && <li key="g" className="col-span-2"><Stat tone="green" label={tr('Goal', 'Meta')} value={`${days}/${rewards.weeklyGoal}`} unit={tr('days this week', 'dias nesta semana')} bar={{ value: days, max: rewards.weeklyGoal }} note={remaining === 0 ? tr('Goal reached', 'Meta atingida') : remaining === 1 ? tr('1 more day', 'Mais 1 dia') : tr(`${remaining} more days`, `Mais ${remaining} dias`)} /></li>,
		look.showBadges && <li key="b" className="col-span-2"><Link to="/progress" className="block h-full rounded-[1.75rem] focus-visible:ring-3 focus-visible:ring-ring focus-visible:outline-none"><Stat tone="amber" label={tr('Badges', 'Emblemas')} value={game.badges.filter((badge) => badge.earned).length} unit={tr('earned', 'conquistados')} note={tr(`${game.badges.length} milestones to explore`, `${game.badges.length} marcos para explorar`)} /></Link></li>
	].filter(Boolean);
	if (cards.length === 0) return null;
	return <ul aria-label={tr('Your habit', 'Seu hábito')} className="grid grid-cols-2 gap-3">{cards}</ul>;
}

/** The habit in one row, for the list views that use the whole width. */
function HabitStrip({ rewards, completedCourses, bookmarks }: { rewards: RewardSummary | undefined; completedCourses: number; bookmarks: number }) {
	const study = rewards?.study;
	if (!rewards || !study) return null;
	const game = buildGamification({ rewards, completedCourses });
	const earned = game.badges.filter((badge) => badge.earned).length;
	const days = Math.min(study.weekActiveDays, rewards.weeklyGoal);
	const items: [string, string][] = [
		[tr('Streak', 'Sequência'), `${study.currentStreak} ${study.currentStreak === 1 ? tr('day', 'dia') : tr('days', 'dias')}`],
		[tr(`Level ${game.level.level}`, `Nível ${game.level.level}`), `${game.level.xp} XP`],
		[tr('Goal', 'Meta'), `${days}/${rewards.weeklyGoal} ${tr('days', 'dias')}`],
		[tr('Badges', 'Emblemas'), tr(`${earned} earned`, `${earned} conquistados`)],
		[tr('Bookmarks', 'Favoritos'), String(bookmarks)]
	];
	return (
		<dl aria-label={tr('Your habit', 'Seu hábito')} className="grid grid-cols-2 content-center gap-4 rounded-[1.5rem] border bg-card p-4 sm:grid-cols-5">
			{items.map(([label, value]) => <div key={label}><dt className="text-sm text-muted-foreground">{label}</dt><dd className="text-xl font-semibold">{value}</dd></div>)}
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
			{props.categories.length > 1 && (
				<div className="flex items-center gap-2">
					<Label htmlFor="category" className="sr-only">{tr('Category', 'Categoria')}</Label>
					<select id="category" value={props.category ?? ''} onChange={(event) => props.onCategory(event.target.value || null)} className="h-11 rounded-lg border border-input bg-background px-3 text-sm">
						<option value="">{tr('All categories', 'Todas as categorias')}</option>
						{props.categories.map((name) => <option key={name} value={name}>{name}</option>)}
					</select>
				</div>
			)}
			<div className="flex items-center gap-2">
				<Label htmlFor="sort" className="sr-only">{tr('Sort', 'Ordenar')}</Label>
				<select id="sort" value={props.sort} onChange={(event) => props.onSort(event.target.value as LibrarySort)} className="h-11 rounded-lg border border-input bg-background px-3 text-sm">
					<option value="recent">{tr('Sort: Recent', 'Ordem: Recentes')}</option>
					<option value="title">{tr('Sort: Title A–Z', 'Ordem: Título A–Z')}</option>
					<option value="progress">{tr('Sort: Progress', 'Ordem: Progresso')}</option>
				</select>
				<div role="group" aria-label={tr('View', 'Visualização')} className="flex overflow-hidden rounded-xl border">
					{views.map(([value, label]) => (
						<button key={value} type="button" aria-pressed={props.view === value} onClick={() => props.onView(value)} className={cn('inline-flex h-11 items-center border-l px-3 text-sm first:border-l-0 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', props.view === value ? 'bg-brand-tint font-bold' : 'font-medium hover:bg-muted')}>{label}</button>
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
		const code = error instanceof Error ? error.message : '';
		if (code === 'credentials_change_required') return tr('Change your password on the server before managing courses.', 'Troque sua senha no servidor antes de gerenciar cursos.');
		// The transport already worded these two for the user; any other error stays generic.
		if (error instanceof Error && (error.name === 'NoCoursesError' || error.name === 'RescanBusyError')) return error.message;
		return tr('The library action failed. Try again.', 'A ação da biblioteca falhou. Tente novamente.');
	};

	const runImport = (folder?: string) =>
		importCourse.mutate(folder, {
			onSuccess: () => {
				onMessage(tr('Course imported.', 'Curso importado.'));
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
			<BrandMark className="mx-auto size-12 text-primary" />
			<p className="font-serif text-2xl font-semibold text-balance">{tr('Bring in the courses you already have', 'Traga os cursos que você já tem')}</p>
			<p className="text-muted-foreground">
				{isAdmin
					? tr('Pick a folder of videos and documents. LearnVault builds the lessons, remembers where you stopped and never touches your files.', 'Escolha uma pasta de vídeos e documentos. O LearnVault monta as aulas, lembra onde você parou e nunca mexe nos seus arquivos.')
					: tr('Ask an administrator to give you access to a course.', 'Peça a um administrador acesso a um curso.')}
			</p>
			{isAdmin && <div className="flex flex-wrap justify-center gap-2"><LibraryActions onMessage={onMessage} /></div>}
		</div>
	);
}
