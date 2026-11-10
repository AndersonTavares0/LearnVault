import { NavLink, Outlet } from 'react-router';
import { useLocale } from '@/hooks/use-locale';
import { tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';

/**
 * Phase 1 shell: landmarks, skip link and the global destinations. The full topbar,
 * search and mobile tab bar land in Phase 2 (#336).
 */
export function AppShell() {
	useLocale();
	const links = [
		{ to: '/', label: tr('My learning', 'Meu aprendizado'), end: true },
		{ to: '/progress', label: tr('Progress', 'Progresso') },
		{ to: '/notes', label: tr('Notes', 'Notas') },
		{ to: '/settings', label: tr('Settings', 'Ajustes') }
	];
	return (
		<>
			<a href="#main" onClick={(event) => { event.preventDefault(); document.getElementById('main')?.focus(); }} className="sr-only focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:rounded-md focus:bg-background focus:px-4 focus:py-3">
				{tr('Skip to content', 'Pular para o conteúdo')}
			</a>
			<header className="border-b">
				<nav aria-label={tr('Main', 'Principal')} className="mx-auto flex max-w-6xl flex-wrap items-center gap-1 px-4">
					<span className="mr-4 font-semibold">LearnVault</span>
					{links.map((link) => (
						<NavLink key={link.to} to={link.to} end={link.end} className={({ isActive }) => cn('inline-flex min-h-11 items-center rounded-md px-3 text-sm hover:bg-muted', isActive && 'font-medium text-primary')}>
							{link.label}
						</NavLink>
					))}
				</nav>
			</header>
			<main id="main" tabIndex={-1} className="mx-auto max-w-6xl px-4 py-8 outline-none">
				<Outlet />
			</main>
		</>
	);
}
