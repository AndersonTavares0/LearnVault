import { formatCount } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import type { DiscoveredCourse, Transport } from '@/lib/transport';

/** What "Refresh courses" did beyond the local courses. */
export type RefreshOutcome = {
	sources: number;
	failed: { name: string; message: string }[];
	discovered: DiscoveredCourse[];
};

/** The server refuses to rescan when no local course is registered; the transport names that error. */
export const isNoCourses = (error: unknown): boolean => error instanceof Error && error.name === 'NoCoursesError';

/**
 * Refreshes the whole library: the local courses first, then every remote source one by one. Each
 * step is its own short request, so a slow source never holds the others, and a source that cannot
 * be read is reported without stopping the rest. A server with only remote sources has no local
 * course to rescan, which is not a failure.
 */
export async function refreshLibrary(
	transport: Pick<Transport, 'rescan' | 'listSources' | 'scanSource'>,
	onProgress: (message: string) => void = () => {}
): Promise<RefreshOutcome> {
	// Only a server administrator can list sources; the desktop has none. Anything else means "no sources".
	const sources = ((await transport.listSources?.().catch(() => [])) ?? []).filter((source) => !source.credentialError);
	let discovered: DiscoveredCourse[] = [];
	try {
		const outcome = await transport.rescan();
		if (outcome && Array.isArray(outcome.discovered)) discovered = outcome.discovered;
	} catch (error) {
		if (!(sources.length > 0 && isNoCourses(error))) throw error;
	}
	const failed: RefreshOutcome['failed'] = [];
	for (const source of sources) {
		onProgress(tr(`Reading “${source.name}”…`, `Lendo “${source.name}”…`));
		try {
			await transport.scanSource!(source.id);
		} catch (error) {
			failed.push({ name: source.name, message: error instanceof Error ? error.message : '' });
		}
	}
	return { sources: sources.length, failed, discovered };
}

export function refreshMessage(outcome: Omit<RefreshOutcome, 'discovered'> & { discovered?: DiscoveredCourse[] }): string {
	if (outcome.failed.length > 0) {
		const names = outcome.failed.map((source) => `“${source.name}”`).join(', ');
		const one = outcome.failed.length === 1;
		return tr(
			`Catalog updated, but ${one ? 'this source' : 'these sources'} could not be read: ${names}. ${outcome.failed[0].message} Unavailable courses and your study progress are kept.`,
			`Catálogo atualizado, mas ${one ? 'esta fonte não pôde' : 'estas fontes não puderam'} ser lida${one ? '' : 's'}: ${names}. ${outcome.failed[0].message} Cursos indisponíveis e seu progresso de estudo são preservados.`
		).trim();
	}
	if (outcome.sources === 0) return tr('Catalog updated.', 'Catálogo atualizado.');
	return tr(`Catalog updated, including ${formatCount(outcome.sources, 'remote source', 'remote sources')}.`, `Catálogo atualizado, incluindo ${formatCount(outcome.sources, 'fonte remota', 'fontes remotas')}.`);
}
