import { useQuery, useQueryClient } from '@tanstack/react-query';
import { RefreshCw } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { keys } from '@/app/data';
import { Button } from '@/components/ui/button';
import { tr } from '@/lib/i18n';
import { transport, transportKind } from '@/lib/transport';

/**
 * Background indexing is server-only and runs on its own. While it walks the library the
 * shell shows a small live progress line; a failure stays visible until the next run, so a
 * stale catalog is never silent. Only administrators can cancel it.
 */
export function IndexingIndicator({ isAdmin }: { isAdmin: boolean }) {
	const enabled = transportKind === 'http';
	const client = useQueryClient();
	const status = useQuery({
		queryKey: ['indexing'],
		enabled,
		retry: false,
		queryFn: () => transport.indexingStatus(),
		refetchInterval: (query) => (query.state.data?.status === 'running' ? 1_500 : 60_000)
	});
	const data = status.data;

	// A finished run changed the catalog: refresh every reader once, not on every poll.
	const lastFinished = useRef<number | null | undefined>(undefined);
	useEffect(() => {
		if (!data) return;
		if (lastFinished.current !== undefined && data.finishedAt !== null && data.finishedAt !== lastFinished.current) void client.invalidateQueries({ queryKey: keys.catalog });
		lastFinished.current = data.finishedAt;
	}, [client, data]);

	if (!enabled || !data || (data.status !== 'running' && data.status !== 'error')) return null;

	const progress = data.progress;
	const message = data.status === 'running'
		? tr(
			`Indexing your library… ${progress.filesSeen} files seen, ${progress.added} added, ${progress.changed} changed, ${progress.removed} removed.`,
			`Indexando sua biblioteca… ${progress.filesSeen} arquivos vistos, ${progress.added} adicionados, ${progress.changed} alterados, ${progress.removed} removidos.`
		)
		: tr('Some courses could not be indexed. The catalog may be out of date.', 'Alguns cursos não puderam ser indexados. O catálogo pode estar desatualizado.');

	return (
		<section aria-label={tr('Library indexing', 'Indexação da biblioteca')} className="border-b bg-panel print:hidden">
			<div className="mx-auto flex max-w-[88rem] flex-wrap items-center gap-x-4 gap-y-1 px-5 py-2 sm:px-10">
				<RefreshCw className={`size-5 shrink-0 ${data.status === 'running' ? 'animate-spin text-primary motion-reduce:animate-none' : 'text-muted-foreground'}`} aria-hidden="true" />
				<p role="status" className="min-w-0 flex-1 text-sm">{message}</p>
				{isAdmin && data.status === 'running' && (
					<Button variant="ghost" size="sm" onClick={() => { void transport.cancelIndexing().then(() => status.refetch()); }}>
						{tr('Cancel indexing', 'Cancelar indexação')}
					</Button>
				)}
			</div>
		</section>
	);
}
