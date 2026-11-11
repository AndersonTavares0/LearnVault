import { useQuery } from '@tanstack/react-query';
import { ArrowUpCircle, X } from 'lucide-react';
import { useState } from 'react';
import { Link } from 'react-router';
import { Button } from '@/components/ui/button';
import { tr } from '@/lib/i18n';
import { transport, transportKind } from '@/lib/transport';
import { updateOutcome, updateReleaseUrl } from '@/lib/update';
import { safeStorage } from '@/app/preferences';

const DISMISSED_KEY = 'learnvault-update-dismissed';

function dismissedVersion(): string | null {
	try {
		return safeStorage()?.getItem(DISMISSED_KEY) ?? null;
	} catch {
		return null;
	}
}

/**
 * Tells the administrator of a server that a newer version exists, on whatever page they are on.
 * It only reads the server's own check (which asks GitHub at most once an hour and can be turned
 * off), never installs anything, and stays quiet once dismissed for that version. The desktop app
 * does not look for updates by itself: it makes no network request until the person asks in Settings.
 */
export function UpdateNotice({ isAdmin }: { isAdmin: boolean }) {
	const enabled = transportKind === 'http' && isAdmin;
	const status = useQuery({ queryKey: ['update'], enabled, staleTime: 3_600_000, retry: false, queryFn: () => transport.checkUpdate(false) });
	const [dismissed, setDismissed] = useState(dismissedVersion);
	const data = status.data;
	if (!enabled || !data || updateOutcome(data) !== 'available' || !data.latest || dismissed === data.latest) return null;
	const releaseUrl = updateReleaseUrl(data);
	const dismiss = () => {
		setDismissed(data.latest);
		try {
			safeStorage()?.setItem(DISMISSED_KEY, data.latest!);
		} catch {
			// The notice just comes back next time.
		}
	};
	return (
		<section aria-label={tr('Update available', 'Atualização disponível')} className="border-b bg-panel print:hidden">
			<div className="mx-auto flex max-w-7xl flex-wrap items-center gap-x-4 gap-y-1 px-5 py-2 sm:px-10">
				<ArrowUpCircle className="size-5 shrink-0 text-primary" aria-hidden="true" />
				<p role="status" className="min-w-0 flex-1 text-sm"><span className="font-semibold">{tr(`Version ${data.latest} is available`, `A versão ${data.latest} está disponível`)}</span> <span className="text-muted-foreground">{tr(`(you have ${data.current}).`, `(você tem a ${data.current}).`)}</span></p>
				{releaseUrl && <a href={releaseUrl} target="_blank" rel="noopener noreferrer" className="inline-flex min-h-11 items-center text-sm font-medium text-primary underline-offset-4 hover:underline">{tr('What’s new', 'Novidades')}</a>}
				<Link to="/settings#updates" className="inline-flex min-h-11 items-center text-sm font-medium text-primary underline-offset-4 hover:underline">{tr('How to update', 'Como atualizar')}</Link>
				<Button variant="ghost" size="icon" onClick={dismiss} aria-label={tr('Dismiss until the next version', 'Dispensar até a próxima versão')}><X /></Button>
			</div>
		</section>
	);
}
