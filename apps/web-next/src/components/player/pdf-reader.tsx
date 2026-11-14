import { ChevronDown, ChevronLeft, ChevronRight, ChevronUp, ExternalLink, Highlighter, Keyboard, Maximize2, Minus, Moon, MoreVertical, PanelRight, Plus, Rows3, Search, Square, StickyNote, Sun, Underline, X } from 'lucide-react';
import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent, type MouseEvent } from 'react';
import type { PDFDocumentProxy, RenderTask } from 'pdfjs-dist';
import { usePdfAnnotationMutations, usePdfAnnotations } from '@/app/data';
import { ColorSwatches, PdfMarks } from '@/components/player/pdf-marks';
import { Button } from '@/components/ui/button';
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogTitle } from '@/components/ui/dialog';
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuRadioGroup, DropdownMenuRadioItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { studySyncSupported } from '@/lib/player';
import { tr } from '@/lib/i18n';
import { clampPage, loadPdfJs, nextPage, pageAt, pageScroll, pagesMatching, pageWindow, pdfCommand, PDF_MARK_COLORS, PDF_PAGE_GAP, selectionBoxes, stepZoom, type PdfTheme, type PdfView } from '@/lib/pdf';
import { transport, type FileEntry, type PdfAnnotation, type PdfAnnotationColor, type PdfRect } from '@/lib/transport';
import { cn } from '@/lib/utils';
import type { PlayerPreferences } from '@/lib/player-preferences';

type Props = {
	file: FileEntry;
	courseId: string;
	/** 1-based page to open on, from `?t=` or the saved position. */
	initialPage: number | null;
	/** Called with the page whenever it changes, so reading progress can be saved. */
	onPage: (page: number, pages: number) => void;
	preferences: PlayerPreferences;
	onPreferencesChange: (next: PlayerPreferences) => void;
};

/** The selected range when it sits inside one page's text layer; a selection across pages is not markable. */
function selectionIn(root: HTMLElement | null): { range: Range; layer: HTMLElement; page: number } | null {
	const selection = window.getSelection();
	const range = selection?.rangeCount ? selection.getRangeAt(0) : null;
	if (!range || range.collapsed) return null;
	const node = range.commonAncestorContainer;
	const element = node instanceof Element ? node : node.parentElement;
	const layer = element?.closest<HTMLElement>('.pdf-text-layer');
	if (!layer || !root?.contains(layer)) return null;
	return { range, layer, page: Number(layer.closest<HTMLElement>('[data-page]')?.dataset.page) || 1 };
}

type Load = { state: 'loading' } | { state: 'ready'; doc: PDFDocumentProxy } | { state: 'error'; message: string };

/** A fallback page shape until the first page is measured; A4 is the common case. */
const DEFAULT_RATIO = 1.414;

/** Paper, sepia and night surfaces; the night one is a warm near-black, never pure black. */
const PAGE_SURFACES: Record<PdfTheme, string> = {
	paper: '#fff',
	sepia: 'color-mix(in oklch, var(--brand) 8%, #FFFDF9)',
	night: 'color-mix(in oklch, var(--brand) 22%, black)'
};

/** Night dims and re-hues the page instead of a plain invert, so ink stays readable and photos keep their hue. */
const PAGE_FILTER: Record<PdfTheme, string | undefined> = {
	paper: undefined,
	sepia: 'sepia(0.45) saturate(0.9)',
	night: 'invert(0.92) hue-rotate(180deg)'
};

/**
 * A toolbar is one tab stop: arrow keys move between its buttons and only the focused one is
 * tabbable. Form fields stay out of the roving set.
 */
function useRovingToolbar(initial: string) {
	const [at, setAt] = useState(initial);
	const tab = (id: string) => ({ tabIndex: at === id ? 0 : -1, onFocus: () => setAt(id) });
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if (event.ctrlKey || event.metaKey || event.altKey) return;
		if (!['ArrowRight', 'ArrowLeft', 'Home', 'End'].includes(event.key)) return;
		if ((event.target as HTMLElement).closest('input, select, textarea')) return;
		const items = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('button:not([disabled])')).filter((item) => item.offsetParent !== null);
		const from = items.findIndex((item) => item === document.activeElement);
		if (from === -1) return;
		event.preventDefault();
		const to = event.key === 'Home' ? 0
			: event.key === 'End' ? items.length - 1
				: (from + (event.key === 'ArrowRight' ? 1 : -1) + items.length) % items.length;
		items[to].focus();
	};
	return { tab, onKeyDown };
}

