import { BookOpen, ChevronRight, Flame, FolderPlus, LayoutGrid, List, Play, RefreshCw, Search, Star, Target } from 'lucide-react';
import { useMemo, useState, type FormEvent } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { BrandMark } from '@/components/brand';
import { Eyebrow, Hero, Ring, Stat } from '@/components/panel';
import { CourseCard, routerPath } from '@/components/course-card';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { appearanceStore } from '@/lib/appearance';
import { buildGamification } from '@/lib/gamification';
import { tr } from '@/lib/i18n';
import { buildLibrary, type CourseEntry, type LibraryFilter, type ResumeEntry } from '@/lib/library';
import { persistLibraryView, readLibraryView } from '@/lib/library-view';
import { studySyncSupported } from '@/lib/player';
import { findLessonLocation, href, type LessonLocation } from '@/lib/router';
import { transportKind, type Catalog, type RewardSummary } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useCatalog, useFavorites, useImportCourse, useRecent, useRescan, useRewards, useSession } from '@/app/data';
import { safeStorage } from '@/app/preferences';
import { PageHeader } from '@/app/shell';

type View = 'grid' | 'list';
// The four legacy views collapse into two (docs/design/parity.md).

export function HomePage() {
	useLocale();
	const session = useSession();
	const user = session.data?.user;
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const rewards = useRewards();
	const [params, setParams] = useSearchParams();
	const look = useStore(appearanceStore);
	const [view, setView] = useState<View>(() => (readLibraryView(safeStorage()) === 'covers' ? 'grid' : readLibraryView(safeStorage()) === 'list' ? 'list' : look.homeLayout));
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
	const changeView = (next: View) => {
		setView(next);
		persistLibraryView(safeStorage(), next === 'grid' ? 'covers' : 'list');
	};

	if (catalog.isPending) return <p role="status" className="text-muted-foreground">{tr('Loading your courses…', 'Carregando seus cursos…')}</p>;
	if (catalog.isError) return <p role="alert" className="text-destructive">{catalog.error.message}</p>;

	const profile = user?.userId ?? '';
	const filtered = Boolean(query || filter !== 'all' || category);

	return (
		<div className="space-y-8">
			<PageHeader icon={BookOpen} title={tr('My learning', 'Meu aprendizado')} actions={user?.isAdmin && library.entries.length > 0 ? <LibraryActions onMessage={setMessage} /> : undefined} />
			{library.entries.length > 0 && <FilterTabs counts={library.counts} filter={filter} onFilter={(value) => setParam('filter', value === 'all' ? null : value)} />}
			<p role="status" aria-live="polite" className="text-sm text-muted-foreground empty:hidden">{message}</p>

			<div className="grid items-start gap-6 lg:grid-cols-[minmax(0,1fr)_21rem]">
				<div className="min-w-0 space-y-10">
					{!filtered && studySyncSupported && library.entries.length > 0 && <ResumeCard items={library.resume} entries={library.entries} />}

					<section aria-labelledby="courses-heading" className="space-y-5">
						<div className="flex flex-wrap items-center gap-3">
							<h2 id="courses-heading" className="mr-auto font-heading text-2xl font-bold">{filtered ? tr('Results', 'Resultados') : tr('Your library', 'Sua biblioteca')}</h2>
							{library.entries.length > 0 && <Filters categories={library.categories} category={category} onCategory={(value) => setParam('category', value)} view={view} onView={changeView} />}
						</div>
						{library.entries.length > 0 && (
							<div role="search" className="relative md:hidden">
								<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
								<Input type="search" value={query} onChange={(event) => setParam('q', event.target.value || null)} placeholder={tr('Search courses and lessons', 'Buscar cursos e aulas')} aria-label={tr('Search courses', 'Buscar cursos')} className="pl-9" />
							</div>
						)}
						{library.entries.length === 0 ? (
							<EmptyLibrary isAdmin={Boolean(user?.isAdmin)} onMessage={setMessage} />
						) : library.visible.length === 0 ? (
							<div className="rounded-[2rem] border border-dashed p-8 text-center">
								<p className="font-medium">{tr('No results found', 'Nenhum resultado encontrado')}</p>
								<Button variant="outline" className="mt-4" onClick={() => setParams({}, { replace: true })}>{tr('Clear filters and search', 'Limpar filtros e busca')}</Button>
							</div>
						) : (
							<ul className={view === 'grid' ? 'grid grid-cols-1 gap-x-5 gap-y-[calc(var(--density)*2rem)] sm:grid-cols-2 xl:grid-cols-3' : 'divide-y border-y'}>
								{library.visible.map((entry) => <CourseCard key={entry.courseId} entry={entry} profile={profile} variant={view === 'grid' ? 'grid' : 'list'} />)}
							</ul>
						)}
					</section>
				</div>
				{library.entries.length > 0 && (
					<aside aria-label={tr('Your habit and bookmarks', 'Seu hábito e favoritos')} className="space-y-6">
						<Habit rewards={rewards.data} completedCourses={library.entries.filter((entry) => entry.status === 'completed').length} />
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
		<div role="group" aria-label={tr('Show', 'Mostrar')} className="-mx-5 flex gap-3 overflow-x-auto px-5 pb-1 sm:mx-0 sm:flex-wrap sm:overflow-visible sm:px-0">
			{options.map(([value, label]) => (
				<button key={value} type="button" aria-pressed={filter === value} onClick={() => onFilter(value)} className={cn('min-h-12 shrink-0 rounded-full border-2 border-brand-soft bg-card px-5 text-base font-medium hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', filter === value && 'border-reward-fill bg-panel font-semibold')}>
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
	return (
		<section aria-label={tr('Continue learning', 'Continuar estudando')}>
			<Hero className="flex flex-wrap items-center justify-between gap-8">
				<div className="min-w-0 max-w-xl space-y-4">
					<Eyebrow>{tr('Next up', 'A seguir')}</Eyebrow>
					<h2 className="font-heading text-3xl font-bold text-balance sm:text-4xl">{first.location.lesson.title}</h2>
					<p className="text-lg text-muted-foreground">{first.location.courseTitle}</p>
					<div className="flex flex-wrap gap-3 pt-2">
						<Link to={routerPath(href('course', first.location.courseId, first.location.lesson.path, first.progress.positionSeconds))} className="inline-flex h-14 items-center gap-2 rounded-full bg-primary px-8 text-base font-semibold text-primary-foreground hover:bg-brand-deep focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
							<Play className="size-4 fill-current" aria-hidden="true" />{tr('Resume', 'Retomar')}
						</Link>
						<Link to={routerPath(href('course', first.location.courseId))} className="inline-flex h-14 items-center rounded-full border bg-card px-8 text-base font-medium hover:bg-brand-tint focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">{tr('Open course', 'Abrir curso')}</Link>
					</div>
				</div>
				{entry && entry.total > 0 && <Ring value={value} label={tr(`${Math.round(value * 100)}% of the course`, `${Math.round(value * 100)}% do curso`)} caption={tr('complete', 'concluído')} size={150} />}
			</Hero>
		</section>
	);
}

/** The lessons starred in a course, one click from the home page. */
function Bookmarks({ mediaIds, catalog }: { mediaIds: string[]; catalog: Catalog | undefined }) {
	const items = mediaIds.map((id) => findLessonLocation(catalog ?? null, id)).filter((location): location is LessonLocation => location !== null).slice(0, 5);
	return (
		<section aria-labelledby="bookmarks-heading" className="rounded-[2rem] border bg-card p-5 sm:p-6">
			<h2 id="bookmarks-heading" className="font-heading text-xl font-bold">{tr('Bookmarks', 'Favoritos')}</h2>
			{items.length === 0 ? (
				<p className="mt-4 text-center text-sm text-muted-foreground">{tr('Nothing saved yet. Star a lesson and it will wait for you here.', 'Nada salvo ainda. Marque uma aula com estrela e ela espera por você aqui.')}</p>
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
	const cards = [
		look.showStreak && <li key="s"><Stat icon={<Flame />} label={tr('Streak', 'Sequência')} value={study.currentStreak} unit={study.currentStreak === 1 ? tr('day', 'dia') : tr('days', 'dias')} /></li>,
		look.showXp && <li key="x"><Stat icon={<Star />} label={tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)} value={game.level.xp} unit="XP" /></li>,
		look.showGoal && <li key="g" className="col-span-2"><Stat icon={<Target />} label={tr('Goal', 'Meta')} value={`${days}/${rewards.weeklyGoal}`} unit={tr('days this week', 'dias nesta semana')} /></li>
	].filter(Boolean);
	if (cards.length === 0) return null;
	return <ul aria-label={tr('Your habit', 'Seu hábito')} className="grid grid-cols-2 gap-4">{cards}</ul>;
}

function Filters(props: { categories: string[]; category: string | null; onCategory: (value: string | null) => void; view: View; onView: (view: View) => void }) {
	return (
		<div className="flex flex-wrap items-center gap-2">
			{props.categories.length > 1 && (
				<div className="flex items-center gap-2">
					<Label htmlFor="category" className="sr-only">{tr('Category', 'Categoria')}</Label>
					<select id="category" value={props.category ?? ''} onChange={(event) => props.onCategory(event.target.value || null)} className="h-11 rounded-lg border border-input bg-background px-3 text-sm">
						<option value="">{tr('All categories', 'Todas as categorias')}</option>
						{props.categories.map((name) => <option key={name} value={name}>{name}</option>)}
					</select>
				</div>
			)}
			<div role="group" aria-label={tr('View', 'Visualização')} className="ml-auto flex gap-1">
				<Button size="icon" variant={props.view === 'grid' ? 'secondary' : 'ghost'} aria-pressed={props.view === 'grid'} aria-label={tr('Covers', 'Capas')} onClick={() => props.onView('grid')}><LayoutGrid /></Button>
				<Button size="icon" variant={props.view === 'list' ? 'secondary' : 'ghost'} aria-pressed={props.view === 'list'} aria-label={tr('List', 'Lista')} onClick={() => props.onView('list')}><List /></Button>
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

	const runImport = (folder?: string) =>
		importCourse.mutate(folder, {
			onSuccess: () => {
				onMessage(tr('Course imported.', 'Curso importado.'));
				setImportOpen(false);
				setImportPath('');
				navigate('/');
			},
			onError: (error) => { if (transportKind === 'http') setImportError(error.message); else onMessage(error.message); }
		});
	const submit = (event: FormEvent) => {
		event.preventDefault();
		if (importPath.trim()) runImport(importPath);
	};

	return (
		<>
			<Button variant="outline" disabled={rescan.isPending} onClick={() => { onMessage(tr('Reading courses…', 'Lendo os cursos…')); rescan.mutate(undefined, { onSuccess: () => onMessage(tr('Catalog updated.', 'Catálogo atualizado.')), onError: (error) => onMessage(error.message) }); }}>
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
