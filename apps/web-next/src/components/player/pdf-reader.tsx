import { ExternalLink, Maximize2, Minus, Plus } from 'lucide-react';
import { useEffect, useRef, useState, type KeyboardEvent, type MouseEvent } from 'react';
import type { PDFDocumentProxy, RenderTask } from 'pdfjs-dist';
import { usePdfAnnotationMutations, usePdfAnnotations } from '@/app/data';
import { PdfMarks } from '@/components/player/pdf-marks';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { studySyncSupported } from '@/lib/player';
import { tr } from '@/lib/i18n';
import { clampPage, loadPdfJs, PDF_MARK_COLORS, selectionBoxes, stepZoom } from '@/lib/pdf';
import { transport, type FileEntry, type PdfAnnotationColor } from '@/lib/transport';
import { cn } from '@/lib/utils';

type Props = {
	file: FileEntry;
	courseId: string;
	/** 1-based page to open on, from `?t=` or the saved position. */
	initialPage: number | null;
	/** Called with the page whenever it changes, so reading progress can be saved. */
	onPage: (page: number, pages: number) => void;
};

/** The selected range when it sits inside the given text layer; anything else is not markable. */
function selectionIn(layer: HTMLElement | null): Range | null {
	const selection = window.getSelection();
	const range = selection?.rangeCount ? selection.getRangeAt(0) : null;
	return range && !range.collapsed && layer?.contains(range.commonAncestorContainer) ? range : null;
}

type Load = { state: 'loading' } | { state: 'ready'; doc: PDFDocumentProxy } | { state: 'error'; message: string };

/**
 * Reads a PDF from the course, read-only. Pages render to a canvas with a selectable text
 * layer on top, so text can be copied and read by screen readers. The original file is never
 * written. Without pdf.js (the desktop build) the file opens in the system viewer instead.
 */
