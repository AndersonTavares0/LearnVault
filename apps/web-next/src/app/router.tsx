import { createHashRouter, Navigate, useLocation, type RouteObject } from 'react-router';
import { href, locationFromHash } from '@/lib/router';
import { defaultRoute } from '@/lib/session';
import { transportKind } from '@/lib/transport';
import { AppShell } from './shell';
import { HomePage } from '@/pages/home';
import { LoginPage } from '@/pages/login';
import { CourseRoute } from '@/pages/course';
import { NotesPage, ProgressPage, SettingsPage } from './pages';

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

export const routes: RouteObject[] = [
	...(transportKind === 'http' ? [{ path: '/login', element: <LoginPage /> }] : []),
	{
		element: <AppShell />,
		children: [
			{ index: true, element: <HomePage /> },
			{ path: 'home', element: <Navigate to="/" replace /> },
			{ path: 'course/:courseId/*', element: <CourseRoute /> },
			{ path: 'progress', element: <ProgressPage /> },
			{ path: 'notes', element: <NotesPage /> },
			{ path: 'settings', element: <SettingsPage /> }
		]
	},
	{ path: '*', element: <LegacyRedirect /> }
];

export const createRouter = () => createHashRouter(routes);
