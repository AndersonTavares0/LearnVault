import { describe, expect, it } from 'vitest';
import { clampPage, findLessonPdf, formatFileSize, isPdf, lessonPdfId, stepZoom } from './pdf';
import type { FileEntry, Lesson } from './transport';

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
