import { BookOpen, ChartNoAxesColumn, History, Layers, LogOut, Moon, NotebookPen, Search, Settings, Sun, UserRound, type LucideIcon } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { Link, Navigate, NavLink, Outlet, useLocation, useNavigate } from 'react-router';
import { BrandMark } from '@/components/brand';
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
import { focusSaved, focusSync, retryFocusSessions, setFocusOutboxScope } from '@/lib/pomodoro-store';
import { playerPreferencesScope } from '@/lib/player-preferences';
import { progressOutbox, setProgressOutboxScope } from '@/lib/progress-outbox';
import { SESSION_EXPIRED_EVENT } from '@/lib/session-guard';
import { transportKind } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { keys, useSession, useSignOut } from './data';
import { clearUserLocalData, safeStorage } from './preferences';

const navItems = () => [
	{ to: '/', label: tr('My learning', 'Meu aprendizado'), icon: BookOpen, end: true },
	{ to: '/review', label: tr('Review', 'Revisão'), icon: Layers },
	{ to: '/progress', label: tr('Progress', 'Progresso'), icon: ChartNoAxesColumn },
	{ to: '/notes', label: tr('Notes', 'Notas'), icon: NotebookPen }
];

/**
 * App frame (docs/design/redesign.md, "Navigation"): topbar with the global destinations,
 * search and account menu; on phones a bottom tab bar replaces the topbar links.
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
		<main ref={main} id="main" tabIndex={-1} className="app-scroll min-h-0 flex-1 overflow-y-auto focus-visible:-outline-offset-2 focus-visible:outline-2 focus-visible:outline-ring print:overflow-visible [&_h1:focus-visible]:outline-2 [&_h1:focus-visible]:outline-offset-4 [&_h1:focus-visible]:outline-ring">
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
	return (
		// The bar and the banners stay put and the content region owns the scroll, so the page
		// scrollbar starts below the top bar instead of crossing it (issue #607).
		<div className="flex h-dvh flex-col print:block print:h-auto">
			<DocumentLock />
			<SkipLink />
			<DesktopTitlebar />
			<header className="shrink-0 border-b print:hidden">
				<div className="mx-auto flex h-[4.5rem] max-w-[var(--container-max,88rem)] items-center gap-x-8 px-5 sm:px-10">
					<Link to="/" className="inline-flex items-center gap-2.5 rounded-md font-heading text-2xl font-bold tracking-tight" aria-label={tr('LearnVault home', 'Início do LearnVault')}><BrandMark className="size-8 text-foreground" />LearnVault</Link>
					<nav aria-label={tr('Main', 'Principal')} className="hidden flex-1 justify-center gap-1 lg:flex">
						{navItems().map((item) => (
							<NavLink key={item.to} to={item.to} end={item.end} className={({ isActive }) => cn('inline-flex min-h-11 items-center rounded-xl px-4 text-[0.95rem] font-medium whitespace-nowrap text-muted-foreground transition-colors duration-150 hover:bg-brand-tint hover:text-foreground', isActive && 'bg-brand-soft font-semibold text-brand-ink')}>
								{item.label}
							</NavLink>
						))}
					</nav>
					<div className="ml-auto flex items-center gap-1 lg:ml-0">
						<GlobalSearch />
						<FocusMenu />
						<ThemeToggle />
						<AccountMenu username={user.username} userId={user.userId} />
					</div>
				</div>
			</header>
			<UpdateNotice isAdmin={Boolean(user.isAdmin)} />
			<IndexingIndicator isAdmin={Boolean(user.isAdmin)} />
			<MainContent pathname={location.pathname} />
			<MobileTabs />
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

function AccountMenu({ username, userId, wide }: { username: string; userId: string; wide?: boolean }) {
	useLocale();
	const navigate = useNavigate();
	const signOut = useSignOut();
	const [unsynced, setUnsynced] = useState<{ sessions: number; positions: number } | null>(null);
	const leave = () => signOut.mutate(undefined, { onSettled: () => { clearUserLocalData(userId); navigate('/login', { replace: true }); } });

	return (
		<>
			<DropdownMenu>
				<DropdownMenuTrigger render={wide ? <Button variant="ghost" className="min-w-0 flex-1 justify-start gap-2" aria-label={tr(`Account: ${username}`, `Conta: ${username}`)} /> : <Button variant="ghost" size="icon" aria-label={tr(`Account: ${username}`, `Conta: ${username}`)} />}>
					<span className="grid size-8 shrink-0 place-items-center rounded-full bg-primary text-sm font-bold text-primary-foreground" aria-hidden="true">{username.trim()[0]?.toUpperCase() ?? '?'}</span>{wide && <span className="truncate">{username}</span>}
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


function MobileTabs() {
	return (
		<nav aria-label={tr('Sections', 'Seções')} className="fixed inset-x-0 bottom-0 z-40 border-t bg-background/95 pb-[env(safe-area-inset-bottom)] backdrop-blur-xs lg:hidden print:hidden">
			<ul className="grid grid-cols-5">
				{navItems().map((item) => (
					<li key={item.to}>
						<NavLink to={item.to} end={item.end} aria-label={item.to === '/' ? tr(`Library: ${item.label}`, `Biblioteca: ${item.label}`) : item.label} className={({ isActive }) => cn('flex min-h-14 flex-col items-center justify-center gap-0.5 px-1 text-center text-xs text-muted-foreground', isActive && 'font-medium text-primary')}>
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

/** Page title: a tinted icon tile, the title in serif with its last word in the accent, and an optional line under it. */
export function PageHeader({ icon: Icon, title, description, actions, className }: { title: string; description?: ReactNode; actions?: ReactNode; icon?: LucideIcon; className?: string }) {
	return (
		<div className={cn('mb-8 flex flex-wrap items-end justify-between gap-4', className)}>
			<div className="flex min-w-0 items-center gap-3.5">
				{Icon && (
					<span aria-hidden="true" className="grid size-11 shrink-0 place-items-center rounded-2xl bg-brand-soft text-brand-ink ring-1 ring-primary/20">
						<Icon className="size-5" />
					</span>
				)}
				<div className="min-w-0 space-y-1">
					<h1 className="font-heading text-3xl font-bold text-balance sm:text-4xl">{title}</h1>
					{description && <div className="text-muted-foreground">{description}</div>}
				</div>
			</div>
			{actions && <div className="flex flex-wrap gap-2">{actions}</div>}
		</div>
	);
}