/** True from the first breakpoint up; the reading bar collapses to icons plus one menu below it. */
function useWide() {
	const query = '(min-width: 640px)';
	const [wide, setWide] = useState(() => window.matchMedia?.(query)?.matches ?? true);
	useEffect(() => {
		const media = window.matchMedia?.(query);
		if (!media) return;
		const update = () => setWide(media.matches);
		update();
		media.addEventListener('change', update);
		window.addEventListener('resize', update);
		return () => {
			media.removeEventListener('change', update);
			window.removeEventListener('resize', update);
		};
	}, []);
	return wide;
}

/**
 * Reads a PDF from the course, read-only. Pages render to a canvas with a selectable text
 * layer on top, so text can be copied and read by screen readers. The whole document can sit
 * in one scrollable column (pages painted only near the viewport) or one page at a time.
 * The original file is never written. Without pdf.js (the desktop build) the file opens in
 * the system viewer instead.
 */
export function PdfReader({ file, courseId, initialPage, onPage, preferences, onPreferencesChange }: Props) {
	const [load, setLoad] = useState<Load>({ state: 'loading' });
	const [page, setPage] = useState(() => Math.max(1, initialPage ?? 1));
	const [zoom, setZoom] = useState<number | 'fit'>('fit');
	const [markColor, setMarkColor] = useState<PdfAnnotationColor>('yellow');
	const [markMode, setMarkMode] = useState(false);
	const [findOpen, setFindOpen] = useState(false);
	const wide = useWide();
	const [panelOpen, setPanelOpen] = useState(wide);
	const [hasSelection, setHasSelection] = useState(false);
	const [base, setBase] = useState<{ width: number; height: number } | null>(null);
	const [width, setWidth] = useState(0);
	const [scrollTop, setScrollTop] = useState(0);
	const [viewport, setViewport] = useState(0);
	const [help, setHelp] = useState(false);
	const mediaId = file.mediaId ?? null;
	const marks = usePdfAnnotations(mediaId);
	const { create, update, remove } = usePdfAnnotationMutations(mediaId);
	// A note needs text, so a selection waits here until its text is written.
	const [draft, setDraft] = useState<{ page: number; rects: PdfRect[]; quote: string; text: string } | null>(null);
	const [query, setQuery] = useState('');
	const [searchState, setSearch] = useState<{ url: string; query: string; pages: number[]; at: number } | null>(null);
	// Results belong to the document they were found in.
	const docKey = `${courseId}/${file.path}`;
	const search = searchState?.url === docKey ? searchState : null;
	const [searching, setSearching] = useState(false);
	const texts = useRef<{ url: string; list: string[] } | null>(null);
	const stage = useRef<HTMLDivElement>(null);
	const viewportRef = useRef<HTMLDivElement>(null);
	const frame = useRef(0);
	const strideRef = useRef(0);
	const url = transport.mediaUrl(file, courseId);
	const mode = preferences.pdfView;
	const theme = preferences.pdfTheme;
	const bar = useRovingToolbar('mode');

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
			.catch(() => { if (active) setLoad({ state: 'error', message: tr(`“${file.name}” could not be opened. It may be damaged or password protected.`, `Não foi possível abrir “${file.name}”. Ele pode estar danificado ou protegido por senha.`) }); });
		return () => {
			active = false;
			void task?.destroy();
		};
	}, [url, file.name]);

	const pages = load.state === 'ready' ? load.doc.numPages : 0;

	// The first page's shape sets the height of every placeholder, so the scrollbar stays still
	// while pages are painted and dropped.
	useEffect(() => {
		if (load.state !== 'ready') return;
		let active = true;
		void load.doc.getPage(1).then((first) => {
			const size = first.getViewport({ scale: 1 });
			if (active) setBase({ width: size.width, height: size.height });
		}).catch(() => undefined);
		return () => { active = false; };
	}, [load]);

	// Mark buttons enable only while a selection sits in one page's text.
	useEffect(() => {
		const update = () => setHasSelection(Boolean(selectionIn(stage.current)));
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

	useEffect(() => () => cancelAnimationFrame(frame.current), []);

	const ratio = base && base.width > 0 ? base.height / base.width : DEFAULT_RATIO;
	const pageWidth = Math.max(0, zoom === 'fit' ? Math.floor(width) : Math.round((base?.width ?? 0) * zoom));
	const stride = Math.round(pageWidth * ratio) + PDF_PAGE_GAP;
	useEffect(() => { strideRef.current = stride; }, [stride]);
	const paint = mode === 'continuous' ? pageWindow(scrollTop, viewport, stride, pages) : { first: current - 1, last: current - 1 };

	/** Keeps the column on `target`, so a mode or zoom change does not lose the place. */
	const scrollTo = (target: number) => {
		const element = viewportRef.current;
		if (element && stride > 0) element.scrollTop = pageScroll(target, stride);
	};
	useEffect(() => {
		if (mode === 'continuous' && load.state === 'ready') scrollTo(current);
	}, [mode, zoom, base, width]); // eslint-disable-line react-hooks/exhaustive-deps -- re-anchor when the layout changes, not on every scroll-driven page change

	/** Reads the scroll position once per frame: the painted window, the anchor page, the viewport. */
	const trackScroll = (element: HTMLElement) => {
		if (frame.current !== 0) return;
		frame.current = requestAnimationFrame(() => {
			frame.current = 0;
			setScrollTop(element.scrollTop);
			setViewport(element.clientHeight);
			if (pages) setPage(pageAt(element.scrollTop, element.clientHeight, strideRef.current, pages, element.scrollHeight - element.clientHeight));
		});
	};

	if (!loadPdfJs) return <ViewerFallback file={file} />;

	const mark = (kind: 'highlight' | 'underline' | 'note') => {
		const selected = selectionIn(stage.current);
		if (!selected || !mediaId) return;
		const canvas = selected.layer.closest<HTMLElement>('[data-page]')?.querySelector('canvas');
		if (!canvas) return;
		const rects = selectionBoxes(selected.range.getClientRects(), canvas.getBoundingClientRect());
		const quote = selected.range.toString().trim().slice(0, 4000);
		if (rects.length > 0) {
			if (kind === 'note') setDraft({ page: selected.page, rects, quote, text: '' });
			else create.mutate({ page: selected.page, kind, color: markColor, rects, quote });
		}
		window.getSelection()?.removeAllRanges();
	};

	const go = (next: number) => {
		if (!pages) return;
		const target = clampPage(next, pages);
		setPage(target);
		if (mode === 'continuous') scrollTo(target);
	};

	/** Reads every page's text once, then lists the pages that contain the query. */
	const runSearch = async (event: FormEvent) => {
		event.preventDefault();
		if (load.state !== 'ready' || !query.trim()) return;
		const { doc } = load;
		setSearching(true);
		try {
			if (texts.current?.url !== docKey) {
				const list = await Promise.all(Array.from({ length: doc.numPages }, async (_, index) => {
					const content = await (await doc.getPage(index + 1)).getTextContent();
					return content.items.map((item) => ('str' in item ? item.str : '')).join(' ');
				}));
				texts.current = { url: docKey, list };
			}
			const found = pagesMatching(texts.current.list, query);
			setSearch({ url: docKey, query: query.trim(), pages: found, at: 0 });
			if (found.length > 0) go(found[0]);
		} catch {
			setSearch({ url: docKey, query: query.trim(), pages: [], at: 0 });
		} finally {
			setSearching(false);
		}
	};
	const stepMatch = (direction: 1 | -1) => {
		if (!search || search.pages.length === 0) return;
		const at = (search.at + direction + search.pages.length) % search.pages.length;
		setSearch({ ...search, at });
		go(search.pages[at]);
	};
	/** `n`/`p`: the next search hit, or the next marked page when there is no search. */
	const stepStop = (direction: 1 | -1) => {
		if (search && search.pages.length > 0) return stepMatch(direction);
		const target = nextPage(current, (marks.data ?? []).map((item) => item.page), direction);
		if (target !== null) go(target);
	};
	const run = (command: ReturnType<typeof pdfCommand>) => {
		switch (command) {
			case 'pagePrevious': return go(current - 1);
			case 'pageNext': return go(current + 1);
			case 'pageFirst': return go(1);
			case 'pageLast': return go(pages);
			case 'zoomIn': return setZoom(stepZoom(zoom, 1));
			case 'zoomOut': return setZoom(stepZoom(zoom, -1));
			case 'stopNext': return stepStop(1);
			case 'stopPrevious': return stepStop(-1);
			case 'highlight': return mark('highlight');
			case 'underline': return mark('underline');
			case 'help': return setHelp(true);
		}
	};
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		// Typing in a field or the page search is never a shortcut.
		if ((event.target as HTMLElement).closest('input, select, textarea, [contenteditable="true"]')) return;
		// Browser shortcuts such as Ctrl+- and Ctrl+N are not ours to take.
		if (event.ctrlKey || event.metaKey || event.altKey) return;
		const command = pdfCommand(event.key);
		if (!command) return;
		event.preventDefault();
		run(command);
	};

	const save = (patch: Partial<PlayerPreferences>) => onPreferencesChange({ ...preferences, ...patch });
	const zoomLabel = zoom === 'fit' ? tr('Fit', 'Ajustar') : `${Math.round(zoom * 100)}%`;
	const shortcuts = [
		{ keys: 'j / k', action: tr('Next / previous page', 'Próxima / página anterior') },
		{ keys: 'g / G', action: tr('First / last page', 'Primeira / última página') },
		{ keys: '+ / -', action: tr('Zoom in / out', 'Aumentar / diminuir zoom') },
		{ keys: 'n / p', action: tr('Next / previous search hit or mark', 'Próximo / anterior resultado ou marcação') },
		{ keys: 'h', action: tr('Highlight the selected text', 'Destacar o texto selecionado') },
		{ keys: 'u', action: tr('Underline the selected text', 'Sublinhar o texto selecionado') },
		{ keys: '?', action: tr('Open this shortcut sheet', 'Abrir esta lista de atalhos') }
	];

	return (
		<div className="space-y-3">
			<div role="toolbar" aria-label={tr('PDF controls', 'Controles do PDF')} onKeyDown={bar.onKeyDown} className="grid grid-cols-[auto_1fr_auto] items-center gap-1 rounded-2xl border bg-card p-1.5 sm:gap-2 sm:p-2">
				{/* Start: reading mode and zoom. */}
				<div className="flex min-w-0 items-center gap-1">
					<DropdownMenu>
						<DropdownMenuTrigger render={<Button {...bar.tab('mode')} variant="ghost" size="sm" aria-label={tr(`Reading mode: ${mode === 'continuous' ? 'Continuous' : 'Single page'}`, `Modo de leitura: ${mode === 'continuous' ? 'Contínuo' : 'Página única'}`)} />}>
							{mode === 'continuous' ? <Rows3 /> : <Square />}
							<span className="hidden lg:inline">{mode === 'continuous' ? tr('Continuous', 'Contínuo') : tr('Single page', 'Página única')}</span>
						</DropdownMenuTrigger>
						<DropdownMenuContent align="start" className="w-56">
							<DropdownMenuRadioGroup value={mode} onValueChange={(value) => save({ pdfView: value as PdfView })}>
								<DropdownMenuRadioItem value="continuous">{tr('Continuous scrolling', 'Rolagem contínua')}</DropdownMenuRadioItem>
								<DropdownMenuRadioItem value="single">{tr('One page at a time', 'Uma página por vez')}</DropdownMenuRadioItem>
							</DropdownMenuRadioGroup>
						</DropdownMenuContent>
					</DropdownMenu>
					{wide && (
						<>
							<Button {...bar.tab('zoomOut')} variant="ghost" size="icon" aria-label={tr('Zoom out', 'Diminuir zoom')} onClick={() => setZoom(stepZoom(zoom, -1))}><Minus /></Button>
							<span className="min-w-12 text-center text-sm tabular-nums" aria-live="polite">{zoomLabel}</span>
							<Button {...bar.tab('zoomIn')} variant="ghost" size="icon" aria-label={tr('Zoom in', 'Aumentar zoom')} onClick={() => setZoom(stepZoom(zoom, 1))}><Plus /></Button>
							<Button {...bar.tab('fit')} variant="ghost" size="icon" aria-label={tr('Fit to width', 'Ajustar à largura')} aria-pressed={zoom === 'fit'} onClick={() => setZoom('fit')}><Maximize2 /></Button>
						</>
					)}
				</div>

				{/* Centre: the page, with its jump field. */}
				<div className="flex items-center gap-1.5">
					{wide && <Button {...bar.tab('previous')} variant="ghost" size="icon" aria-label={tr('Previous page', 'Página anterior')} disabled={current <= 1} onClick={() => go(current - 1)}><ChevronLeft /></Button>}
					<Label htmlFor="pdf-page" className="sr-only">{tr('Page', 'Página')}</Label>
					<Input id="pdf-page" type="number" inputMode="numeric" min={1} max={pages || undefined} value={current} disabled={!pages} onChange={(event) => go(Number(event.target.value))} className="pdf-page-field h-11 w-12 text-center tabular-nums sm:w-14" />
					<span className="text-xs whitespace-nowrap text-muted-foreground tabular-nums sm:text-sm">{tr(`of ${pages || '…'}`, `de ${pages || '…'}`)}</span>
					{wide && <Button {...bar.tab('next')} variant="ghost" size="icon" aria-label={tr('Next page', 'Próxima página')} disabled={!pages || current >= pages} onClick={() => go(current + 1)}><ChevronRight /></Button>}
				</div>

				{/* End: annotate, find, panel, reading theme. */}
				<div className="flex min-w-0 items-center justify-end gap-1">
					{studySyncSupported && mediaId && load.state === 'ready' && (
						<Button {...bar.tab('mark')} variant={markMode ? 'selected' : 'ghost'} size="icon" aria-label={tr('Annotate', 'Anotar')} aria-pressed={markMode} onClick={() => setMarkMode(!markMode)}><Highlighter /></Button>
					)}
					{load.state === 'ready' && (
						<Button {...bar.tab('find')} variant={findOpen ? 'selected' : 'ghost'} size="icon" aria-label={tr('Find in the document', 'Localizar no documento')} aria-expanded={findOpen} onClick={() => setFindOpen(!findOpen)}><Search /></Button>
					)}
					{wide && studySyncSupported && mediaId && (
						<Button {...bar.tab('panel')} variant="ghost" size="icon" aria-label={tr('Marks panel', 'Painel de marcações')} aria-expanded={panelOpen} onClick={() => setPanelOpen(!panelOpen)}><PanelRight /></Button>
					)}
					<DropdownMenu>
						<DropdownMenuTrigger render={<Button {...bar.tab('theme')} variant="ghost" size="icon" aria-label={tr('Reading theme', 'Tema de leitura')} />}>{theme === 'night' ? <Moon /> : <Sun />}</DropdownMenuTrigger>
						<DropdownMenuContent align="end" className="w-44">
							<DropdownMenuRadioGroup value={theme} onValueChange={(value) => save({ pdfTheme: value as PdfTheme })}>
								<DropdownMenuRadioItem value="paper">{tr('Paper', 'Papel')}</DropdownMenuRadioItem>
								<DropdownMenuRadioItem value="sepia">{tr('Sepia', 'Sépia')}</DropdownMenuRadioItem>
								<DropdownMenuRadioItem value="night">{tr('Night', 'Noturno')}</DropdownMenuRadioItem>
							</DropdownMenuRadioGroup>
						</DropdownMenuContent>
					</DropdownMenu>
					{wide && (
						<Button {...bar.tab('help')} variant="ghost" size="icon" aria-label={tr('Keyboard shortcuts', 'Atalhos de teclado')} aria-keyshortcuts="?" onClick={() => setHelp(true)}><Keyboard /></Button>
					)}
					{!wide && (
						<DropdownMenu>
							<DropdownMenuTrigger render={<Button {...bar.tab('more')} variant="ghost" size="icon" aria-label={tr('More reading controls', 'Mais controles de leitura')} />}><MoreVertical /></DropdownMenuTrigger>
							<DropdownMenuContent align="end" className="w-56">
								<DropdownMenuItem disabled={current <= 1} onClick={() => go(current - 1)}><ChevronLeft />{tr('Previous page', 'Página anterior')}</DropdownMenuItem>
								<DropdownMenuItem disabled={!pages || current >= pages} onClick={() => go(current + 1)}><ChevronRight />{tr('Next page', 'Próxima página')}</DropdownMenuItem>
								<DropdownMenuItem onClick={() => setZoom(stepZoom(zoom, 1))}><Plus />{tr('Zoom in', 'Aumentar zoom')}</DropdownMenuItem>
								<DropdownMenuItem onClick={() => setZoom(stepZoom(zoom, -1))}><Minus />{tr('Zoom out', 'Diminuir zoom')}</DropdownMenuItem>
								<DropdownMenuItem onClick={() => setZoom('fit')}><Maximize2 />{tr('Fit to width', 'Ajustar à largura')}</DropdownMenuItem>
								{studySyncSupported && mediaId && <DropdownMenuItem onClick={() => setPanelOpen(!panelOpen)}><PanelRight />{tr('Marks panel', 'Painel de marcações')}</DropdownMenuItem>}
								<DropdownMenuItem onClick={() => setHelp(true)}><Keyboard />{tr('Keyboard shortcuts', 'Atalhos de teclado')}</DropdownMenuItem>
							</DropdownMenuContent>
						</DropdownMenu>
					)}
				</div>
			</div>

			{findOpen && load.state === 'ready' && (
				<form role="search" aria-label={tr('Search in the document', 'Buscar no documento')} onSubmit={(event) => void runSearch(event)} className="flex flex-wrap items-center gap-2 rounded-2xl border bg-card p-2">
					<div className="relative min-w-0 flex-1">
						<Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
						<Input type="search" value={query} onChange={(event) => setQuery(event.target.value)} aria-label={tr('Search in the document', 'Buscar no documento')} placeholder={tr('Search in the document', 'Buscar no documento')} className="h-11 pl-9" />
					</div>
					<Button type="submit" variant="outline" size="sm" disabled={searching || !query.trim()}>{tr('Search', 'Buscar')}</Button>
					{search && search.pages.length > 0 && (
						<>
							<Button type="button" variant="ghost" size="icon" aria-label={tr('Previous result', 'Resultado anterior')} onClick={() => stepMatch(-1)}><ChevronUp /></Button>
							<Button type="button" variant="ghost" size="icon" aria-label={tr('Next result', 'Próximo resultado')} onClick={() => stepMatch(1)}><ChevronDown /></Button>
						</>
					)}
					<Button type="button" variant="ghost" size="icon" aria-label={tr('Close find', 'Fechar localização')} onClick={() => setFindOpen(false)}><X /></Button>
				</form>
			)}

			{load.state === 'ready' && (
				<div className="space-y-1">
					<p role="status" aria-live="polite" className="min-h-5 text-sm text-muted-foreground">
						{search && (search.pages.length > 0 ? tr(`Found on ${search.pages.length} ${search.pages.length === 1 ? 'page' : 'pages'}: now on page ${search.pages[search.at]}.`, `Encontrado em ${search.pages.length} ${search.pages.length === 1 ? 'página' : 'páginas'}: agora na página ${search.pages[search.at]}.`) : tr(`Nothing found for “${search.query}”.`, `Nada encontrado para “${search.query}”.`))}
					</p>
				</div>
			)}

			{markMode && studySyncSupported && mediaId && load.state === 'ready' && (
				<div role="toolbar" aria-label={tr('Mark the selected text', 'Marcar o texto selecionado')} className="flex flex-wrap items-center gap-2 rounded-2xl border bg-card p-2">
					<ColorSwatches label={tr('Ink color', 'Cor da tinta')} value={markColor} onChange={setMarkColor} />
					{/* Mouse-down is cancelled so pressing a mark button keeps the text selected. */}
					<Button variant="outline" size="sm" aria-keyshortcuts="h" disabled={!hasSelection || create.isPending} onMouseDown={(event: MouseEvent) => event.preventDefault()} onClick={() => mark('highlight')}><Highlighter />{tr('Highlight', 'Destacar')}</Button>
					<Button variant="outline" size="sm" aria-keyshortcuts="u" disabled={!hasSelection || create.isPending} onMouseDown={(event: MouseEvent) => event.preventDefault()} onClick={() => mark('underline')}><Underline />{tr('Underline', 'Sublinhar')}</Button>
					<Button variant="outline" size="sm" disabled={!hasSelection || create.isPending} onMouseDown={(event: MouseEvent) => event.preventDefault()} onClick={() => mark('note')}><StickyNote />{tr('Add note', 'Adicionar nota')}</Button>
					{(create.error || update.error || remove.error) && <p role="alert" className="text-sm text-destructive">{(create.error ?? update.error ?? remove.error)?.message}</p>}
				</div>
			)}

			{draft && (
				<form className="space-y-3 rounded-2xl border bg-panel p-4" aria-label={tr('New note', 'Nova nota')} onSubmit={(event) => { event.preventDefault(); if (!draft.text.trim()) return; create.mutate({ page: draft.page, kind: 'note', color: markColor, rects: draft.rects, quote: draft.quote, content: draft.text.trim() }, { onSuccess: () => setDraft(null) }); }}>
					{draft.quote && <blockquote className="border-l-4 pl-3 text-sm" style={{ borderColor: PDF_MARK_COLORS[markColor] }}>{draft.quote}</blockquote>}
					<Label htmlFor="pdf-note-text">{tr(`Note for page ${draft.page}`, `Nota da página ${draft.page}`)}</Label>
					<textarea id="pdf-note-text" value={draft.text} rows={3} maxLength={4000} onChange={(event) => setDraft({ ...draft, text: event.target.value })} className="min-h-11 w-full resize-y rounded-xl border border-input bg-background px-3 py-2 text-sm focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none" />
					<div className="flex gap-2">
						<Button type="submit" size="sm" disabled={!draft.text.trim() || create.isPending}>{tr('Save note', 'Salvar nota')}</Button>
						<Button type="button" variant="ghost" size="sm" onClick={() => setDraft(null)}>{tr('Cancel', 'Cancelar')}</Button>
					</div>
				</form>
			)}

			{load.state === 'error' ? (
				<p role="alert" className="rounded-2xl border border-destructive/50 p-4 text-destructive">{load.message}</p>
			) : (
				// The group takes focus so single-key shortcuts only act on the reader (WCAG 2.1.4).
				// eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex, jsx-a11y/no-noninteractive-element-interactions
				<div ref={viewportRef} role="group" tabIndex={0} onKeyDown={onKeyDown} onScroll={mode === 'continuous' ? (event) => trackScroll(event.currentTarget) : undefined} aria-label={tr('PDF page', 'Página do PDF')} aria-keyshortcuts="ArrowLeft ArrowRight PageUp PageDown Home End j k g G + - n p h u" className="pdf-scroll h-[70vh] overflow-auto rounded-[1.75rem] border bg-panel p-2 focus-visible:ring-3 focus-visible:ring-ring sm:p-4">
					<div ref={stage} className="mx-auto w-full max-w-4xl">
						{load.state === 'loading' && <p role="status" className="p-8 text-center text-muted-foreground">{tr('Opening the PDF…', 'Abrindo o PDF…')}</p>}
						{load.state === 'ready' && (
							<div className="space-y-4">
								{(mode === 'continuous' ? Array.from({ length: pages }, (_, index) => index + 1) : [current]).map((number) => (
									<div key={number} className="mx-auto" style={{ width: pageWidth || undefined }}>
										{number - 1 >= paint.first && number - 1 <= paint.last ? (
											<PdfPage doc={load.doc} page={number} pages={pages} width={width} zoom={zoom} ratio={ratio} pageTheme={theme} marks={(marks.data ?? []).filter((item) => item.page === number)} />
										) : (
											// A placeholder with the document's page shape keeps the scrollbar still.
											<div aria-hidden="true" className="w-full shadow-sm" style={{ aspectRatio: `1 / ${ratio}`, backgroundColor: PAGE_SURFACES[theme] }} />
										)}
									</div>
								))}
							</div>
						)}
					</div>
				</div>
			)}
			<p role="status" aria-live="polite" className="sr-only">{pages ? tr(`Page ${current} of ${pages}`, `Página ${current} de ${pages}`) : ''}</p>
			{studySyncSupported && mediaId && panelOpen && (
				<section aria-labelledby="pdf-marks-heading" className="space-y-3 rounded-[1.75rem] border bg-card p-5">
					<h2 id="pdf-marks-heading" className="font-heading text-xl font-bold">{tr('Marks in this document', 'Marcações neste documento')}</h2>
					<PdfMarks title={file.name} items={marks.data ?? []} page={current} onGo={go} onChange={(annotationId, change) => update.mutate({ annotationId, ...change })} onDelete={(annotationId) => remove.mutate(annotationId)} pending={remove.isPending} />
				</section>
			)}

			<Dialog open={help} onOpenChange={setHelp}>
				<DialogContent showCloseButton={false}>
					<DialogTitle>{tr('Keyboard shortcuts', 'Atalhos de teclado')}</DialogTitle>
					<DialogDescription>{tr('These keys work while the PDF page has focus.', 'Estas teclas funcionam enquanto a página do PDF está em foco.')}</DialogDescription>
					<ul className="space-y-2 text-sm">
						{shortcuts.map((item) => (
							<li key={item.keys} className="flex items-baseline gap-3">
								<kbd className="min-w-16 font-mono text-xs font-semibold">{item.keys}</kbd>
								<span className="text-muted-foreground">{item.action}</span>
							</li>
						))}
					</ul>
					<DialogClose render={<Button variant="outline" />}>{tr('Close', 'Fechar')}</DialogClose>
				</DialogContent>
			</Dialog>
		</div>
	);
}

