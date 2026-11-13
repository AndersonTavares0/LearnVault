import { tr } from './i18n';

/** Fired once per expired session so the shell can drop the cached identity. */
export const SESSION_EXPIRED_EVENT = 'learnvault:session-expired';

const WRAPPED = Symbol.for('learnvault.session-guard');

/**
 * A 401 means the session row is gone (expired, rotated or revoked). Every transport call
 * would otherwise surface its own error, so the guard converts the first 401 into one event
 * and the shell re-reads the session, which sends the visitor back to the sign-in screen.
 *
 * `window.fetch` is patched instead of the ~75 transport call sites: one seam, no per-call
 * bookkeeping, and it also covers requests issued by libraries.
 */
export function installSessionGuard(): void {
	if (typeof window === 'undefined') return;
	const original = window.fetch;
	if ((original as unknown as Record<symbol, boolean>)[WRAPPED]) return;
	const guard = async (input: RequestInfo | URL, init?: RequestInit) => {
		const response = await original(input, init);
		if (response.status === 401 && isApiRequest(input)) {
			window.dispatchEvent(new CustomEvent(SESSION_EXPIRED_EVENT));
		}
		return response;
	};
	(guard as unknown as Record<symbol, boolean>)[WRAPPED] = true;
	window.fetch = guard as typeof window.fetch;
}

function isApiRequest(input: RequestInfo | URL): boolean {
	const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
	const path = url.startsWith('http') ? new URL(url).pathname : url;
	// A rejected sign-in is a normal flow, not an expiry.
	return path.startsWith('/api/') && !path.startsWith('/api/login');
}

/** Copy shown once after the guard reports an expiry; empty on the desktop build. */
export const sessionExpiredNotice = (): string =>
	tr('Your session expired. Sign in again to continue.', 'Sua sessão expirou. Entre novamente para continuar.');
