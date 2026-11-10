import { Flame, FolderPlus, LayoutGrid, List, RefreshCw, Search, Star } from 'lucide-react';
import { useId, useMemo, useState, type FormEvent } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { CourseCover } from '@/components/course-cover';
import { Button, buttonVariants } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { formatCount } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { buildLibrary, type CourseEntry, type LibraryFilter, type ResumeEntry } from '@/lib/library';
import { persistLibraryView, readLibraryView } from '@/lib/library-view';
import { studySyncSupported } from '@/lib/player';
import { href } from '@/lib/router';
import { transportKind } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useCatalog, useFavorites, useImportCourse, useRecent, useRescan, useRewards, useSession, useToggleCourseFavorite } from '@/app/data';
import { safeStorage } from '@/app/preferences';
import { PageHeader } from '@/app/shell';

/** Router paths drop the leading `#` of the canonical hash links built by `href()`. */
const path = (hash: string) => hash.replace(/^#/, '');
const percent = (done: number, total: number) => (total ? Math.round((done / total) * 100) : 0);

type View = 'grid' | 'list';
// The four legacy views collapse into two (docs/design/parity.md).
const readView = (): View => (['list', 'compact'].includes(readLibraryView(safeStorage())) ? 'list' : 'grid');

export function HomePage() {
	useLocale();
	const session = useSession();
	const user = session.data?.user;
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const rewards = useRewards();
	const [params, setParams] = useSearchParams();
	const [view, setView] = useState<View>(readView);
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

	const filtered = Boolean(query || filter !== 'all' || category);
	return (
		<div className="space-y-10">
			<PageHeader
				title={tr('My learning', 'Meu aprendizado')}
				description={formatCount(library.entries.length, tr('course', 'curso'), tr('courses', 'cursos'))}
				actions={user?.isAdmin && <LibraryActions onMessage={setMessage} />}
			/>
			<p role="status" aria-live="polite" className="-mt-6 text-sm text-muted-foreground empty:hidden">{message}</p>

			{library.resume.length > 0 && !filtered && <ContinueLearning items={library.resume} profile={user?.userId ?? ''} />}

			{studySyncSupported && rewards.data && !filtered && (
				<section aria-labelledby="today-heading" className="flex flex-wrap items-center gap-x-6 gap-y-2 rounded-lg border bg-card px-4 py-3 text-sm">
					<h2 id="today-heading" className="sr-only">{tr('Today', 'Hoje')}</h2>
					<span className="flex items-center gap-2">
						<Flame className={cn('size-4', rewards.data.study.currentStreak > 0 ? 'text-primary' : 'text-muted-foreground')} aria-hidden="true" />
						{formatCount(rewards.data.study.currentStreak, tr('day streak', 'dia seguido'), tr('day streak', 'dias seguidos'))}
					</span>
					<span>{tr(`${rewards.data.study.weekActiveDays} of ${rewards.data.weeklyGoal} days this week`, `${rewards.data.study.weekActiveDays} de ${rewards.data.weeklyGoal} dias nesta semana`)}</span>
					<Link to="/progress" className="ml-auto inline-flex min-h-11 items-center font-medium text-primary underline-offset-4 hover:underline">{tr('See progress', 'Ver progresso')}</Link>
				</section>
			)}

			<section aria-labelledby="courses-heading" className="space-y-4">
				<h2 id="courses-heading" className="text-lg font-semibold">{tr('All courses', 'Todos os cursos')}</h2>
				<div role="search" className="relative md:hidden">
					<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
					<Input type="search" value={query} onChange={(event) => setParam('q', event.target.value || null)} placeholder={tr('Search courses and lessons', 'Buscar cursos e aulas')} aria-label={tr('Search courses', 'Buscar cursos')} className="pl-9" />
				</div>
				<Filters counts={library.counts} filter={filter} onFilter={(value) => setParam('filter', value === 'all' ? null : value)} categories={library.categories} category={category} onCategory={(value) => setParam('category', value)} view={view} onView={changeView} />

				{library.entries.length === 0 ? (
					<EmptyLibrary isAdmin={Boolean(user?.isAdmin)} />
				) : library.visible.length === 0 ? (
					<div className="rounded-lg border border-dashed p-8 text-center">
						<p className="font-medium">{tr('No results found', 'Nenhum resultado encontrado')}</p>
						<Button variant="outline" className="mt-4" onClick={() => setParams({}, { replace: true })}>{tr('Clear filters and search', 'Limpar filtros e busca')}</Button>
					</div>
				) : (
					<ul className={view === 'grid' ? 'grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3' : 'divide-y rounded-lg border'}>
						{library.visible.map((entry) => (
							<CourseItem key={entry.courseId} entry={entry} view={view} profile={user?.userId ?? ''} />
						))}
					</ul>
				)}
			</section>
		</div>
	);
}

function ContinueLearning({ items, profile }: { items: ResumeEntry[]; profile: string }) {
	const [first, ...rest] = items;
	const link = (entry: ResumeEntry) => path(href('course', entry.location.courseId, entry.location.lesson.path, entry.progress.positionSeconds));
	return (
		<section aria-labelledby="continue-heading" className="space-y-4">
			<h2 id="continue-heading" className="text-lg font-semibold">{tr('Continue studying', 'Continuar estudando')}</h2>
			<article className="grid gap-4 rounded-xl border bg-card p-4 sm:grid-cols-[minmax(0,16rem)_1fr] sm:items-center">
				<CourseCoverFor courseId={first.location.courseId} profile={profile} eager />
				<div className="min-w-0 space-y-3">
					<div className="min-w-0">
						<p className="truncate text-sm text-muted-foreground">{first.location.courseTitle}{first.location.moduleTitle ? ` · ${first.location.moduleTitle}` : ''}</p>
						<h3 className="text-xl font-semibold text-balance">{first.location.lesson.title}</h3>
					</div>
					<ProgressBar value={first.courseProgress * 100} label={tr(`${Math.round(first.courseProgress * 100)}% of the course complete`, `${Math.round(first.courseProgress * 100)}% do curso concluído`)} />
					<Link to={link(first)} className={buttonVariants()}>{tr('Resume lesson', 'Retomar aula')}</Link>
				</div>
			</article>
			{rest.length > 0 && (
				<ul className="grid gap-3 sm:grid-cols-3">
					{rest.map((entry) => (
						<li key={entry.progress.mediaId}>
							<Link to={link(entry)} className="flex min-h-11 flex-col rounded-lg border bg-card p-3 hover:bg-muted">
								<span className="truncate text-xs text-muted-foreground">{entry.location.courseTitle}</span>
								<span className="line-clamp-2 font-medium">{entry.location.lesson.title}</span>
							</Link>
						</li>
					))}
				</ul>
			)}
		</section>
	);
}

/** Looks the course up in the cached catalog so resume cards share the same cover. */
function CourseCoverFor({ courseId, profile, eager }: { courseId: string; profile: string; eager?: boolean }) {
	const catalog = useCatalog();
	const course = catalog.data?.nodes.find((node) => node.courseId === courseId);
	return course ? <CourseCover course={course} profile={profile} eager={eager} /> : null;
}

function ProgressBar({ value, label }: { value: number; label: string }) {
	const id = useId();
	return (
		<div className="space-y-1">
			<div role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(value)} aria-labelledby={id} className="h-2 overflow-hidden rounded-full bg-muted">
				<div className="h-full rounded-full bg-primary" style={{ width: `${value}%` }} />
			</div>
			<p id={id} className="text-xs text-muted-foreground">{label}</p>
		</div>
	);
}