type PdfPageProps = {
	doc: PDFDocumentProxy;
	page: number;
	pages: number;
	width: number;
	zoom: number | 'fit';
	ratio: number;
	pageTheme: PdfTheme;
	marks: PdfAnnotation[];
};

/** One page: its canvas, its marks and its text layer. Mounted only while the page is on screen. */
function PdfPage({ doc, page, pages, width, zoom, ratio, pageTheme, marks }: PdfPageProps) {
	const canvas = useRef<HTMLCanvasElement>(null);
	const textLayer = useRef<HTMLDivElement>(null);
	const [size, setSize] = useState<{ width: number; height: number } | null>(null);

	useEffect(() => {
		if (!width) return;
		let cancelled = false;
		let render: RenderTask | null = null;
		void (async () => {
			const [pdfjs, pageProxy] = await Promise.all([loadPdfJs!(), doc.getPage(page)]);
			if (cancelled || !canvas.current || !textLayer.current) return;
			const target = canvas.current;
			const textTarget = textLayer.current;
			const base = pageProxy.getViewport({ scale: 1 });
			const scale = zoom === 'fit' ? Math.max(0.25, width / base.width) : zoom;
			const viewport = pageProxy.getViewport({ scale });
			const pixelRatio = window.devicePixelRatio || 1;
			target.width = Math.floor(viewport.width * pixelRatio);
			target.height = Math.floor(viewport.height * pixelRatio);
			target.style.width = `${Math.floor(viewport.width)}px`;
			target.style.height = `${Math.floor(viewport.height)}px`;
			textTarget.replaceChildren();
			textTarget.style.width = target.style.width;
			textTarget.style.height = target.style.height;
			textTarget.style.setProperty('--total-scale-factor', String(viewport.scale));
			setSize({ width: Math.floor(viewport.width), height: Math.floor(viewport.height) });
			render = pageProxy.render({ canvas: target, viewport, transform: pixelRatio === 1 ? undefined : [pixelRatio, 0, 0, pixelRatio, 0, 0] });
			await render.promise;
			if (cancelled) return;
			await new pdfjs.TextLayer({ textContentSource: pageProxy.streamTextContent(), container: textTarget, viewport }).render();
		})().catch(() => undefined); // a cancelled render rejects; a real failure leaves the page blank and the toolbar usable
		return () => {
			cancelled = true;
			render?.cancel();
		};
	}, [doc, page, width, zoom]);

	return (
		<div data-page={page} className="relative w-full shadow-sm" style={{ backgroundColor: PAGE_SURFACES[pageTheme], ...(size ? {} : { aspectRatio: `1 / ${ratio}` }) }}>
			<canvas ref={canvas} role="img" aria-label={tr(`Page ${page} of ${pages}`, `Página ${page} de ${pages}`)} style={{ filter: PAGE_FILTER[pageTheme] }} className={cn('block', !size && 'hidden')} />
			{size && marks.flatMap((item) => item.rects.map((rect, index) => (
				<span key={`${item.annotationId}-${index}`} aria-hidden="true" className={cn('pointer-events-none absolute', item.kind === 'underline' ? 'border-b-2' : pageTheme === 'night' ? 'mix-blend-screen' : 'mix-blend-multiply', item.kind === 'note' && 'outline-1 outline-dashed outline-foreground/40')}
					style={{ left: `${rect.x * 100}%`, top: `${rect.y * 100}%`, width: `${rect.width * 100}%`, height: `${rect.height * 100}%`, ...(item.kind === 'underline' ? { borderColor: PDF_MARK_COLORS[item.color] } : { backgroundColor: PDF_MARK_COLORS[item.color] }) }} />
			)))}
			<div ref={textLayer} className="pdf-text-layer" />
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
