import { useQuery } from '@tanstack/react-query';
import { ArrowUpCircle, X } from 'lucide-react';
import { useState } from 'react';
import { Link } from 'react-router';
import { Button } from '@/components/ui/button';
import { tr } from '@/lib/i18n';
import { transport, transportKind, type Transport, type UpdateStatus } from '@/lib/transport';
import { markUpdateChecked, readUpdateCheckAt, shouldCheckForUpdate, UPDATE_CHECK_INTERVAL_MS, updateOutcome, updateReleaseUrl } from '@/lib/update';
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
 * off), never installs anything, and stays quiet once dismissed for that version.
 *
 * On the desktop it runs the app's own check on launch, at most once a day, and hands the result
 * to the non-modal restart prompt below.
 */
export function UpdateNotice({ isAdmin }: { isAdmin: boolean }) {
	const desktop = transportKind === 'desktop';
	const enabled = desktop || (transportKind === 'http' && isAdmin);
	const status = useQuery({
		queryKey: desktop ? ['update', 'automatic'] : ['update'],
		enabled,
		staleTime: desktop ? UPDATE_CHECK_INTERVAL_MS : 3_600_000,
		retry: false,
		queryFn: async (): Promise<UpdateStatus | null> => {
			if (!desktop) return transport.checkUpdate(false);
			// The daily gate lives here, not in render: a launch inside the day since the last
			// check resolves to "nothing to show" and makes no request at all.
			const now = Date.now();
			if (!shouldCheckForUpdate(readUpdateCheckAt(safeStorage()), now)) return null;
			// A failed check still sets the stamp: offline work must not retry in a loop.
			markUpdateChecked(safeStorage(), now);
			return transport.checkUpdate(false);
		}
	});
	const [dismissed, setDismissed] = useState(dismissedVersion);
	const data = status.data;
	if (desktop) return data?.available ? <DesktopUpdatePrompt status={data} /> : null;
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

type UpdateClient = Pick<Transport, 'downloadUpdate' | 'restartToUpdate'>;
type PromptState = 'available' | 'downloading' | 'ready' | 'error';

/**
 * Desktop updater banner: download in the background, then restart on an explicit click.
 *
 * It never reloads the page, never restarts by itself and never blocks playback: the player is a
 * route under this banner, so an open lesson keeps playing until the person chooses to restart.
 */
export function DesktopUpdatePrompt({ status, client = transport }: { status: UpdateStatus; client?: UpdateClient }) {
	const [state, setState] = useState<PromptState>('available');

	const download = async () => {
		setState('downloading');
		try {
			const downloaded = client.downloadUpdate ? await client.downloadUpdate() : false;
			setState(downloaded ? 'ready' : 'error');
		} catch {
			setState('error');
		}
	};
	const restart = async () => {
		try {
			await client.restartToUpdate?.();
		} catch {
			setState('error');
		}
	};

	const version = status.latest ?? tr('unknown', 'desconhecida');
	return (
		<section aria-label={tr('Update available', 'Atualização disponível')} className="border-b bg-panel print:hidden">
			<div className="mx-auto flex max-w-7xl flex-wrap items-center gap-x-4 gap-y-1 px-5 py-2 sm:px-10">
				<ArrowUpCircle className="size-5 shrink-0 text-primary" aria-hidden="true" />
				<p role={state === 'error' ? 'alert' : 'status'} className="min-w-0 flex-1 text-sm">
					{state === 'error'
						? <span className="text-destructive">{tr('Could not prepare the update. Try again.', 'Não foi possível preparar a atualização. Tente novamente.')}</span>
						: state === 'downloading'
							? <span className="font-semibold">{tr(`Downloading version ${version}…`, `Baixando a versão ${version}…`)}</span>
							: state === 'ready'
								? <span className="font-semibold">{tr(`Version ${version} is ready`, `A versão ${version} está pronta`)} <span className="font-normal text-muted-foreground">{tr('— restart to finish.', '— reinicie para concluir.')}</span></span>
								: <span className="font-semibold">{tr(`Version ${version} is available`, `A versão ${version} está disponível`)} <span className="font-normal text-muted-foreground">{tr(`(you have ${status.current}).`, `(você tem a ${status.current}).`)}</span></span>}
				</p>
				{state === 'available' && <Button variant="outline" onClick={download}>{tr('Download update', 'Baixar atualização')}</Button>}
				{state === 'downloading' && <Button variant="outline" disabled>{tr('Downloading…', 'Baixando…')}</Button>}
				{state === 'ready' && <Button onClick={restart}>{tr('Restart to update', 'Reiniciar para atualizar')}</Button>}
				{state === 'error' && <Button variant="outline" onClick={download}>{tr('Try again', 'Tentar novamente')}</Button>}
			</div>
			{status.changelog && state !== 'error' && (
				<details className="mx-auto max-w-7xl px-5 pb-2 sm:px-10">
					<summary className="min-h-11 cursor-pointer content-center text-sm font-medium">{tr('What’s new', 'Novidades')}</summary>
					{/* Text only: the release notes come from the network and are never rendered as HTML. */}
					<pre className="mt-2 text-sm whitespace-pre-wrap">{status.changelog}</pre>
				</details>
			)}
		</section>
	);
}
