import type { PDFDocumentProxy, RenderTask } from 'pdfjs-dist';
import { useEffect, useRef, useState } from 'react';
import { tr } from '@/lib/i18n';
import { activeOutlineIndex, type OutlineItem } from '@/lib/pdf';
import { cn } from '@/lib/utils';

const THUMB_WIDTH = 96;

type Props = {
	doc: PDFDocumentProxy;
	page: number;
	pages: number;
	/** The table of contents, or `null` while it is being read. */
	outline: OutlineItem[] | null;
	onGo: (page: number) => void;
};

/**
 * Ways to move through a document besides scrolling: its table of contents, with the heading of the
 * current page marked, and a strip of page thumbnails. A document without a table of contents opens
 * on the page list.
 */
export function PdfContents({ doc, page, pages, outline, onGo }: Props) {
	const hasOutline = (outline?.length ?? 0) > 0;
	const [tab, setTab] = useState<'outline' | 'pages'>('outline');
	const shown = hasOutline ? tab : 'pages';
	const active = hasOutline ? activeOutlineIndex(outline!, page) : -1;
	const strip = useRef<HTMLOListElement>(null);

	// The strip follows the current page by moving itself, never the page around it.
	useEffect(() => {
		const list = strip.current;
		const current = list?.querySelector<HTMLElement>('[aria-current="page"]');
		if (list && current) list.scrollLeft = current.offsetLeft - (list.clientWidth - current.offsetWidth) / 2;
	}, [page, shown]);

	return (
		<section aria-label={tr('Contents', 'Conteúdo')} className="space-y-3 rounded-2xl border bg-card p-3">
			{hasOutline && (
				<div role="group" aria-label={tr('Show', 'Mostrar')} className="flex w-fit gap-1 rounded-full bg-muted p-1">
					{([['outline', tr('Outline', 'Sumário')], ['pages', tr('Pages', 'Páginas')]] as const).map(([value, label]) => (
						<button key={value} type="button" aria-pressed={shown === value} onClick={() => setTab(value)}
							className={cn('min-h-11 rounded-full px-4 text-sm font-medium focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', shown === value ? 'bg-background shadow-sm' : 'text-muted-foreground')}>{label}</button>
					))}
				</div>
			)}
			{outline === null && <p role="status" className="text-sm text-muted-foreground">{tr('Reading the contents…', 'Lendo o conteúdo…')}</p>}
			{outline !== null && !hasOutline && <p className="text-sm text-muted-foreground">{tr('This document has no table of contents, so here are its pages.', 'Este documento não tem sumário, então aqui estão as páginas dele.')}</p>}
			{shown === 'outline' && hasOutline ? (
				<ol className="max-h-72 space-y-0.5 overflow-y-auto">
					{outline!.map((item, index) => (
						<li key={`${index}-${item.title}`} style={{ paddingInlineStart: `${item.depth * 0.9}rem` }}>
							{item.page === null ? (
								<span className="block px-3 py-2 text-sm text-muted-foreground">{item.title}</span>
							) : (
								<button type="button" aria-current={index === active ? 'location' : undefined} onClick={() => onGo(item.page!)}
									className={cn('flex min-h-11 w-full items-center justify-between gap-3 rounded-xl px-3 text-left text-sm hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', index === active && 'bg-accent font-semibold')}>
									<span className="min-w-0 break-words">{item.title}</span>
									<span className="shrink-0 tabular-nums text-muted-foreground">{item.page}</span>
								</button>
							)}
						</li>
					))}
				</ol>
			) : outline !== null && (
				<ol ref={strip} aria-label={tr('Pages', 'Páginas')} className="flex gap-2 overflow-x-auto pb-2">
					{Array.from({ length: pages }, (_, index) => index + 1).map((number) => (
						<li key={number} className="shrink-0">
							<Thumbnail doc={doc} page={number} current={number === page} onGo={onGo} />
						</li>
					))}
				</ol>
			)}
		</section>
	);
}

/** One page, drawn small only once it scrolls into view, so a long document costs nothing until it is looked at. */
function Thumbnail({ doc, page, current, onGo }: { doc: PDFDocumentProxy; page: number; current: boolean; onGo: (page: number) => void }) {
	const holder = useRef<HTMLButtonElement>(null);
	const canvas = useRef<HTMLCanvasElement>(null);
	const [visible, setVisible] = useState(typeof IntersectionObserver === 'undefined');

	useEffect(() => {
		const element = holder.current;
		if (!element || typeof IntersectionObserver === 'undefined') return;
		const observer = new IntersectionObserver((entries) => { if (entries.some((entry) => entry.isIntersecting)) { setVisible(true); observer.disconnect(); } }, { rootMargin: '200px' });
		observer.observe(element);
		return () => observer.disconnect();
	}, []);

	useEffect(() => {
		if (!visible) return;
		let cancelled = false;
		let task: RenderTask | null = null;
		void (async () => {
			const proxy = await doc.getPage(page);
			const viewport = proxy.getViewport({ scale: THUMB_WIDTH / proxy.getViewport({ scale: 1 }).width });
			const target = canvas.current;
			if (cancelled || !target) return;
			target.width = Math.floor(viewport.width);
			target.height = Math.floor(viewport.height);
			task = proxy.render({ canvas: target, viewport });
			await task.promise;
		})().catch(() => undefined); // a cancelled render rejects; a thumbnail that fails stays blank
		return () => {
			cancelled = true;
			task?.cancel();
		};
	}, [visible, doc, page]);

	return (
		<button ref={holder} type="button" aria-label={tr(`Go to page ${page}`, `Ir para a página ${page}`)} aria-current={current ? 'page' : undefined} onClick={() => onGo(page)}
			className={cn('flex min-h-11 flex-col items-center gap-1 rounded-xl border p-1.5 text-xs hover:bg-accent focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none', current && 'border-primary ring-2 ring-primary')}>
			<canvas ref={canvas} aria-hidden="true" className="block h-auto bg-white" style={{ width: THUMB_WIDTH, minHeight: 40 }} />
			<span className="tabular-nums">{page}</span>
		</button>
	);
}
