import { tr } from './i18n';
import type { Route } from './router';
import type { TransportKind, User } from './transport';

/** Initial state resolved when the application mounts. */
export type SessionBootstrap = {
	/** Current session; the desktop receives a local identity with no login. */
	user: User | null;
	/** Readable message of a failure reading the session, or an empty string. */
	error: string;
};

/**
 * Initial route of the build when the hash does not name a navigable route.
 *
 * The server starts at the sign-in: without a session, the form is the starting point. The desktop
 * has no session or sign-in, so any sign-in or unknown address falls to the
 * home screen instead of opening a form that cannot authenticate.
 */
export function defaultRoute(kind: TransportKind): Route {
	return kind === 'desktop' ? 'home' : 'login';
}

/** Local identity, with no credentials and no data about the courses' paths. */
export const desktopLocalUser: User = {
	userId: 'local',
	username: 'Biblioteca local',
	role: 'admin',
	isAdmin: true,
	mustChangeCredentials: false
};

/**
 * Resolves the application's initial session.
 *
 * The desktop is always local: with no credentials and no network ports, it uses the
 * synthetic identity and goes straight to the application, even with no course imported (the home
 * screen invites you to import). HTTP reads the session from the server and, on a network
 * failure, keeps the sign-in form with an appropriate message instead of declaring the
 * session ended.
 */
export async function resolveSession(
	readSession: () => Promise<User | null>,
	kind: TransportKind = 'http'
): Promise<SessionBootstrap> {
	if (kind === 'desktop') return { user: desktopLocalUser, error: '' };
	try {
		return { user: await readSession(), error: '' };
	} catch {
		return { user: null, error: tr('Could not connect to the service.', 'Não foi possível conectar ao serviço.') };
	}
}
