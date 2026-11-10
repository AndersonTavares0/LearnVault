import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fakePdfjs } from '@/test/fake-pdfjs';
import { transport, type FileEntry, type PdfAnnotation } from '@/lib/transport';
import { PdfReader } from './pdf-reader';

vi.mock('pdfjs-dist', () => fakePdfjs(5));
vi.mock('pdfjs-dist/build/pdf.worker.min.mjs?url', () => ({ default: '/worker.js' }));

const file: FileEntry = { name: 'reading.pdf', path: 'c/reading.pdf', size: 1000, mediaId: 'reading', kind: 'material', availability: 'available' };
const mark = (id: string, page: number): PdfAnnotation => ({ annotationId: id, mediaId: 'reading', page, kind: 'highlight', color: 'green', rects: [{ x: 0.1, y: 0.1, width: 0.2, height: 0.02 }], quote: `quote ${id}`, content: '', createdAt: 1, updatedAt: 1 });

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

const open = async (props: Partial<{ initialPage: number | null; onPage: (page: number, pages: number) => void }> = {}) => {
	const onPage = props.onPage ?? vi.fn();
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	render(<QueryClientProvider client={client}><PdfReader file={file} courseId="c" initialPage={props.initialPage ?? null} onPage={onPage} /></QueryClientProvider>);
	await screen.findByText('of 5');
	return onPage;
};

describe('PdfReader', () => {
	it('opens on the first page and reports each page it moves to', async () => {
		const onPage = await open();
		await waitFor(() => expect(onPage).toHaveBeenCalledWith(1, 5));
		fireEvent.click(screen.getByRole('button', { name: 'Next page' }));
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('2');
		expect(onPage).toHaveBeenLastCalledWith(2, 5);
		fireEvent.click(screen.getByRole('button', { name: 'Previous page' }));
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('1');
	});

	it('starts on the saved page and keeps typed pages inside the document', async () => {
		await open({ initialPage: 4 });
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('4');
		fireEvent.change(screen.getByLabelText('Page'), { target: { value: '99' } });
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('5');
		expect((screen.getByRole('button', { name: 'Next page' }) as HTMLButtonElement).disabled).toBe(true);
	});

	it('moves with the keyboard and changes zoom', async () => {
		await open();
		const reader = screen.getByRole('group', { name: 'PDF page' });
		fireEvent.keyDown(reader, { key: 'End' });
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('5');
		fireEvent.keyDown(reader, { key: 'Home' });
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('1');
		expect(screen.getByText('Fit')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Zoom in' }));
		expect(screen.getByText('125%')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Fit to width' }));
		expect(screen.getByText('Fit')).toBeTruthy();
	});

	it('lists the marks, jumps to their page and deletes one', async () => {
		vi.spyOn(transport, 'listPdfAnnotations').mockResolvedValue([mark('a1', 4), mark('a2', 2)]);
		const remove = vi.spyOn(transport, 'deletePdfAnnotation').mockResolvedValue(undefined);
		await open();
		expect(await screen.findByText('quote a2')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Page 4' }));
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('4');
		fireEvent.click(screen.getByRole('button', { name: 'Delete mark on page 2' }));
		await waitFor(() => expect(remove).toHaveBeenCalledWith('a2'));
	});

	it('keeps the mark buttons off until text is selected', async () => {
		vi.spyOn(transport, 'listPdfAnnotations').mockResolvedValue([]);
		await open();
		expect((screen.getByRole('button', { name: 'Highlight' }) as HTMLButtonElement).disabled).toBe(true);
		expect((screen.getByRole('button', { name: 'Underline' }) as HTMLButtonElement).disabled).toBe(true);
		expect(await screen.findByText(/No marks yet/)).toBeTruthy();
	});
});
