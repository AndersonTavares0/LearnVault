import { useMutation, useQuery } from '@tanstack/react-query';
import { Button } from '@/components/ui/button';
import { tr } from '@/lib/i18n';
import { transport, transportKind } from '@/lib/transport';
import { updateMessage, updateOutcome, updateReleaseUrl } from '@/lib/update';

/** Version, update check, desktop install and (server) update history. Admin-only on the server. */
export function UpdateSettings() {
	const status = useQuery({ queryKey: ['update'], queryFn: () => transport.checkUpdate(false), retry: false });
	const history = useQuery({ queryKey: ['update-history'], enabled: transportKind === 'http', queryFn: () => transport.updateHistory(), retry: false });
	const check = useMutation({ mutationFn: () => transport.checkUpdate(true), onSuccess: () => status.refetch() });
	const install = useMutation({ mutationFn: async () => { if (!transport.downloadUpdate || !(await transport.downloadUpdate())) throw new Error(tr('No update available to install right now.', 'Nenhuma atualização disponível para instalar agora.')); await transport.restartToUpdate?.(); } });
	const data = check.data ?? status.data;
	const outcome = data ? updateOutcome(data) : null;
	const releaseUrl = data ? updateReleaseUrl(data) : null;

	return (
		<div className="space-y-4">
			{data && <p className="text-sm text-muted-foreground">{tr(`Installed version: ${data.current}`, `Versão instalada: ${data.current}`)}</p>}
			<div className="flex flex-wrap items-center gap-2">
				<Button variant="outline" disabled={check.isPending} onClick={() => check.mutate()}>{check.isPending ? tr('Checking…', 'Verificando…') : tr('Check for updates', 'Verificar atualização')}</Button>
				{outcome === 'available' && transportKind === 'desktop' && (
					<Button disabled={install.isPending} onClick={() => install.mutate()}>{install.isPending ? tr('Downloading…', 'Baixando…') : tr('Update and restart', 'Atualizar e reiniciar')}</Button>
				)}
			</div>
			{data && <p role="status" className="text-sm">{updateMessage(data)} {releaseUrl && <a href={releaseUrl} target="_blank" rel="noopener noreferrer" className="font-medium text-primary underline-offset-4 hover:underline">{tr('Release notes', 'Notas da versão')}</a>}</p>}
			{(check.error || install.error) && <p role="alert" className="text-sm text-destructive">{(check.error ?? install.error)?.message}</p>}
			{outcome === 'available' && data?.changelog && (
				<details className="rounded-lg border p-3">
					<summary className="min-h-11 cursor-pointer content-center font-medium">{tr('What’s new', 'Novidades')}</summary>
					{/* Text only: the changelog comes from the network and is never rendered as HTML. */}
					<pre className="mt-2 text-sm whitespace-pre-wrap">{data.changelog}</pre>
				</details>
			)}
			{(history.data?.length ?? 0) > 0 && (
				<details className="rounded-lg border p-3">
					<summary className="min-h-11 cursor-pointer content-center font-medium">{tr(`Update history (${history.data!.length})`, `Histórico de atualizações (${history.data!.length})`)}</summary>
					<ul className="mt-2 space-y-2 text-sm">
						{history.data!.map((entry, index) => (
							<li key={`${entry.timestamp}-${index}`}>
								<span className="font-medium">{entry.status === 'success' ? tr('Updated', 'Atualizado') : entry.status === 'rollback' ? tr('Rolled back', 'Revertido') : entry.status}</span>
								{' '}{entry.from} → {entry.to} · <time dateTime={entry.timestamp}>{entry.timestamp}</time>
								{entry.message && <p className="text-muted-foreground">{entry.message}</p>}
							</li>
						))}
					</ul>
				</details>
			)}
		</div>
	);
}
