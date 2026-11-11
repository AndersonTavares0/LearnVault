import { Flame, FolderPlus, LayoutGrid, List, Play, RefreshCw, Search, Star, Target } from 'lucide-react';
import { useMemo, useState, type FormEvent } from 'react';
import { Link, useNavigate, useSearchParams } from 'react-router';
import { Band } from '@/components/band';
import { BrandMark } from '@/components/brand';
import { CourseCard, ProgressBar, routerPath } from '@/components/course-card';
import { CourseCover } from '@/components/course-cover';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { appearanceStore } from '@/lib/appearance';
import { buildGamification } from '@/lib/gamification';
import { formatCount } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { buildLibrary, type CourseEntry, type LibraryFilter, type ResumeEntry } from '@/lib/library';
import { persistLibraryView, readLibraryView } from '@/lib/library-view';
import { studySyncSupported } from '@/lib/player';
import { href } from '@/lib/router';
import { transportKind, type RewardSummary } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useCatalog, useFavorites, useImportCourse, useRecent, useRescan, useRewards, useSession } from '@/app/data';
import { safeStorage } from '@/app/preferences';

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
		<div>
			<Band>
				<div className="flex flex-wrap items-end justify-between gap-4 pb-5">
					<div className="min-w-0 space-y-1">
						<h1 className="text-3xl font-bold text-balance sm:text-4xl">{tr('My learning', 'Meu aprendizado')}</h1>
						<p className="text-ink-muted">{formatCount(library.entries.length, tr('course', 'curso'), tr('courses', 'cursos'))}</p>
					</div>
					{user?.isAdmin && library.entries.length > 0 && <LibraryActions onMessage={setMessage} onDark />}
				</div>
				{library.entries.length > 0 && <FilterTabs counts={library.counts} filter={filter} onFilter={(value) => setParam('filter', value === 'all' ? null : value)} />}
			</Band>
			<p role="status" aria-live="polite" className="mb-4 text-sm text-muted-foreground empty:hidden">{message}</p>

			<div className="space-y-10">
				{!filtered && look.showContinue && library.resume.length > 0 && <ContinueLearning items={library.resume} entries={library.entries} profile={profile} />}
				{!filtered && studySyncSupported && rewards.data && <HabitRow rewards={rewards.data} completedCourses={library.entries.filter((entry) => entry.status === 'completed').length} />}

				<section aria-labelledby="courses-heading" className="space-y-5">
					<div className="flex flex-wrap items-center gap-3">
						<h2 id="courses-heading" className="mr-auto text-2xl font-bold">{filtered ? tr('Results', 'Resultados') : tr('All courses', 'Todos os cursos')}</h2>
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
						<div className="rounded-lg border border-dashed p-8 text-center">
							<p className="font-medium">{tr('No results found', 'Nenhum resultado encontrado')}</p>
							<Button variant="outline" className="mt-4" onClick={() => setParams({}, { replace: true })}>{tr('Clear filters and search', 'Limpar filtros e busca')}</Button>
						</div>
					) : (
						<ul className={view === 'grid' ? 'grid grid-cols-1 gap-x-5 gap-y-[calc(var(--density)*2rem)] sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4' : 'divide-y border-y'}>
							{library.visible.map((entry) => <CourseCard key={entry.courseId} entry={entry} profile={profile} variant={view === 'grid' ? 'grid' : 'list'} />)}
						</ul>
					)}
				</section>
			</div>
		</div>
	);
}

/** Underline tabs on the band: which courses to show, with how many in each. */
function FilterTabs({ counts, filter, onFilter }: { counts: Record<LibraryFilter, number>; filter: LibraryFilter; onFilter: (value: LibraryFilter) => void }) {
	const options: [LibraryFilter, string][] = [
		['all', tr('All courses', 'Todos os cursos')],
		['in_progress', tr('In progress', 'Em andamento')],
		['not_started', tr('Not started', 'Não iniciados')],
		['completed', tr('Completed', 'Concluídos')],
		...(studySyncSupported ? [['favorites', tr('Favorites', 'Favoritos')] as [LibraryFilter, string]] : [])
	];
	return (
		<div role="group" aria-label={tr('Show', 'Mostrar')} className="-mb-px flex flex-wrap gap-x-6">
			{options.map(([value, label]) => (
				<button key={value} type="button" aria-pressed={filter === value} onClick={() => onFilter(value)} className={cn('min-h-12 border-b-4 border-transparent px-0.5 text-base font-semibold text-ink-muted hover:text-ink-foreground focus-visible:ring-2 focus-visible:ring-white/70 focus-visible:outline-none', filter === value && 'border-ink-foreground text-ink-foreground')}>
					{label} <span className="text-sm font-normal opacity-80">{counts[value]}</span>
				</button>
			))}
		</div>
	);
}

