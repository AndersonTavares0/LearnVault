import { getLocale, tr } from './i18n';
import type { FileEntry, Lesson, PdfAnnotation } from './transport';

/**
 * pdf.js is only bundled for the server (HTTP) build. The desktop bundle must contain no
 * network `fetch(` (src/build-output.test.ts), and pdf.js ships one, so the inline build
 * constant lets the bundler drop this whole branch there; the desktop app hands the file to
 * the system viewer instead. The worker is a same-origin asset, so `script-src 'self'` holds.
 */
export const loadPdfJs = __LEARNVAULT_TRANSPORT__ === 'http'
	? async () => {
		const [pdfjs, worker] = await Promise.all([import('pdfjs-dist'), import('pdfjs-dist/build/pdf.worker.min.mjs?url')]);
		pdfjs.GlobalWorkerOptions.workerSrc = worker.default;
		return pdfjs;
	}
	: null;

export const isPdf = (file: Pick<FileEntry, 'name'>) => /\.pdf$/i.test(file.name);

/**
 * A lesson is a PDF lesson when it has nothing to play and a readable PDF: the PDF is then
 * the lesson itself. A lesson with media keeps its player, and its PDFs stay materials.
 */
export function findLessonPdf(lesson: Lesson): FileEntry | null {
	if (lesson.media.length > 0) return null;
	return lesson.materials.find((file) => isPdf(file) && file.availability?.trim().toLowerCase() !== 'missing' && file.mediaId) ?? null;
}

/**
 * Id that carries the reading progress of a lesson node with no media: its first readable PDF.
 * Course totals still count only playable lessons, so reading a PDF lesson never changes a
 * course's percentage; it only remembers the page and lets the lesson be marked done.
 */
export function lessonPdfId(node: { files: FileEntry[]; looseFiles: FileEntry[] }): string | null {
	const files = [...node.files, ...node.looseFiles];
	if (files.some((file) => file.kind === 'media')) return null;
	return files.find((file) => isPdf(file) && file.availability?.trim().toLowerCase() !== 'missing' && file.mediaId)?.mediaId ?? null;
}

export const PDF_ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3] as const;

/** Reading modes: one page at a time, or the whole document in one vertical column. */
export type PdfView = 'single' | 'continuous';

/** Reading surfaces: the white page, warm paper, or a dimmed night page. */
export type PdfTheme = 'paper' | 'sepia' | 'night';

/** Gap between the pages of the continuous column, matching `space-y-4`. */
export const PDF_PAGE_GAP = 16;

/** 0-based first and last page to paint for a scroll position, with `overscan` pages on each side. */
export function pageWindow(scrollTop: number, viewport: number, stride: number, pages: number, overscan = 1): { first: number; last: number } {
	if (pages < 1) return { first: 0, last: -1 };
	// Before the column is measured there is no geometry to virtualize with.
	if (!(stride > 0)) return { first: 0, last: pages - 1 };
	const top = Math.max(0, scrollTop);
	const first = Math.max(0, Math.floor(top / stride) - overscan);
	const last = Math.min(pages - 1, Math.floor((top + Math.max(0, viewport)) / stride) + overscan);
	return { first, last: Math.max(first, last) };
}

/**
 * 1-based page under the reading anchor, a third down the viewport. At the bottom of the
 * scroll the last page wins, because the column cannot scroll the final page to its top.
 */
export function pageAt(scrollTop: number, viewport: number, stride: number, pages: number, maxScroll = Number.POSITIVE_INFINITY): number {
	if (!(stride > 0) || pages < 1) return 1;
	if (scrollTop >= maxScroll - 1) return pages;
	const anchor = Math.max(0, scrollTop) + Math.min(viewport / 3, stride / 2);
	return clampPage(Math.floor(anchor / stride) + 1, pages);
}

/** Scroll offset of a page's top inside the continuous column. */
export function pageScroll(page: number, stride: number): number {
	return Math.max(0, page - 1) * stride;
}

