import { BookOpen, KeyRound, ChartNoAxesColumn, History, Layers, LogOut, Moon, NotebookPen, Search, Settings, Sun, UserRound, type LucideIcon } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { Link, Navigate, NavLink, Outlet, useLocation, useNavigate } from 'react-router';
import { routerPath } from '@/components/course-card';
import { CourseCover } from '@/components/course-cover';
import { DesktopTitlebar } from '@/components/desktop-titlebar';
import { GlobalSearch } from '@/components/global-search';
import { IndexingIndicator } from '@/components/indexing-indicator';
import { UpdateNotice } from '@/components/update-notice';
import { FocusMenu } from '@/components/focus-menu';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { appearanceStore, resolveTheme, updateAppearance } from '@/lib/appearance';
import { tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { href } from '@/lib/router';
import { focusSaved, focusSync, retryFocusSessions, setFocusOutboxScope } from '@/lib/pomodoro-store';
import { playerPreferencesScope } from '@/lib/player-preferences';
import { progressOutbox, setProgressOutboxScope } from '@/lib/progress-outbox';
import { SESSION_EXPIRED_EVENT } from '@/lib/session-guard';
import { transportKind } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { keys, useCatalog, useRecent, useSession, useSignOut } from './data';
import { clearUserLocalData, safeStorage } from './preferences';

const navItems = () => [
	{ to: '/', label: tr('My learning', 'Meu aprendizado'), icon: BookOpen, end: true },
	{ to: '/review', label: tr('Review', 'Revisão'), icon: Layers },
	{ to: '/progress', label: tr('Progress', 'Progresso'), icon: ChartNoAxesColumn },
	{ to: '/notes', label: tr('Notes', 'Notas'), icon: NotebookPen }
];

/** The shortcut GlobalSearch listens for, labelled for the platform. */
const searchShortcut = () => (/Mac|iPhone|iPad|iPod/.test(navigator.userAgent) ? '⌘K' : 'Ctrl K');

/**
 * App frame (docs/design/redesign.md, "Navigation"): on phones and tablets a top bar with search
 * and account menu, plus a bottom tab bar. From lg the same header becomes a left rail with the
 * destinations, search, courses in progress and the account controls at its foot.
 * Server builds require a session; the desktop build has a local identity.
 */
/**
 * Stops the document from scrolling while the shell is on screen (see app.css): the content region
 * scrolls instead. The sign-in page and the route error page sit outside the shell, so they keep
 * a scrollable document, which a short or zoomed viewport needs to reach their controls.
 */
function DocumentLock() {
	useLayoutEffect(() => {
		const root = document.documentElement;
		root.dataset.appShell = '';
		return () => {
			delete root.dataset.appShell;
		};
	}, []);
	return null;
}

function MainContent({ pathname }: { pathname: string }) {
	const main = useRef<HTMLElement>(null);
	const previousPath = useRef(pathname);
	useLayoutEffect(() => {
		if (previousPath.current === pathname) return;
		previousPath.current = pathname;
		const content = main.current;
		if (!content) return;
		const focusHeading = () => {
			const heading = content.querySelector('h1');
			if (!heading) return false;
			heading.tabIndex = -1;
			heading.focus();
			return true;
		};
		if (focusHeading()) return;

		// Give loading and error-only pages a focus target. Promote it to the heading
		// when it arrives, but only if the learner has not moved focus elsewhere.
		content.focus();
		const observer = new MutationObserver(() => {
			if (document.activeElement !== content || focusHeading()) observer.disconnect();
		});
		observer.observe(content, { childList: true, subtree: true });
		return () => observer.disconnect();
	}, [pathname]);

	return (
		<main ref={main} id="main" tabIndex={-1} className="app-scroll min-h-0 flex-1 overflow-y-auto scroll-pb-[calc(4.5rem+env(safe-area-inset-bottom))] focus-visible:-outline-offset-2 focus-visible:outline-2 focus-visible:outline-ring lg:scroll-pb-4 print:overflow-visible [&_h1:focus-visible]:outline-2 [&_h1:focus-visible]:outline-offset-4 [&_h1:focus-visible]:outline-ring">
			{/* The phone tab bar floats over the bottom edge, hence the extra clearance. */}
			<div className="mx-auto w-full max-w-[var(--container-max,88rem)] px-5 pt-8 pb-28 sm:px-10 sm:pt-10 sm:pb-32 lg:pb-10">
				<Outlet />
			</div>
		</main>
	);
}

export function AppShell() {
	useLocale();
	const session = useSession();
	const location = useLocation();

	const client = useQueryClient();
	const userId = session.data?.user?.userId ?? null;
	// Focus sessions and unsaved positions queue per user, so a shared browser never mixes accounts.
	useEffect(() => {
		const scope = playerPreferencesScope(transportKind, userId);
		setFocusOutboxScope(scope);
		setProgressOutboxScope(scope);
	}, [userId]);
	useEffect(() => focusSaved.subscribe(() => {
		void client.invalidateQueries({ queryKey: ['focus-summary'] });
		void client.invalidateQueries({ queryKey: keys.rewards });
	}), [client]);
	// A 401 from any API call re-reads the session; the guard below sends the visitor to /login.
	useEffect(() => {
		const onExpired = () => {
			void client.invalidateQueries({ queryKey: keys.session });
		};
		window.addEventListener(SESSION_EXPIRED_EVENT, onExpired);
		return () => window.removeEventListener(SESSION_EXPIRED_EVENT, onExpired);
	}, [client]);

	if (session.isPending) return <p role="status" className="p-8 text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>;
	if (!session.data?.user) return <Navigate to="/login" replace state={{ from: `${location.pathname}${location.search}` }} />;

	const user = session.data.user;
	// A temporary password only unlocks the account page: every other route would answer 403.
	const locked = transportKind === 'http' && user.mustChangeCredentials;
	if (locked && location.pathname !== '/settings') return <Navigate to="/settings#profile" replace />;
	return (
		// The bar and the banners stay put and the content region owns the scroll, so the page
		// scrollbar starts below the top bar instead of crossing it (issue #607).
		<div className="flex h-dvh flex-col print:block print:h-auto">
			<DocumentLock />
			<SkipLink />
			<DesktopTitlebar />
			<div className="flex min-h-0 flex-1 flex-col lg:flex-row print:block">
				<header className="shrink-0 border-b border-[var(--study-rule)] bg-background/95 lg:w-[236px] lg:border-r lg:border-b-0 lg:bg-sidebar print:hidden">
					<div className="mx-auto flex h-16 max-w-[var(--container-max,88rem)] items-center gap-x-3 px-4 sm:gap-x-8 sm:px-10 lg:h-full lg:max-w-none lg:flex-col lg:items-stretch lg:gap-1 lg:px-3 lg:py-4">
						<Link to="/" className="inline-flex min-h-11 items-center rounded-md font-heading text-xl font-bold tracking-tight sm:text-2xl lg:min-h-0 lg:px-2.5 lg:pt-1 lg:pb-3.5 lg:text-base lg:font-semibold" aria-label={tr('LearnVault home', 'Início do LearnVault')}>LearnVault</Link>
						{!locked && (
							<div className="relative lg:[&_input]:pr-14">
								<GlobalSearch />
								<kbd aria-hidden="true" className="pointer-events-none absolute top-1/2 right-2.5 hidden -translate-y-1/2 rounded-[5px] border border-border px-1.5 text-[0.6875rem] text-muted-foreground lg:block">{searchShortcut()}</kbd>
							</div>
						)}
						{!locked && (
							<nav aria-label={tr('Main', 'Principal')} className="hidden flex-col gap-0.5 lg:flex">
								{navItems().map((item) => (
									<NavLink key={item.to} to={item.to} end={item.end} className={({ isActive }) => cn('flex h-8 items-center gap-2.5 rounded-[7px] px-2.5 text-sm font-medium whitespace-nowrap transition-colors duration-150 focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', isActive ? 'bg-muted font-semibold text-foreground' : 'text-muted-foreground hover:bg-muted/60 hover:text-foreground')}>
										<item.icon className="size-4 shrink-0 opacity-80" aria-hidden="true" />
										{item.label}
									</NavLink>
								))}
							</nav>
						)}
						{!locked && <InProgress profile={user.userId} />}
						<div className="ml-auto flex items-center gap-1 lg:mt-auto lg:ml-0 lg:w-full lg:gap-0.5 lg:border-t lg:border-[var(--study-rule)] lg:pt-2.5">
							{!locked && <FocusMenu />}
							<ThemeToggle />
							<AccountMenu username={user.username} userId={user.userId} />
						</div>
					</div>
				</header>
				<div className="flex min-h-0 min-w-0 flex-1 flex-col">
					{locked ? <CredentialsBanner /> : (
						<>
							<UpdateNotice isAdmin={Boolean(user.isAdmin)} />
							<IndexingIndicator isAdmin={Boolean(user.isAdmin)} />
						</>
					)}
					<MainContent pathname={location.pathname} />
				</div>
				{!locked && <MobileTabs />}
			</div>
		</div>
	);
}

/** Explains why the library is closed until the temporary password has been replaced. */
function CredentialsBanner() {
	useLocale();
	return (
		<div role="alert" className="shrink-0 border-b border-warning/40 bg-warning-tint print:hidden">
			<div className="mx-auto flex max-w-[var(--container-max,88rem)] items-start gap-3 px-4 py-3 text-sm sm:px-10">
				<KeyRound className="mt-0.5 size-4 shrink-0" aria-hidden="true" />
				<p className="text-pretty"><strong className="font-semibold">{tr('Change your temporary password first.', 'Troque primeiro a senha temporária.')}</strong> {tr('This account still uses the temporary sign-in. Choose a new username and password below to open your library.', 'Esta conta ainda usa o acesso temporário. Escolha abaixo um novo usuário e uma nova senha para abrir sua biblioteca.')}</p>
			</div>
		</div>
	);
}

/** One press switches between the light and the dark page; "follow the system" is in Settings. */
function ThemeToggle() {
	const look = useStore(appearanceStore);
	const dark = resolveTheme(look.theme) === 'dark';
	return (
		<Button variant="ghost" size="icon" aria-label={dark ? tr('Switch to the light theme', 'Usar o tema claro') : tr('Switch to the dark theme', 'Usar o tema escuro')} onClick={() => updateAppearance({ theme: dark ? 'light' : 'dark' }, safeStorage())}>
			{dark ? <Sun /> : <Moon />}
		</Button>
	);
}

function SkipLink() {
	return (
		<a href="#main" onClick={(event) => { event.preventDefault(); document.getElementById('main')?.focus(); }} className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-md focus:bg-background focus:px-4 focus:py-3 focus:ring-3 focus:ring-ring">
			{tr('Skip to content', 'Pular para o conteúdo')}
		</a>
	);
}

function AccountMenu({ username, userId }: { username: string; userId: string }) {
	useLocale();
	const navigate = useNavigate();
	const signOut = useSignOut();
	const [unsynced, setUnsynced] = useState<{ sessions: number; positions: number } | null>(null);
	const leave = () => signOut.mutate(undefined, { onSettled: () => { clearUserLocalData(userId); navigate('/login', { replace: true }); } });

	return (
		<>
			<DropdownMenu>
				<DropdownMenuTrigger render={<Button variant="ghost" size="icon" className="lg:order-first lg:min-w-0 lg:flex-1 lg:justify-start lg:gap-2 lg:px-2" aria-label={tr(`Account: ${username}`, `Conta: ${username}`)} />}>
					<span className="grid size-8 shrink-0 place-items-center rounded-full bg-primary text-sm font-bold text-primary-foreground" aria-hidden="true">{username.trim()[0]?.toUpperCase() ?? '?'}</span>
					<span className="hidden truncate text-sm font-medium lg:block">{username}</span>
				</DropdownMenuTrigger>
				<DropdownMenuContent align="end" className="w-56">
					{/* Base UI requires group labels inside a group. */}
					<DropdownMenuGroup>
						<DropdownMenuLabel className="truncate">{username}</DropdownMenuLabel>
					</DropdownMenuGroup>
					<DropdownMenuSeparator />
					<DropdownMenuItem onClick={() => navigate('/settings#profile')}><UserRound />{tr('Profile', 'Perfil')}</DropdownMenuItem>
					<DropdownMenuItem onClick={() => navigate('/history')}><History />{tr('Watch history', 'Histórico de aulas')}</DropdownMenuItem>
					<DropdownMenuItem onClick={() => navigate('/settings')}><Settings />{tr('Settings', 'Ajustes')}</DropdownMenuItem>
					{transportKind === 'http' && (
						<DropdownMenuItem onClick={() => {
							const counts = { sessions: focusSync.get().pending.length, positions: progressOutbox.get().length };
							if (counts.sessions > 0 || counts.positions > 0) setUnsynced(counts);
							else leave();
						}}>
							<LogOut />{tr('Sign out', 'Sair')}
						</DropdownMenuItem>
					)}
				</DropdownMenuContent>
			</DropdownMenu>
			{/* Unsynced focus sessions stay queued for this user; unsaved positions are dropped. Sign-out warns first (docs/design/security.md). */}
			<AlertDialog open={unsynced !== null} onOpenChange={(open) => !open && setUnsynced(null)}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>{tr('Some study data is not saved yet', 'Alguns dados de estudo ainda não foram salvos')}</AlertDialogTitle>
						<AlertDialogDescription>
							{[
								unsynced?.sessions ? tr(`${unsynced.sessions} focus session(s) are waiting to reach the server. If you sign out now they stay in this browser and sync the next time you sign in here.`, `${unsynced.sessions} sessão(ões) de foco aguardam envio ao servidor. Se você sair agora, elas ficam neste navegador e sincronizam no próximo acesso por aqui.`) : '',
								unsynced?.positions ? tr('Your latest lesson position is not saved and will be discarded if you sign out now.', 'Sua posição mais recente na aula não foi salva e será descartada se você sair agora.') : ''
							].filter(Boolean).join(' ')}
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel onClick={() => void retryFocusSessions()}>{tr('Stay and sync', 'Ficar e sincronizar')}</AlertDialogCancel>
						<AlertDialogAction onClick={() => { setUnsynced(null); leave(); }}>{tr('Sign out anyway', 'Sair mesmo assim')}</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</>
	);
}

/** The rail's quiet list: up to five started courses, most recently studied first. Desktop only; hidden when empty. */
function InProgress({ profile }: { profile: string }) {
	useLocale();
	const catalog = useCatalog();
	const recent = useRecent();
	const courses = useMemo(() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: new Set() }).entries
		.filter((entry) => entry.status === 'in_progress')
		.sort((a, b) => b.lastStudiedAt - a.lastStudiedAt)
		.slice(0, 5), [catalog.data, recent.data]);
	if (courses.length === 0) return null;
	return (
		<section aria-labelledby="rail-in-progress" className="hidden pt-4 lg:block">
			<h2 id="rail-in-progress" className="px-2.5 pb-1.5 text-xs font-medium text-muted-foreground">{tr('In progress', 'Em andamento')}</h2>
			<ul className="space-y-0.5">
				{courses.map((entry) => (
					<li key={entry.courseId}>
						<Link to={routerPath(href('course', entry.courseId))} className="flex min-h-8 items-center gap-2.5 rounded-[7px] px-2.5 py-1 text-sm text-muted-foreground transition-colors hover:bg-muted/60 hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
							<CourseCover course={entry.course} profile={profile} compact className="aspect-video w-[34px] shrink-0 rounded-[4px] ring-0" />
							<span className="min-w-0 truncate">{entry.course.title}</span>
						</Link>
					</li>
				))}
			</ul>
		</section>
	);
}