/** "Continue learning": the lessons you stopped in, each one click from where you left off. */
function ContinueLearning({ items, entries, profile }: { items: ResumeEntry[]; entries: CourseEntry[]; profile: string }) {
	const link = (entry: ResumeEntry) => routerPath(href('course', entry.location.courseId, entry.location.lesson.path, entry.progress.positionSeconds));
	return (
		<section aria-labelledby="continue-heading" className="space-y-4">
			<h2 id="continue-heading" className="text-2xl font-bold">{tr('Continue learning', 'Continuar estudando')}</h2>
			<ul className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
				{items.slice(0, 3).map((entry, index) => (
					<li key={entry.progress.mediaId} className="relative flex gap-4 rounded-lg border bg-card p-3 hover:bg-brand-tint">
						<CourseCoverFor entry={entries.find((course) => course.courseId === entry.location.courseId)} profile={profile} eager={index === 0} compact className="w-32 shrink-0 self-start" />
						<div className="flex min-w-0 flex-1 flex-col justify-between gap-2">
							<div className="min-w-0">
								<p className="truncate text-sm text-muted-foreground">{entry.location.courseTitle}</p>
								<h3 className="line-clamp-2 leading-snug font-bold">
									<Link to={link(entry)} className="after:absolute after:inset-0 after:content-[''] focus-visible:outline-none">{entry.location.lesson.title}</Link>
								</h3>
							</div>
							<ProgressBar value={entry.courseProgress} label={tr(`${Math.round(entry.courseProgress * 100)}% complete`, `${Math.round(entry.courseProgress * 100)}% concluído`)} />
							<span className="inline-flex w-fit items-center gap-1.5 text-sm font-bold text-primary"><Play className="size-4 fill-current" aria-hidden="true" />{tr('Resume', 'Retomar')}</span>
						</div>
					</li>
				))}
			</ul>
		</section>
	);
}

/** The habit tools in one row: this week's goal, the streak and the level. Each can be turned off in Settings. */
function HabitRow({ rewards, completedCourses }: { rewards: RewardSummary; completedCourses: number }) {
	const look = useStore(appearanceStore);
	const game = buildGamification({ rewards, completedCourses });
	const study = rewards.study;
	const goal = rewards.weeklyGoal;
	const cards = [
		look.showGoal && (
			<article key="goal" className="space-y-3 rounded-lg border bg-card p-4">
				<h3 className="flex items-center gap-2 font-bold"><Target className="size-5 text-primary" aria-hidden="true" />{tr('Weekly goal', 'Meta semanal')}</h3>
				<p className="text-2xl font-bold tabular-nums">{Math.min(study.weekActiveDays, goal)} <span className="text-base font-normal text-muted-foreground">{tr(`of ${goal} days`, `de ${goal} dias`)}</span></p>
				<div role="progressbar" aria-valuemin={0} aria-valuemax={goal} aria-valuenow={Math.min(study.weekActiveDays, goal)} aria-label={tr('Weekly goal', 'Meta semanal')} className="flex gap-1">
					{Array.from({ length: goal }, (_, index) => <span key={index} className={cn('h-2 flex-1 rounded-full', index < study.weekActiveDays ? 'bg-primary' : 'bg-muted')} />)}
				</div>
			</article>
		),
		look.showStreak && (
			<article key="streak" className="space-y-3 rounded-lg border bg-card p-4">
				<h3 className="flex items-center gap-2 font-bold"><Flame className="size-5 text-reward" aria-hidden="true" />{tr('Streak', 'Sequência')}</h3>
				<p className="text-2xl font-bold tabular-nums">{formatCount(study.currentStreak, tr('day', 'dia'), tr('days', 'dias'))}</p>
				<p className="text-sm text-muted-foreground">{tr(`Best: ${study.longestStreak}`, `Melhor: ${study.longestStreak}`)}</p>
			</article>
		),
		look.showXp && (
			<article key="xp" className="space-y-3 rounded-lg border bg-card p-4">
				<h3 className="flex items-center gap-2 font-bold"><Star className="size-5 text-reward" aria-hidden="true" />{tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)}</h3>
				<p className="text-2xl font-bold tabular-nums">{game.level.xp} <span className="text-base font-normal text-muted-foreground">XP</span></p>
				<ProgressBar value={game.level.progress} label={tr(`${game.level.xpToNext} XP to level ${game.level.level + 1}`, `${game.level.xpToNext} XP para o nível ${game.level.level + 1}`)} />
			</article>
		)
	].filter(Boolean);
	if (cards.length === 0) return null;
	return <section aria-label={tr('Your habit', 'Seu hábito')} className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">{cards}</section>;
}

/** The resume band shows the same cover as the course card, with its lesson bar. */
function CourseCoverFor({ entry, profile, eager, compact, className }: { entry?: CourseEntry; profile: string; eager?: boolean; compact?: boolean; className?: string }) {
	return entry ? <CourseCover course={entry.course} profile={profile} eager={eager} done={entry.done} total={entry.total} compact={compact} className={className} /> : null;
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

export function LibraryActions({ onMessage, onDark = false }: { onMessage: (message: string) => void; onDark?: boolean }) {
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
			<Button variant="outline" className={cn(onDark && 'border-white/60 bg-transparent text-ink-foreground hover:bg-white/10 hover:text-ink-foreground')} disabled={rescan.isPending} onClick={() => { onMessage(tr('Reading courses…', 'Lendo os cursos…')); rescan.mutate(undefined, { onSuccess: () => onMessage(tr('Catalog updated.', 'Catálogo atualizado.')), onError: (error) => onMessage(error.message) }); }}>
				<RefreshCw className={cn(rescan.isPending && 'motion-safe:animate-spin')} />{tr('Refresh courses', 'Atualizar cursos')}
			</Button>
			<Button className={cn(onDark && 'bg-ink-foreground text-ink hover:bg-white/90')} disabled={importCourse.isPending} aria-haspopup={transportKind === 'http' ? 'dialog' : undefined} onClick={() => (transportKind === 'desktop' ? runImport() : setImportOpen(true))}>
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