/** The next page of `order` from `page` in a direction, wrapping around; null when there is none. */
export function nextPage(page: number, order: number[], direction: 1 | -1): number | null {
	const list = [...new Set(order)].sort((a, b) => a - b);
	if (list.length === 0) return null;
	return direction > 0
		? list.find((item) => item > page) ?? list[0]
		: [...list].reverse().find((item) => item < page) ?? list[list.length - 1];
}

export type PdfCommand = 'pagePrevious' | 'pageNext' | 'pageFirst' | 'pageLast' | 'zoomIn' | 'zoomOut' | 'stopNext' | 'stopPrevious' | 'highlight' | 'underline' | 'help';

/** The reader command bound to a key. Modified keys stay with the browser and are filtered out first. */
const PDF_COMMANDS: Record<string, PdfCommand> = {
	ArrowLeft: 'pagePrevious', PageUp: 'pagePrevious', k: 'pagePrevious',
	ArrowRight: 'pageNext', PageDown: 'pageNext', j: 'pageNext',
	Home: 'pageFirst', g: 'pageFirst',
	End: 'pageLast', G: 'pageLast',
	'+': 'zoomIn', '=': 'zoomIn',
	'-': 'zoomOut',
	n: 'stopNext', p: 'stopPrevious',
	h: 'highlight', u: 'underline', '?': 'help'
};

export function pdfCommand(key: string): PdfCommand | null {
	return PDF_COMMANDS[key] ?? null;
}

export function clampPage(page: number, pages: number): number {
	if (!Number.isFinite(page) || pages < 1) return 1;
	return Math.min(pages, Math.max(1, Math.round(page)));
}

/** The next zoom step in a direction; stays at the ends. A fit-to-width view steps from 1×. */
export function stepZoom(current: number | 'fit', direction: 1 | -1): number {
	const base = current === 'fit' ? 1 : current;
	const index = PDF_ZOOMS.findIndex((zoom) => zoom >= base - 1e-6);
	const from = index === -1 ? PDF_ZOOMS.length - 1 : index;
	return PDF_ZOOMS[Math.min(PDF_ZOOMS.length - 1, Math.max(0, from + direction))];
}

/** "1.2 MB": one decimal under ten, none above; 1024-based like the file manager. */
export function formatFileSize(bytes: number): string {
	if (!Number.isFinite(bytes) || bytes < 1024) return `${Math.max(0, Math.round(bytes || 0))} B`;
	const units = ['KB', 'MB', 'GB'];
	let value = bytes / 1024;
	let unit = 0;
	while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit += 1; }
	return `${value.toLocaleString(getLocale(), { maximumFractionDigits: value < 10 ? 1 : 0 })} ${units[unit]}`;
}

/** Marker colors as CSS backgrounds: warm amber, brass, steel and clay inks, never neon. */
export const PDF_MARK_COLORS = {
	yellow: '#E8C06A',
	green: '#BCC58C',
	blue: '#A8BFD2',
	pink: '#DCA997'
} as const;

const unit = (value: number) => Math.min(1, Math.max(0, value));

/** Most rectangles the server accepts for one mark. */
const MAX_MARK_RECTS = 200;

/**
 * Client rectangles of a text selection, as fractions of the page box. Each one is clipped to
 * the page and kept inside it (x + width <= 1), because the server refuses anything outside.
 * The browser returns one rectangle per text fragment, so fragments on the same line are merged
 * into one box; a long selection then stays under the server's limit.
 */