export function PdfReader({ file, courseId, initialPage, onPage }: Props) {
	const [load, setLoad] = useState<Load>({ state: 'loading' });
	const [page, setPage] = useState(() => Math.max(1, initialPage ?? 1));
	const [zoom, setZoom] = useState<number | 'fit'>('fit');
	const [markColor, setMarkColor] = useState<PdfAnnotationColor>('yellow');
	const [hasSelection, setHasSelection] = useState(false);
	const mediaId = file.mediaId ?? null;
	const marks = usePdfAnnotations(mediaId);
	const { create, remove } = usePdfAnnotationMutations(mediaId);
	const [width, setWidth] = useState(0);
	const stage = useRef<HTMLDivElement>(null);
	const canvas = useRef<HTMLCanvasElement>(null);
	const textLayer = useRef<HTMLDivElement>(null);
	const url = transport.mediaUrl(file, courseId);

	useEffect(() => {
		if (!loadPdfJs || !url) return;
		let active = true;
		let task: { promise: Promise<PDFDocumentProxy>; destroy: () => Promise<void> } | null = null;
		void loadPdfJs()
			.then((pdfjs) => {
				task = pdfjs.getDocument({ url, useWasm: false });
				return task.promise;
			})
			.then((doc) => { if (active) setLoad({ state: 'ready', doc }); })
			.catch(() => { if (active) setLoad({ state: 'error', message: tr('This PDF could not be opened. It may be damaged or password protected.', 'Não foi possível abrir este PDF. Ele pode estar danificado ou protegido por senha.') }); });
		return () => {
			active = false;
			void task?.destroy();
		};
	}, [url]);

	const pages = load.state === 'ready' ? load.doc.numPages : 0;

	// Mark buttons enable only while a selection sits in this page's text.
	useEffect(() => {
		const update = () => setHasSelection(Boolean(selectionIn(textLayer.current)));
		document.addEventListener('selectionchange', update);
		return () => document.removeEventListener('selectionchange', update);
	}, []);
	const current = pages ? clampPage(page, pages) : page;

	useEffect(() => {
		const element = stage.current;
		if (!element || typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(([entry]) => setWidth(Math.floor(entry.contentRect.width)));
		observer.observe(element);
		return () => observer.disconnect();
	}, []);

	useEffect(() => {
		if (pages) onPage(current, pages);
	}, [current, pages]); // eslint-disable-line react-hooks/exhaustive-deps -- report the page, not the callback identity

	useEffect(() => {
		if (load.state !== 'ready' || !canvas.current || !textLayer.current || width === 0) return;
		const { doc } = load;
		const target = canvas.current;
		const textTarget = textLayer.current;
		let render: RenderTask | null = null;
		let cancelled = false;
		void (async () => {
			const [pdfjs, pageProxy] = await Promise.all([loadPdfJs!(), doc.getPage(current)]);
			if (cancelled) return;
			const base = pageProxy.getViewport({ scale: 1 });
			const scale = zoom === 'fit' ? Math.max(0.25, width / base.width) : zoom;
			const viewport = pageProxy.getViewport({ scale });
			const ratio = window.devicePixelRatio || 1;
			target.width = Math.floor(viewport.width * ratio);
			target.height = Math.floor(viewport.height * ratio);
			target.style.width = `${Math.floor(viewport.width)}px`;
			target.style.height = `${Math.floor(viewport.height)}px`;
			textTarget.replaceChildren();
			textTarget.style.width = target.style.width;
			textTarget.style.height = target.style.height;
			textTarget.style.setProperty('--total-scale-factor', String(viewport.scale));
			render = pageProxy.render({ canvas: target, viewport, transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0] });
			await render.promise;
			if (cancelled) return;
			await new pdfjs.TextLayer({ textContentSource: pageProxy.streamTextContent(), container: textTarget, viewport }).render();
		})().catch(() => undefined); // a cancelled render rejects; a real failure leaves the page blank and the toolbar usable
		return () => {
			cancelled = true;
			render?.cancel();
		};
	}, [load, current, zoom, width]);

	if (!loadPdfJs) return <ViewerFallback file={file} />;

	const mark = (kind: 'highlight' | 'underline') => {
		const range = selectionIn(textLayer.current);
		if (!range || !canvas.current || !mediaId) return;
		const rects = selectionBoxes(range.getClientRects(), canvas.current.getBoundingClientRect());
		const quote = range.toString().trim().slice(0, 4000);
		if (rects.length > 0) create.mutate({ page: current, kind, color: markColor, rects, quote });
		window.getSelection()?.removeAllRanges();
	};

	const go = (next: number) => { if (pages) setPage(clampPage(next, pages)); };
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if ((event.target as HTMLElement).closest('input, select, textarea')) return;
		const actions: Record<string, () => void> = {
			ArrowLeft: () => go(current - 1), PageUp: () => go(current - 1), ArrowRight: () => go(current + 1), PageDown: () => go(current + 1),
			Home: () => go(1), End: () => go(pages), '+': () => setZoom(stepZoom(zoom, 1)), '-': () => setZoom(stepZoom(zoom, -1))
		};
		const action = actions[event.key];
		if (action) { event.preventDefault(); action(); }
	};

	return (
		<div className="space-y-3">
			<div role="toolbar" aria-label={tr('PDF controls', 'Controles do PDF')} className="flex flex-wrap items-center gap-2 rounded-xl border bg-card p-2">
				<Button variant="outline" size="sm" disabled={current <= 1} onClick={() => go(current - 1)}>{tr('Previous page', 'Página anterior')}</Button>
				<Label htmlFor="pdf-page" className="sr-only">{tr('Page', 'Página')}</Label>
				<Input id="pdf-page" type="number" inputMode="numeric" min={1} max={pages || undefined} value={current} disabled={!pages} onChange={(event) => go(Number(event.target.value))} className="h-9 w-16 text-center tabular-nums" />
				<span className="text-sm text-muted-foreground tabular-nums">{tr(`of ${pages || '…'}`, `de ${pages || '…'}`)}</span>
				<Button variant="outline" size="sm" disabled={!pages || current >= pages} onClick={() => go(current + 1)}>{tr('Next page', 'Próxima página')}</Button>
				<div className="ml-auto flex items-center gap-1">
					<Button variant="ghost" size="icon" aria-label={tr('Zoom out', 'Diminuir zoom')} onClick={() => setZoom(stepZoom(zoom, -1))}><Minus /></Button>
					<span className="min-w-12 text-center text-sm tabular-nums" aria-live="polite">{zoom === 'fit' ? tr('Fit', 'Ajustar') : `${Math.round(zoom * 100)}%`}</span>
					<Button variant="ghost" size="icon" aria-label={tr('Zoom in', 'Aumentar zoom')} onClick={() => setZoom(stepZoom(zoom, 1))}><Plus /></Button>
					<Button variant="ghost" size="icon" aria-label={tr('Fit to width', 'Ajustar à largura')} aria-pressed={zoom === 'fit'} onClick={() => setZoom('fit')}><Maximize2 /></Button>
				</div>
			</div>

			{studySyncSupported && mediaId && load.state === 'ready' && (
				<div role="toolbar" aria-label={tr('Mark the selected text', 'Marcar o texto selecionado')} className="flex flex-wrap items-center gap-2">
					<Label htmlFor="pdf-mark-color" className="text-sm">{tr('Color', 'Cor')}</Label>
					<select id="pdf-mark-color" value={markColor} onChange={(event) => setMarkColor(event.target.value as PdfAnnotationColor)} className="h-9 rounded-lg border border-input bg-background px-2 text-sm">
						<option value="yellow">{tr('Yellow', 'Amarelo')}</option>
						<option value="green">{tr('Green', 'Verde')}</option>
						<option value="blue">{tr('Blue', 'Azul')}</option>
						<option value="pink">{tr('Pink', 'Rosa')}</option>
					</select>
					{/* Mouse-down is cancelled so pressing a mark button keeps the text selected. */}
					<Button variant="outline" size="sm" disabled={!hasSelection || create.isPending} onMouseDown={(event: MouseEvent) => event.preventDefault()} onClick={() => mark('highlight')}>{tr('Highlight', 'Destacar')}</Button>
					<Button variant="outline" size="sm" disabled={!hasSelection || create.isPending} onMouseDown={(event: MouseEvent) => event.preventDefault()} onClick={() => mark('underline')}>{tr('Underline', 'Sublinhar')}</Button>
					{(create.error || remove.error) && <p role="alert" className="text-sm text-destructive">{(create.error ?? remove.error)?.message}</p>}
				</div>
			)}

			{load.state === 'error' ? (
				<p role="alert" className="rounded-xl border border-destructive/50 p-4 text-destructive">{load.message}</p>
			) : (
				// The group takes focus so single-key shortcuts only act on the reader (WCAG 2.1.4).
				// eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex, jsx-a11y/no-noninteractive-element-interactions
				<div role="group" tabIndex={0} onKeyDown={onKeyDown} aria-label={tr('PDF page', 'Página do PDF')} aria-keyshortcuts="ArrowLeft ArrowRight PageUp PageDown Home End + -" className="overflow-auto rounded-xl border bg-muted p-2 focus-visible:ring-3 focus-visible:ring-ring sm:p-4">
					<div ref={stage} className="mx-auto w-full max-w-4xl">
						{load.state === 'loading' && <p role="status" className="p-8 text-center text-muted-foreground">{tr('Opening the PDF…', 'Abrindo o PDF…')}</p>}
						<div className="relative mx-auto w-fit bg-white shadow-sm" hidden={load.state !== 'ready'}>
							<canvas ref={canvas} role="img" aria-label={tr(`Page ${current} of ${pages}`, `Página ${current} de ${pages}`)} className="block" />
							{(marks.data ?? []).filter((item) => item.page === current).flatMap((item) => item.rects.map((rect, index) => (
								<span key={`${item.annotationId}-${index}`} aria-hidden="true" className={cn('pointer-events-none absolute', item.kind === 'highlight' ? 'mix-blend-multiply' : 'border-b-2')}
									style={{ left: `${rect.x * 100}%`, top: `${rect.y * 100}%`, width: `${rect.width * 100}%`, height: `${rect.height * 100}%`, ...(item.kind === 'highlight' ? { backgroundColor: PDF_MARK_COLORS[item.color] } : { borderColor: PDF_MARK_COLORS[item.color] }) }} />
							)))}
							<div ref={textLayer} className="pdf-text-layer" />
						</div>
					</div>
				</div>
			)}
			<p role="status" aria-live="polite" className="sr-only">{pages ? tr(`Page ${current} of ${pages}`, `Página ${current} de ${pages}`) : ''}</p>
			{studySyncSupported && mediaId && (
				<section aria-labelledby="pdf-marks-heading" className="space-y-2">
					<h2 id="pdf-marks-heading" className="font-semibold">{tr('Marks in this document', 'Marcações neste documento')}</h2>
					<PdfMarks items={marks.data ?? []} onGo={go} onDelete={(annotationId) => remove.mutate(annotationId)} pending={remove.isPending} />
				</section>
			)}
		</div>
	);
}

/** Desktop build: pdf.js is not bundled there, so hand the file to the system viewer. */
function ViewerFallback({ file }: { file: FileEntry }) {
	const open = transport.openInSystemPlayer?.bind(transport);
	return (
		<div role="status" className="space-y-3 rounded-xl border bg-card p-6">
			<p className="font-medium">{tr('This PDF opens in your system viewer.', 'Este PDF abre no seu leitor de PDF do sistema.')}</p>
			{open && <Button onClick={() => void open(file)}><ExternalLink />{tr('Open PDF', 'Abrir PDF')}</Button>}
		</div>
	);
}
