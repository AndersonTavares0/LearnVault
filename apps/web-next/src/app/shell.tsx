import { BookOpen, ChartNoAxesColumn, Layers, CircleUser, LogOut, NotebookPen, Search, Settings } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState, type ReactNode } from 'react';
import { Link, Navigate, NavLink, Outlet, useLocation, useNavigate, useSearchParams } from 'react-router';
import { BrandMark } from '@/components/brand';
import { DesktopTitlebar } from '@/components/desktop-titlebar';
import { FocusMenu } from '@/components/focus-menu';
import { GameChips } from '@/components/game-chips';
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { DropdownMenu, DropdownMenuContent, DropdownMenuGroup, DropdownMenuItem, DropdownMenuLabel, DropdownMenuSeparator, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { useLocale } from '@/hooks/use-locale';
import { tr } from '@/lib/i18n';
import { focusSaved, focusSync, retryFocusSessions, setFocusOutboxScope } from '@/lib/pomodoro-store';
import { playerPreferencesScope } from '@/lib/player-preferences';
import { transportKind } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { keys, useSession, useSignOut } from './data';
import { clearUserLocalData } from './preferences';

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
export function AppShell() {
	useLocale();
	const session = useSession();
	const location = useLocation();

	const client = useQueryClient();
	const userId = session.data?.user?.userId ?? null;
	// Focus sessions queue per user, so a shared browser never mixes accounts.
	useEffect(() => {
		setFocusOutboxScope(playerPreferencesScope(transportKind, userId));
	}, [userId]);
	useEffect(() => focusSaved.subscribe(() => {
		void client.invalidateQueries({ queryKey: ['focus-summary'] });
		void client.invalidateQueries({ queryKey: keys.rewards });
	}), [client]);

	if (session.isPending) return <p role="status" className="p-8 text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>;
	if (!session.data?.user) return <Navigate to="/login" replace state={{ from: `${location.pathname}${location.search}` }} />;

	return (
		<div className="min-h-dvh pb-16 sm:pb-0">
			<SkipLink />
			<DesktopTitlebar />
			<Topbar username={session.data.user.username} userId={session.data.user.userId} />
			<main id="main" tabIndex={-1} className="mx-auto w-full max-w-7xl px-4 py-6 outline-none sm:py-8">
				<Outlet />
			</main>
			<MobileTabs />
		</div>
	);
}

function SkipLink() {
	return (
		<a href="#main" onClick={(event) => { event.preventDefault(); document.getElementById('main')?.focus(); }} className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-md focus:bg-background focus:px-4 focus:py-3 focus:ring-3 focus:ring-ring">
			{tr('Skip to content', 'Pular para o conteúdo')}
		</a>
	);
}

function Topbar({ username, userId }: { username: string; userId: string }) {
	useLocale();
	const navigate = useNavigate();
	const signOut = useSignOut();
	const [unsynced, setUnsynced] = useState(0);
	const leave = () => signOut.mutate(undefined, { onSettled: () => { clearUserLocalData(userId); navigate('/login', { replace: true }); } });

	return (
		<header className="sticky top-0 z-40 border-b print:hidden bg-background/95 backdrop-blur supports-backdrop-filter:bg-background/80">
			<div className="mx-auto flex h-[4.25rem] max-w-7xl items-center gap-2 px-4">
				<Link to="/" className="mr-2 flex min-h-11 items-center gap-2 rounded-md font-semibold" aria-label={tr('LearnVault home', 'Início do LearnVault')}>
					<BrandMark className="size-7 text-primary" />
					<span className="hidden lg:inline">LearnVault</span>
				</Link>
				<nav aria-label={tr('Main', 'Principal')} className="hidden items-center gap-1 sm:flex">
					{navItems().map((item) => (
						<NavLink key={item.to} to={item.to} end={item.end} className={({ isActive }) => cn('inline-flex min-h-11 items-center rounded-md px-2 text-sm font-medium whitespace-nowrap text-muted-foreground lg:px-3 hover:bg-muted hover:text-foreground', isActive && 'text-foreground underline decoration-primary decoration-2 underline-offset-[1.15rem]')}>
							{item.label}
						</NavLink>
					))}
				</nav>
				<div className="ml-auto flex items-center gap-1">
					<SearchBox />
					<GameChips />
					<FocusMenu />
					<DropdownMenu>
						<DropdownMenuTrigger render={<Button variant="ghost" size="icon" aria-label={tr(`Account: ${username}`, `Conta: ${username}`)} />}>
							<CircleUser />
						</DropdownMenuTrigger>
						<DropdownMenuContent align="end" className="w-56">
							{/* Base UI requires group labels inside a group. */}
							<DropdownMenuGroup>
								<DropdownMenuLabel className="truncate">{username}</DropdownMenuLabel>
							</DropdownMenuGroup>
							<DropdownMenuSeparator />
							<DropdownMenuItem onClick={() => navigate('/settings')}><Settings />{tr('Settings', 'Ajustes')}</DropdownMenuItem>
							{transportKind === 'http' && (
								<DropdownMenuItem onClick={() => (focusSync.get().pending.length > 0 ? setUnsynced(focusSync.get().pending.length) : leave())}>
									<LogOut />{tr('Sign out', 'Sair')}
								</DropdownMenuItem>
							)}
						</DropdownMenuContent>
					</DropdownMenu>
				</div>
			</div>
			{/* Unsynced focus sessions stay queued for this user; sign-out warns instead of dropping them (docs/design/security.md). */}
			<AlertDialog open={unsynced > 0} onOpenChange={(open) => !open && setUnsynced(0)}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>{tr('Some focus sessions are not saved yet', 'Algumas sessões de foco ainda não foram salvas')}</AlertDialogTitle>
						<AlertDialogDescription>{tr(`${unsynced} session(s) are waiting to reach the server. If you sign out now they stay in this browser and sync the next time you sign in here.`, `${unsynced} sessão(ões) aguardam envio ao servidor. Se você sair agora, elas ficam neste navegador e sincronizam no próximo acesso por aqui.`)}</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel onClick={() => void retryFocusSessions()}>{tr('Stay and sync', 'Ficar e sincronizar')}</AlertDialogCancel>
						<AlertDialogAction onClick={() => { setUnsynced(0); leave(); }}>{tr('Sign out anyway', 'Sair mesmo assim')}</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</header>
	);
}

/** Library search; `/` focuses it from anywhere except while typing (WCAG 2.1.4). */
function SearchBox() {
	const navigate = useNavigate();
	const [params] = useSearchParams();
	const location = useLocation();
	const input = useRef<HTMLInputElement>(null);
	const query = location.pathname === '/' ? params.get('q') ?? '' : '';

	useEffect(() => {
		const onKey = (event: KeyboardEvent) => {
			const target = event.target as HTMLElement | null;
			if (event.key !== '/' || event.ctrlKey || event.metaKey || event.altKey || target?.closest('input, textarea, select, [contenteditable="true"]')) return;
			event.preventDefault();
			input.current?.focus();
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	}, []);

	return (
		<div role="search" className="relative hidden w-44 md:block lg:w-80">
			<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
			<Input
				ref={input}
				type="search"
				value={query}
				onChange={(event) => navigate(event.target.value ? `/?q=${encodeURIComponent(event.target.value)}` : '/', { replace: location.pathname === '/' })}
				placeholder={tr('Search courses and lessons', 'Buscar cursos e aulas')}
				aria-label={tr('Search courses', 'Buscar cursos')}
				aria-keyshortcuts="/"
				className="pl-9"
			/>
		</div>
	);
}

function MobileTabs() {
	const items: { to: string; label: string; icon: typeof BookOpen; end?: boolean }[] = [...navItems(), { to: '/settings', label: tr('Account', 'Conta'), icon: CircleUser }];
	return (
		<nav aria-label={tr('Sections', 'Seções')} className="fixed inset-x-0 bottom-0 z-40 border-t bg-background pb-[env(safe-area-inset-bottom)] sm:hidden print:hidden">
			<ul className="grid grid-cols-5">
				{items.map((item) => (
					<li key={item.to}>
						<NavLink to={item.to} end={item.end} className={({ isActive }) => cn('flex min-h-14 flex-col items-center justify-center gap-0.5 text-xs text-muted-foreground', isActive && 'font-medium text-primary')}>
							<item.icon className="size-5" aria-hidden="true" />
							{item.label}
						</NavLink>
					</li>
				))}
			</ul>
		</nav>
	);
}

export function PageHeader({ title, description, actions, className }: { title: string; description?: ReactNode; actions?: ReactNode; className?: string }) {
	return (
		<div className={cn('mb-8 flex flex-wrap items-end justify-between gap-4', className)}>
			<div className="min-w-0 space-y-1">
				<h1 className="text-3xl font-semibold text-balance sm:text-4xl">{title}</h1>
				{description && <div className="text-muted-foreground">{description}</div>}
			</div>
			{actions && <div className="flex flex-wrap gap-2">{actions}</div>}
		</div>
	);
}