export function selectionBoxes(rects: ArrayLike<DOMRect>, page: DOMRect): { x: number; y: number; width: number; height: number }[] {
	if (page.width <= 0 || page.height <= 0) return [];
	const boxes: { x: number; y: number; width: number; height: number }[] = [];
	for (const rect of Array.from(rects)) {
		const x = unit((rect.left - page.left) / page.width);
		const y = unit((rect.top - page.top) / page.height);
		const width = Math.min(unit(rect.width / page.width), 1 - x);
		const height = Math.min(unit(rect.height / page.height), 1 - y);
		if (width > 0 && height > 0) boxes.push({ x, y, width, height });
	}
	boxes.sort((a, b) => a.y - b.y || a.x - b.x);
	const lines: typeof boxes = [];
	for (const box of boxes) {
		const last = lines.at(-1);
		// Same line: the tops are within half a line of each other.
		if (last && Math.abs(box.y - last.y) < Math.min(box.height, last.height) / 2) {
			const right = Math.max(last.x + last.width, box.x + box.width);
			const bottom = Math.max(last.y + last.height, box.y + box.height);
			last.x = Math.min(last.x, box.x);
			last.y = Math.min(last.y, box.y);
			last.width = right - last.x;
			last.height = bottom - last.y;
		} else {
			lines.push({ ...box });
		}
	}
	return lines.slice(0, MAX_MARK_RECTS);
}

/** Lowercase and without accents, the form in which a query is compared with page text. */
export function foldText(text: string): string {
	return text.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase();
}

/** True when `text` contains `query`, ignoring case and accents. An empty query matches nothing. */
export function containsText(text: string, query: string): boolean {
	const needle = foldText(query.trim());
	return needle !== '' && foldText(text).includes(needle);
}

/** 1-based numbers of the pages whose text contains the query, ignoring case and accents. */
export function pagesMatching(pageTexts: string[], query: string): number[] {
	return pageTexts.flatMap((text, index) => (containsText(text, query) ? [index + 1] : []));
}

/** Marks the text runs of a rendered page that contain the query (and clears the others). */
export function markHits(layer: HTMLElement | null, query: string): number {
	if (!layer) return 0;
	let hits = 0;
	for (const run of layer.querySelectorAll<HTMLElement>('span')) {
		const hit = containsText(run.textContent ?? '', query);
		run.classList.toggle('pdf-hit', hit);
		if (hit) hits += 1;
	}
	return hits;
}

/** A selection with its place, ready to paste: “text” — Title, p. 4. */
export function quoteWithPage(text: string, title: string, page: number): string {
	return `“${text}” — ${title.replace(/\.pdf$/i, '')}, p. ${page}`;
}

const KIND_NAMES = () => ({ highlight: tr('Highlight', 'Destaque'), underline: tr('Underline', 'Sublinhado'), note: tr('Note', 'Nota') });

/** Hide extraction containing invalid control bytes, without guessing the quote's language. */
export function sanitizeQuote(raw: string): string {
	// Broken ToUnicode maps can shift letters as well as spaces; deleting bytes cannot repair them.
	// eslint-disable-next-line no-control-regex -- these bytes identify corrupt PDF text extraction
	if (/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F]/.test(raw)) return '';
	return raw.replace(/\s+/g, ' ').trim().slice(0, 4000);
}

/** The marks as a Markdown document, in page order, so they can be kept outside the app. */
export function marksToMarkdown(title: string, items: PdfAnnotation[]): string {
	const names = KIND_NAMES();
	const sorted = [...items].sort((a, b) => a.page - b.page || a.createdAt - b.createdAt);
	const lines = [`# ${tr('Marks in', 'Marcações em')} ${title}`];
	let page = 0;
	for (const item of sorted) {
		if (item.page !== page) {
			page = item.page;
			lines.push('', `## ${tr('Page', 'Página')} ${page}`, '');
		}
		const quote = sanitizeQuote(item.quote);
		lines.push(`- **${names[item.kind]}**${quote ? `: “${quote}”` : ''}`);
		const note = item.content.trim();
		if (note) lines.push(...note.split('\n').map((line) => `  > ${line}`));
	}
	return `${lines.join('\n')}\n`;
}

/** One entry of a document's table of contents, in reading order. `page` is null when the target cannot be resolved. */
export type OutlineItem = { title: string; page: number | null; depth: number };

/** The few pdf.js calls the outline needs, so it can be read (and tested) without a real document. */
export type OutlineSource = {
	getOutline(): Promise<{ title: string; dest: string | unknown[] | null; items?: unknown[] }[] | null>;
	getDestination(id: string): Promise<unknown[] | null>;
	getPageIndex(ref: never): Promise<number>;
};