function Filters(props: { counts: Record<LibraryFilter, number>; filter: LibraryFilter; onFilter: (value: LibraryFilter) => void; categories: string[]; category: string | null; onCategory: (value: string | null) => void; view: View; onView: (view: View) => void }) {
	const options: [LibraryFilter, string][] = [
		['all', tr('All', 'Todos')],
		['in_progress', tr('In progress', 'Em andamento')],
		['not_started', tr('Not started', 'Não iniciados')],
		['completed', tr('Completed', 'Concluídos')],
		...(studySyncSupported ? [['favorites', tr('Favorites', 'Favoritos')] as [LibraryFilter, string]] : [])
	];
	return (
		<div className="flex flex-wrap items-center gap-2">
			<div role="group" aria-label={tr('Progress', 'Progresso')} className="flex flex-wrap gap-2">
				{options.map(([value, label]) => (
					<Button key={value} size="sm" variant={props.filter === value ? 'default' : 'outline'} aria-pressed={props.filter === value} onClick={() => props.onFilter(value)}>
						{label} <span className="tabular-nums opacity-80">{props.counts[value]}</span>
					</Button>
				))}
			</div>
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

function CourseItem({ entry, view, profile }: { entry: CourseEntry; view: View; profile: string }) {
	const toggle = useToggleCourseFavorite();
	const value = percent(entry.done, entry.total);
	const progressLabel = entry.status === 'completed' ? tr('Completed', 'Concluído') : entry.status === 'not_started' ? tr('Not started', 'Não iniciado') : tr(`${value}% complete`, `${value}% concluído`);
	const lessonsLeft = formatCount(entry.total - entry.done, tr('lesson left', 'aula restante'), tr('lessons left', 'aulas restantes'));
	const favorite = studySyncSupported && (
		<Button variant="ghost" size="icon" aria-pressed={entry.favorite} aria-label={entry.favorite ? tr(`Remove ${entry.course.title} from favorites`, `Remover ${entry.course.title} dos favoritos`) : tr(`Add ${entry.course.title} to favorites`, `Adicionar ${entry.course.title} aos favoritos`)} onClick={() => toggle.mutate({ courseId: entry.courseId, favorite: !entry.favorite })} disabled={toggle.isPending}>
			<Star className={cn(entry.favorite && 'fill-primary text-primary')} />
		</Button>
	);
	// shortcut: content-visibility skips layout of off-screen cards; virtualize (@tanstack/react-virtual) if catalogs past ~2k courses feel slow.
	return (
		<li className={cn('relative [content-visibility:auto]', view === 'grid' ? 'flex flex-col rounded-xl border bg-card' : 'flex items-center gap-4 p-3')}>
			<CourseCover course={entry.course} profile={profile} className={view === 'grid' ? 'rounded-b-none' : 'w-28 shrink-0'} />
			<div className={cn('flex min-w-0 flex-1 flex-col gap-2', view === 'grid' && 'p-4')}>
				<h3 className="font-semibold text-balance">
					<Link to={path(href('course', entry.courseId))} className="after:absolute after:inset-0 after:content-[''] hover:underline">{entry.course.title}</Link>
				</h3>
				{entry.course.sourceFolder && <p className="text-xs text-muted-foreground">{entry.course.sourceFolder}</p>}
				{entry.total > 0 && <ProgressBar value={value} label={`${progressLabel} · ${lessonsLeft}`} />}
				{entry.next && entry.status !== 'completed' && <p className="truncate text-sm text-muted-foreground">{tr('Next:', 'Próxima:')} {entry.next.title}</p>}
			</div>
			{favorite && <div className={cn('relative z-10', view === 'grid' ? 'absolute top-2 right-2 rounded-full bg-background/90' : '')}>{favorite}</div>}
		</li>
	);
}

function LibraryActions({ onMessage }: { onMessage: (message: string) => void }) {
	const rescan = useRescan();
	const importCourse = useImportCourse();
	const navigate = useNavigate();
	const [importOpen, setImportOpen] = useState(false);
	const [importPath, setImportPath] = useState('');

	const runImport = (folder?: string) =>
		importCourse.mutate(folder, {
			onSuccess: () => {
				onMessage(tr('Course imported.', 'Curso importado.'));
				setImportOpen(false);
				setImportPath('');
				navigate('/');
			},
			onError: (error) => onMessage(error.message)
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
			<Button disabled={importCourse.isPending} aria-expanded={transportKind === 'http' ? importOpen : undefined} onClick={() => (transportKind === 'desktop' ? runImport() : setImportOpen((open) => !open))}>
				<FolderPlus />{tr('Import course', 'Importar curso')}
			</Button>
			{importOpen && (
				<form onSubmit={submit} className="flex w-full flex-wrap items-end gap-2">
					<div className="min-w-0 flex-1 space-y-2">
						<Label htmlFor="import-path">{tr('Course folder on the server', 'Pasta do curso no servidor')}</Label>
						<Input id="import-path" value={importPath} onChange={(event) => setImportPath(event.target.value)} placeholder="/library/My course" required autoComplete="off" spellCheck={false} />
					</div>
					<Button type="submit" disabled={importCourse.isPending}>{tr('Import', 'Importar')}</Button>
				</form>
			)}
		</>
	);
}

function EmptyLibrary({ isAdmin }: { isAdmin: boolean }) {
	return (
		<div className="rounded-lg border border-dashed p-8 text-center">
			<p className="font-medium">{tr('No courses yet', 'Nenhum curso ainda')}</p>
			<p className="mt-1 text-sm text-muted-foreground">
				{isAdmin ? tr('Use Import course to add a folder from your library. Files are never moved or changed.', 'Use Importar curso para adicionar uma pasta da sua biblioteca. Os arquivos nunca são movidos nem alterados.') : tr('Ask an administrator to give you access to a course.', 'Peça a um administrador acesso a um curso.')}
			</p>
		</div>
	);
}
