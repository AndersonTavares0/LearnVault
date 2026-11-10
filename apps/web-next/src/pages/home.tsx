import { Flame, FolderPlus, LayoutGrid, List, RefreshCw, Search } from 'lucide-react';
import { useMemo, useState, type FormEvent } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { BrandMark } from '@/components/brand';
import { CourseCard, CourseRow, ProgressBar, routerPath } from '@/components/course-card';
import { CourseCover } from '@/components/course-cover';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
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
import { useCatalog, useFavorites, useImportCourse, useRecent, useRescan, useRewards, useSession } from '@/app/data';
import { safeStorage } from '@/app/preferences';
import { PageHeader } from '@/app/shell';

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

	const profile = user?.userId ?? '';
	const filtered = Boolean(query || filter !== 'all' || category);
	const inProgress = library.entries.filter((entry) => entry.status === 'in_progress');
	const resumeCourseId = library.resume[0]?.location.courseId;
	const alsoInProgress = inProgress.filter((entry) => entry.courseId !== resumeCourseId).slice(0, 4);
	const streak = rewards.data?.study;

	return (
		<div className="space-y-12">
			<PageHeader
				title={tr('My learning', 'Meu aprendizado')}
				description={
					<span className="flex flex-wrap items-center gap-x-4 gap-y-1">
						<span>{formatCount(library.entries.length, tr('course', 'curso'), tr('courses', 'cursos'))}</span>
						{studySyncSupported && streak && (
							<Link to="/progress" className="inline-flex min-h-11 items-center gap-1.5 hover:text-foreground">
								<Flame className={cn('size-4', streak.currentStreak > 0 && 'text-primary')} aria-hidden="true" />
								{formatCount(streak.currentStreak, tr('day streak', 'dia seguido'), tr('day streak', 'dias seguidos'))}
								{', '}{tr(`${streak.weekActiveDays} of ${rewards.data!.weeklyGoal} days this week`, `${streak.weekActiveDays} de ${rewards.data!.weeklyGoal} dias nesta semana`)}
							</Link>
						)}
					</span>
				}
				actions={user?.isAdmin && library.entries.length > 0 && <LibraryActions onMessage={setMessage} />}
			/>
			<p role="status" aria-live="polite" className="-mt-10 text-sm text-muted-foreground empty:hidden">{message}</p>

			{library.resume.length > 0 && !filtered && <ContinueLearning items={library.resume} entries={library.entries} profile={profile} />}

			{!filtered && alsoInProgress.length > 0 && (
				<section aria-labelledby="also-heading">
					<h2 id="also-heading" className="mb-1 text-lg font-semibold">{tr('Also in progress', 'Também em andamento')}</h2>
					<ul className="grid gap-x-10 md:grid-cols-2">{alsoInProgress.map((entry) => <CourseRow key={entry.courseId} entry={entry} profile={profile} />)}</ul>
				</section>
			)}

			<section aria-labelledby="courses-heading" className="space-y-5">
				<h2 id="courses-heading" className="text-2xl font-semibold">{tr('All courses', 'Todos os cursos')}</h2>
				{library.entries.length > 0 && (
					<>
						<div role="search" className="relative md:hidden">
							<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
							<Input type="search" value={query} onChange={(event) => setParam('q', event.target.value || null)} placeholder={tr('Search courses and lessons', 'Buscar cursos e aulas')} aria-label={tr('Search courses', 'Buscar cursos')} className="pl-9" />
						</div>
						<Filters counts={library.counts} filter={filter} onFilter={(value) => setParam('filter', value === 'all' ? null : value)} categories={library.categories} category={category} onCategory={(value) => setParam('category', value)} view={view} onView={changeView} />
					</>
				)}

				{library.entries.length === 0 ? (
					<EmptyLibrary isAdmin={Boolean(user?.isAdmin)} onMessage={setMessage} />
				) : library.visible.length === 0 ? (
					<div className="rounded-lg border border-dashed p-8 text-center">
						<p className="font-medium">{tr('No results found', 'Nenhum resultado encontrado')}</p>
						<Button variant="outline" className="mt-4" onClick={() => setParams({}, { replace: true })}>{tr('Clear filters and search', 'Limpar filtros e busca')}</Button>
					</div>
				) : (
					<ul className={view === 'grid' ? 'grid grid-cols-1 gap-x-6 gap-y-10 sm:grid-cols-2 lg:grid-cols-3' : 'divide-y border-y'}>
						{library.visible.map((entry) => <CourseCard key={entry.courseId} entry={entry} profile={profile} variant={view === 'grid' ? 'grid' : 'list'} />)}
					</ul>
				)}
			</section>
		</div>
	);
}

function ContinueLearning({ items, entries, profile }: { items: ResumeEntry[]; entries: CourseEntry[]; profile: string }) {
	const [first, ...rest] = items;
	const link = (entry: ResumeEntry) => routerPath(href('course', entry.location.courseId, entry.location.lesson.path, entry.progress.positionSeconds));
	return (
		<section aria-labelledby="continue-heading" className="rounded-2xl bg-ink p-5 text-ink-foreground sm:p-8">
			<h2 id="continue-heading" className="sr-only">{tr('Continue studying', 'Continuar estudando')}</h2>
			<article className="grid items-center gap-6 md:grid-cols-[minmax(0,20rem)_1fr]">
				<CourseCoverFor entry={entries.find((entry) => entry.courseId === first.location.courseId)} profile={profile} eager />
				<div className="min-w-0 space-y-4">
					<div className="min-w-0">
						<p className="truncate text-ink-muted">{first.location.courseTitle}{first.location.moduleTitle ? `, ${first.location.moduleTitle}` : ''}</p>
						<h3 className="font-serif text-3xl leading-tight font-semibold text-balance">{first.location.lesson.title}</h3>
					</div>
					<ProgressBar value={first.courseProgress} label={tr(`${Math.round(first.courseProgress * 100)}% of the course complete`, `${Math.round(first.courseProgress * 100)}% do curso concluído`)} className="max-w-md [&_[role=progressbar]]:bg-white/20 [&_[role=progressbar]>div]:bg-[var(--a-soft)] [&_p]:text-ink-muted" />
					<Link to={link(first)} className="inline-flex h-12 items-center justify-center rounded-lg bg-[var(--a-soft)] px-6 text-sm font-semibold text-stone-950 hover:bg-white focus-visible:ring-3 focus-visible:ring-white/60 focus-visible:outline-none">{tr('Resume lesson', 'Retomar aula')}</Link>
				</div>
			</article>
			{rest.length > 0 && (
				<ul className="mt-6 grid gap-2 border-t border-white/15 pt-4 sm:grid-cols-3">
					{rest.map((entry) => (
						<li key={entry.progress.mediaId}>
							<Link to={link(entry)} className="flex min-h-11 flex-col rounded-lg px-3 py-2 hover:bg-white/10 focus-visible:ring-2 focus-visible:ring-white/60 focus-visible:outline-none">
								<span className="truncate text-sm text-ink-muted">{entry.location.courseTitle}</span>
								<span className="line-clamp-2 font-medium">{entry.location.lesson.title}</span>
							</Link>
						</li>
					))}
				</ul>
			)}
		</section>
	);
}

/** The resume band shows the same cover as the course card, with its lesson bar. */
function CourseCoverFor({ entry, profile, eager }: { entry?: CourseEntry; profile: string; eager?: boolean }) {
	return entry ? <CourseCover course={entry.course} profile={profile} eager={eager} done={entry.done} total={entry.total} /> : null;
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
