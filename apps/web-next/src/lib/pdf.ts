import { getLocale } from './i18n';
import type { FileEntry, Lesson } from './transport';

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