function MobileTabs() {
	return (
		<nav aria-label={tr('Sections', 'Seções')} className="fixed inset-x-0 bottom-0 z-40 border-t border-[var(--study-rule)] bg-background pb-[env(safe-area-inset-bottom)] lg:hidden print:hidden">
			<ul className="grid grid-cols-5">
				{navItems().map((item) => (
					<li key={item.to}>
						<NavLink to={item.to} end={item.end} aria-label={item.to === '/' ? tr(`Library: ${item.label}`, `Biblioteca: ${item.label}`) : item.label} className={({ isActive }) => cn('relative flex min-h-14 flex-col items-center justify-center gap-0.5 px-1 text-center text-xs transition-colors', isActive ? 'font-semibold text-foreground before:absolute before:inset-x-3 before:top-0 before:h-0.5 before:bg-primary before:content-[""]' : 'text-muted-foreground')}>
							<item.icon className="size-5" aria-hidden="true" />
							{item.to === '/' ? tr('Library', 'Biblioteca') : item.label}
						</NavLink>
					</li>
				))}
				{/* The account menu stays in the topbar; the fifth slot opens the search palette full-screen. */}
				<li>
					<button type="button" onClick={() => window.dispatchEvent(new Event('learnvault:search'))} className="flex min-h-14 w-full flex-col items-center justify-center gap-0.5 text-xs text-muted-foreground">
						<Search className="size-5" aria-hidden="true" />
						{tr('Search', 'Buscar')}
					</button>
				</li>
			</ul>
		</nav>
	);
}

/** Page title: display typography with no rule under it, so the lesson content leads. */
export function PageHeader({ title, description, actions, className }: { title: string; description?: ReactNode; actions?: ReactNode; icon?: LucideIcon; className?: string }) {
	return (
		<div className={cn('mb-6 flex flex-wrap items-end justify-between gap-4', className)}>
			<div className="min-w-0 space-y-2">
				<h1 className="font-heading text-[2rem] font-semibold tracking-tight text-balance sm:text-[2.5rem] leading-[1.05]">{title}</h1>
				{description && <div className="text-sm text-muted-foreground">{description}</div>}
			</div>
			{actions && <div className="flex flex-wrap gap-2">{actions}</div>}
		</div>
	);
}
