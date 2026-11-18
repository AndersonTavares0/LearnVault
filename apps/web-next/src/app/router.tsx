import { createHashRouter, Navigate, useLocation, type RouteObject } from 'react-router';
import type { ReactNode } from 'react';
import { useLocale } from '@/hooks/use-locale';
import { usePageTitle } from '@/hooks/use-page-title';
import { tr } from '@/lib/i18n';
import { href, locationFromHash } from '@/lib/router';
import { defaultRoute } from '@/lib/session';
import { transportKind } from '@/lib/transport';
import { AppShell } from './shell';
import { RouteError } from './route-error';
import { HomePage } from '@/pages/home';
import { LoginPage } from '@/pages/login';
import { CourseRoute } from '@/pages/course';
import { ProgressPage } from '@/pages/progress';
import { NotesPage } from '@/pages/notes';
import { HistoryPage } from '@/pages/history';
import { ReviewPage } from '@/pages/review';
import { SettingsPage } from '@/pages/settings';

/** Strips the leading `#` so a legacy hash can be used as a router path. */
const toPath = (hash: string) => hash.replace(/^#/, '');

/**
 * Old links must keep working: `#/player/<course>/<lesson>` becomes the course route,
 * `#/statistics` and `#/focus` become `#/progress`. A loose file (`#/player/<path>`)
 * opens the library until the player lands (#338).
 */
function LegacyRedirect() {
	const { pathname, search } = useLocation();
	const legacy = locationFromHash(`#${pathname}${search}`, defaultRoute(transportKind));
	if (legacy.route === 'statistics') return <Navigate to="/progress" replace />;
	if (legacy.route === 'player' && legacy.courseId && legacy.lessonPath) {
		return <Navigate to={toPath(href('course', legacy.courseId, legacy.lessonPath, legacy.positionSeconds))} replace />;
	}
	return <Navigate to={transportKind === 'desktop' ? '/' : '/login'} replace />;
}

function TitledPage({ en, ptBr, children }: { en: string; ptBr: string; children: ReactNode }) {
	useLocale();
	usePageTitle(tr(en, ptBr));
	return children;
}

export const routes: RouteObject[] = [
	...(transportKind === 'http' ? [{ path: '/login', element: <TitledPage en="Sign in" ptBr="Entrar"><LoginPage /></TitledPage> }] : []),
	{
		element: <AppShell />,
		errorElement: <RouteError />,
		children: [
			{ index: true, element: <TitledPage en="My learning" ptBr="Meu aprendizado"><HomePage /></TitledPage> },
			{ path: 'home', element: <Navigate to="/" replace /> },
			{ path: 'course/:courseId/*', element: <CourseRoute /> },
			{ path: 'progress', element: <TitledPage en="Your progress" ptBr="Seu progresso"><ProgressPage /></TitledPage> },
			{ path: 'review', element: <TitledPage en="Review" ptBr="Revisão"><ReviewPage /></TitledPage> },
			{ path: 'notes', element: <TitledPage en="Notes" ptBr="Notas"><NotesPage /></TitledPage> },
			{ path: 'history', element: <TitledPage en="Watch history" ptBr="Histórico de aulas"><HistoryPage /></TitledPage> },
			{ path: 'settings', element: <TitledPage en="Settings" ptBr="Ajustes"><SettingsPage /></TitledPage> }
		]
	},
	{ path: '*', element: <LegacyRedirect /> }
];

export const createRouter = () => createHashRouter(routes);
