import { describe, expect, it } from 'vitest';
import { clampPage, findLessonPdf, formatFileSize, isPdf, lessonPdfId, marksToMarkdown, pagesMatching, sanitizeQuote, selectionBoxes, stepZoom } from './pdf';
import type { FileEntry, Lesson, PdfAnnotation } from './transport';

const file = (name: string, extra: Partial<FileEntry> = {}): FileEntry => ({ name, path: `c/${name}`, size: 1, mediaId: name, kind: 'material', availability: 'available', ...extra });
const lesson = (media: FileEntry[], materials: FileEntry[]): Lesson => ({ title: 'L', path: 'c/L', media, subtitles: [], materials, moduleTitle: null } as unknown as Lesson);

describe('pdf lessons', () => {
	it('recognizes PDFs by extension, ignoring case', () => {
		expect(isPdf({ name: 'Notes.PDF' })).toBe(true);
		expect(isPdf({ name: 'notes.pdf.txt' })).toBe(false);
	});

	it('treats a lesson with nothing to play and a readable PDF as a PDF lesson', () => {
		const pdf = file('reading.pdf');
		expect(findLessonPdf(lesson([], [file('slides.pptx'), pdf]))).toBe(pdf);
	});

	it('keeps the player when the lesson has media, and skips missing PDFs', () => {
		expect(findLessonPdf(lesson([file('v.mp4', { kind: 'media' })], [file('reading.pdf')]))).toBeNull();
		expect(findLessonPdf(lesson([], [file('gone.pdf', { availability: 'missing' })]))).toBeNull();
	});
});

describe('pdf navigation', () => {
	it('clamps the page into the document', () => {
		expect(clampPage(0, 10)).toBe(1);
		expect(clampPage(99, 10)).toBe(10);
		expect(clampPage(Number.NaN, 10)).toBe(1);
		expect(clampPage(3.6, 10)).toBe(4);
	});

	it('steps zoom through fixed levels and stops at the ends', () => {
		expect(stepZoom(1, 1)).toBe(1.25);
		expect(stepZoom(1, -1)).toBe(0.75);
		expect(stepZoom(3, 1)).toBe(3);
		expect(stepZoom(0.5, -1)).toBe(0.5);
		expect(stepZoom('fit', 1)).toBe(1.25);
	});
});

describe('file size', () => {
	it('shows a short size with the right unit', () => {
		expect(formatFileSize(512)).toBe('512 B');
		expect(formatFileSize(1_258_291)).toBe('1.2 MB');
		expect(formatFileSize(48 * 1024)).toBe('48 KB');
		expect(formatFileSize(Number.NaN)).toBe('0 B');
	});
});

describe('pdf progress id', () => {
	it('uses the first readable PDF of a lesson with no media', () => {
		expect(lessonPdfId({ files: [file('a.pdf'), file('b.pdf')], looseFiles: [] })).toBe('a.pdf');
		expect(lessonPdfId({ files: [file('gone.pdf', { availability: 'missing' })], looseFiles: [file('b.pdf')] })).toBe('b.pdf');
	});

	it('leaves a lesson with media to its player', () => {
		expect(lessonPdfId({ files: [file('v.mp4', { kind: 'media' }), file('a.pdf')], looseFiles: [] })).toBeNull();
	});
});

describe('selection boxes', () => {
	const page = { left: 100, top: 50, width: 400, height: 800 } as DOMRect;
	const box = (left: number, top: number, width: number, height: number) => ({ left, top, width, height, right: left + width, bottom: top + height } as DOMRect);

	it('turns client rectangles into fractions of the page', () => {
		expect(selectionBoxes([box(200, 130, 100, 40)], page)).toEqual([{ x: 0.25, y: 0.1, width: 0.25, height: 0.05 }]);
	});

	it('clips a rectangle that runs past the page edge and drops empty ones', () => {
		const [clipped] = selectionBoxes([box(450, 130, 200, 40)], page);
		expect(clipped.x + clipped.width).toBeLessThanOrEqual(1);
		expect(selectionBoxes([box(600, 130, 10, 40)], page)).toEqual([]);
		expect(selectionBoxes([box(200, 130, 0, 40)], page)).toEqual([]);
	});

	it('merges fragments on one line and keeps a long selection under the server limit', () => {
		const line = selectionBoxes([box(200, 130, 40, 20), box(260, 131, 40, 20), box(200, 170, 80, 20)], page);
		expect(line).toHaveLength(2);
		expect(line[0].x).toBeCloseTo(0.25);
		expect(line[0].width).toBeCloseTo(0.25);
		expect(line[0].height).toBeCloseTo(0.02625);
		const many = selectionBoxes(Array.from({ length: 900 }, (_, index) => box(120 + (index % 3) * 50, 60 + Math.floor(index / 3) * 2.5, 40, 2)), page);
		expect(many.length).toBeLessThanOrEqual(200);
	});

	it('returns nothing for a page with no size', () => {
		expect(selectionBoxes([box(200, 130, 100, 40)], { ...page, width: 0 } as DOMRect)).toEqual([]);
	});
});

describe('PDF quote extraction', () => {
	it('rejects broken text maps rather than pretending byte removal repairs the quote', () => {
		expect(sanitizeQuote('HUD\u0003GR\u0003/HVWH\u000f\u0003(V')).toBe('');
		expect(sanitizeQuote('text\u0000text')).toBe('');
		expect(sanitizeQuote('text\u0085text')).toBe('');
	});

	it.each(['HTTP CSS XML', '中文 日本語', 'Привет мир', 'مرحبا بالعالم', 'می\u200cروم', 'ação e educação', 'x + y = 2'])('preserves valid text: %s', (text) => {
		expect(sanitizeQuote(text)).toBe(text);
	});

	it('normalizes layout whitespace and keeps the existing quote length limit', () => {
		expect(sanitizeQuote(' \tfirst\r\nsecond  ')).toBe('first second');
		expect(sanitizeQuote('   ')).toBe('');
		expect(sanitizeQuote('a'.repeat(5000))).toHaveLength(4000);
	});
});

describe('pagesMatching and marksToMarkdown', () => {
	it('finds pages by text, ignoring case and blank queries', () => {
		expect(pagesMatching(['The Owl', 'nothing', 'an OWL again'], ' owl ')).toEqual([1, 3]);
		expect(pagesMatching(['a', 'b'], '  ')).toEqual([]);
	});

	it('writes the marks in page order with their notes', () => {
		const item = (annotationId: string, page: number, kind: 'highlight' | 'note', quote: string, content: string): PdfAnnotation => ({ annotationId, mediaId: 'm', page, kind, color: 'yellow', rects: [], quote, content, createdAt: page, updatedAt: page });
		const text = marksToMarkdown('Guide.pdf', [item('b', 4, 'note', 'late', 'check this\nand that'), item('a', 2, 'highlight', 'early  text', '')]);
		expect(text).toBe('# Marks in Guide.pdf\n\n## Page 2\n\n- **Highlight**: “early text”\n\n## Page 4\n\n- **Note**: “late”\n  > check this\n  > and that\n');
	});

	it('exports a corrupt quote with its page and user note intact', () => {
		const item: PdfAnnotation = { annotationId: 'a', mediaId: 'm', page: 2, kind: 'highlight', color: 'yellow', rects: [], quote: 'HUD\u0003GR', content: 'Keep my note', createdAt: 1, updatedAt: 1 };
		expect(marksToMarkdown('Guide.pdf', [item])).toBe('# Marks in Guide.pdf\n\n## Page 2\n\n- **Highlight**\n  > Keep my note\n');
		expect(item.quote).toBe('HUD\u0003GR');
	});
});
