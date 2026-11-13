import { BookOpen, ChartNoAxesColumn, History, Layers, CircleUser, LogOut, Moon, NotebookPen, Search, Settings, Sun, type LucideIcon } from 'lucide-react';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useState, type ReactNode } from 'react';
import { Link, Navigate, NavLink, Outlet, useLocation, useNavigate } from 'react-router';
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

	if (session.isPending) return <p role="status" className="p-8 text-muted-foreground">{tr('Loading…', 'Carregando…')}</p>;
	if (!session.data?.user) return <Navigate to="/login" replace state={{ from: `${location.pathname}${location.search}` }} />;

	const user = session.data.user;
	return (
		<div className="min-h-dvh pb-16 md:pb-0">
			<SkipLink />
			<DesktopTitlebar />
			<header className="border-b print:hidden">
				<div className="mx-auto flex h-[4.5rem] max-w-[88rem] items-center gap-x-8 px-5 sm:px-10">
					<Link to="/" className="rounded-md font-heading text-2xl font-bold tracking-tight" aria-label={tr('LearnVault home', 'Início do LearnVault')}>LearnVault</Link>
					<nav aria-label={tr('Main', 'Principal')} className="hidden flex-1 justify-center gap-1 md:flex">
						{navItems().map((item) => (
							<NavLink key={item.to} to={item.to} end={item.end} className={({ isActive }) => cn('inline-flex min-h-11 items-center rounded-xl px-4 text-[0.95rem] font-medium whitespace-nowrap text-muted-foreground hover:bg-brand-tint hover:text-foreground', isActive && 'bg-panel font-semibold text-foreground')}>
								{item.label}
							</NavLink>
						))}
					</nav>
					<div className="ml-auto flex items-center gap-1 md:ml-0">
						<GlobalSearch />
						<FocusMenu />
						<ThemeToggle />
						<AccountMenu username={user.username} userId={user.userId} />
					</div>
				</div>
			</header>
			<UpdateNotice isAdmin={Boolean(user.isAdmin)} />
			<IndexingIndicator isAdmin={Boolean(user.isAdmin)} />
			<main id="main" tabIndex={-1} className="mx-auto w-full max-w-[88rem] px-5 py-8 outline-none sm:px-10 sm:py-10">
				<Outlet />
			</main>
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
					<CircleUser />{wide && <span className="truncate">{username}</span>}
				</DropdownMenuTrigger>
				<DropdownMenuContent align="end" className="w-56">
					{/* Base UI requires group labels inside a group. */}
					<DropdownMenuGroup>
						<DropdownMenuLabel className="truncate">{username}</DropdownMenuLabel>
					</DropdownMenuGroup>
					<DropdownMenuSeparator />
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
		<nav aria-label={tr('Sections', 'Seções')} className="fixed inset-x-0 bottom-0 z-40 border-t bg-background pb-[env(safe-area-inset-bottom)] md:hidden print:hidden">
			<ul className="grid grid-cols-5">
				{navItems().map((item) => (
					<li key={item.to}>
						<NavLink to={item.to} end={item.end} className={({ isActive }) => cn('flex min-h-14 flex-col items-center justify-center gap-0.5 text-xs text-muted-foreground', isActive && 'font-medium text-primary')}>
							<item.icon className="size-5" aria-hidden="true" />
							{item.label}
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
export function PageHeader({ title, description, actions, icon: Icon, className }: { title: string; description?: ReactNode; actions?: ReactNode; icon?: LucideIcon; className?: string }) {
	const cut = title.lastIndexOf(' ') + 1;
	return (
		<div className={cn('mb-8 flex flex-wrap items-end justify-between gap-4', className)}>
			<div className="flex min-w-0 items-center gap-4">
				{Icon && <span className="hidden size-14 shrink-0 place-items-center rounded-2xl bg-brand-soft text-primary sm:grid" aria-hidden="true"><Icon className="size-7" /></span>}
				<div className="min-w-0 space-y-1">
					<h1 className="font-heading text-3xl font-semibold text-balance sm:text-4xl">{cut > 0 ? <>{title.slice(0, cut)}<em className="text-primary">{title.slice(cut)}</em></> : title}</h1>
					{description && <div className="text-muted-foreground">{description}</div>}
				</div>
			</div>
			{actions && <div className="flex flex-wrap gap-2">{actions}</div>}
		</div>
	);
}
