import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fakePdfjs } from '@/test/fake-pdfjs';
import { transport, type Catalog, type CatalogNode, type FileEntry, type StudyProgress } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

vi.mock('pdfjs-dist', () => fakePdfjs(5));
vi.mock('pdfjs-dist/build/pdf.worker.min.mjs?url', () => ({ default: '/worker.js' }));

const pdf: FileEntry = { name: 'reading.pdf', path: 'c/reading.pdf', size: 1_258_291, mediaId: 'reading', kind: 'material', availability: 'available' };
const video: FileEntry = { name: 'one.mp4', path: 'c/one.mp4', size: 1, mediaId: 'one', kind: 'media', availability: 'available' };
const lesson = (title: string, files: FileEntry[]): CatalogNode => ({ title, path: `c/${title}`, role: 'lesson', files, looseFiles: [], children: [] });
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [{ title: 'Course', path: 'c', role: 'course', courseId: 'c', files: [], looseFiles: [], children: [lesson('Reading', [pdf]), lesson('Video', [video, { ...pdf, name: 'slides.pdf', path: 'c/slides.pdf', mediaId: 'slides' }])] }] };
const progress = (mediaId: string, positionSeconds = 0): StudyProgress => ({ mediaId, positionSeconds, completed: false, updatedAt: 1, recorded: positionSeconds > 0 });

beforeEach(() => {
	localStorage.clear();
	vi.spyOn(transport, 'mediaCapabilities').mockResolvedValue({ transcoding: false });
	vi.spyOn(transport, 'studyProgress').mockImplementation(async (id) => progress(id, id === 'reading' ? 3 : 0));
	vi.spyOn(transport, 'listNotes').mockResolvedValue([]);
	vi.spyOn(transport, 'listBookmarks').mockResolvedValue([]);
	vi.spyOn(transport, 'listFlashcards').mockResolvedValue([]);
	vi.spyOn(transport, 'rewardSummary').mockRejectedValue(new Error('not needed'));
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('PDF lesson', () => {
	it('says it is a PDF, reads it in the page and resumes on the saved page', async () => {
		renderApp('/course/c/c%2FReading', { catalog });
		expect(await screen.findByText('PDF document')).toBeTruthy();
		expect(screen.getByText('1.2 MB')).toBeTruthy();
		expect(screen.getByRole('note').textContent).toContain('the original file is never changed');
		expect((await screen.findByLabelText('Page') as HTMLInputElement).value).toBe('3');
		expect(screen.queryByLabelText(/Lesson player/)).toBeNull();
	});

	it('keeps cards but drops timed notes and the note shortcut', async () => {
		renderApp('/course/c/c%2FReading', { catalog });
		await screen.findByText('PDF document');
		expect(screen.getByRole('tab', { name: 'Cards' })).toBeTruthy();
		expect(screen.queryByRole('tab', { name: 'Notes' })).toBeNull();
		expect(screen.queryByRole('button', { name: /^Note/ })).toBeNull();
		fireEvent.click(screen.getByRole('button', { name: 'Card' }));
		expect(await screen.findByRole('form', { name: 'New card' })).toBeTruthy();
	});

	it('keeps the player for a lesson with media and leaves its PDF as a material', async () => {
		renderApp('/course/c/c%2FVideo', { catalog });
		expect(await screen.findByLabelText('Lesson player: Video')).toBeTruthy();
		expect(screen.queryByText('PDF document')).toBeNull();
	});
});