/** Deepest nesting and most entries kept: a hostile or huge outline cannot stall the reader. */
export const MAX_OUTLINE_DEPTH = 6;
export const MAX_OUTLINE_ITEMS = 1000;

async function outlinePage(doc: OutlineSource, dest: string | unknown[] | null): Promise<number | null> {
	try {
		const explicit = typeof dest === 'string' ? await doc.getDestination(dest) : dest;
		const target = explicit?.[0];
		if (typeof target === 'number') return target + 1;
		if (target && typeof target === 'object') return (await doc.getPageIndex(target as never)) + 1;
	} catch {
		// A broken destination leaves the title without a page rather than failing the whole outline.
	}
	return null;
}

/** Reads the table of contents as a flat list in reading order; empty when the document has none. */
export async function readOutline(doc: OutlineSource): Promise<OutlineItem[]> {
	const root = await doc.getOutline().catch(() => null);
	const items: OutlineItem[] = [];
	const walk = async (nodes: { title: string; dest: string | unknown[] | null; items?: unknown[] }[], depth: number): Promise<void> => {
		for (const node of nodes) {
			if (items.length >= MAX_OUTLINE_ITEMS) return;
			const title = (node.title ?? '').replace(/\s+/g, ' ').trim();
			if (title) items.push({ title, page: await outlinePage(doc, node.dest), depth });
			if (node.items?.length && depth + 1 < MAX_OUTLINE_DEPTH) await walk(node.items as typeof nodes, depth + 1);
		}
	};
	if (root) await walk(root, 0);
	return items;
}

/** Index of the heading that holds `page`: the last entry whose page is at or before it. */
export function activeOutlineIndex(items: OutlineItem[], page: number): number {
	let active = -1;
	items.forEach((item, index) => {
		if (item.page !== null && item.page <= page) active = index;
	});
	return active;
}

/** Highlight editor values for one mark, in PDF user space (points, origin at the bottom left of the page box). */
export function markToEditorValues(item: PdfAnnotation, view: [number, number, number, number], index: number) {
	const [x0, y0, x1, y1] = view;
	const [w, h] = [x1 - x0, y1 - y0];
	const hex = PDF_MARK_COLORS[item.color];
	const color = [1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16));
	return item.rects.map((rect, at) => {
		// An underline is a thin band at the foot of its line; there is no plain underline in pdf.js's editor.
		const top = item.kind === 'underline' ? rect.y + rect.height * 0.85 : rect.y;
		const height = item.kind === 'underline' ? rect.height * 0.15 : rect.height;
		const [left, right, upper, lower] = [x0 + rect.x * w, x0 + (rect.x + rect.width) * w, y1 - top * h, y1 - (top + height) * h];
		return {
			id: `pdfjs_internal_editor_${index}_${at}`,
			value: { annotationType: 9, color, opacity: item.kind === 'underline' ? 1 : 0.4, thickness: 12, quadPoints: [left, upper, right, upper, left, lower, right, lower], outlines: [[left, upper, right, upper, right, lower, left, lower]], pageIndex: item.page - 1, rect: [left, lower, right, upper], rotation: 0, structTreeParentId: null }
		};
	});
}

type SavableDocument = { annotationStorage: { setValue(key: string, value: unknown): void }; getPage(number: number): Promise<{ view: number[] }>; saveDocument(): Promise<Uint8Array> };

/** The PDF with the marks added as standard highlight annotations. The file on disk is untouched: pdf.js appends an incremental update to a copy in memory. */
export async function exportAnnotatedPdf(doc: SavableDocument, items: PdfAnnotation[]): Promise<Uint8Array> {
	const views = new Map<number, [number, number, number, number]>();
	let index = 0;
	for (const item of items) {
		if (!views.has(item.page)) views.set(item.page, (await doc.getPage(item.page)).view as [number, number, number, number]);
		for (const { id, value } of markToEditorValues(item, views.get(item.page)!, index++)) doc.annotationStorage.setValue(id, value);
	}
	return doc.saveDocument();
}
