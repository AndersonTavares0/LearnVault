import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fakePdfjs } from '@/test/fake-pdfjs';
import { createDefaultPlayerPreferences } from '@/lib/player-preferences';
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

const open = async (props: Partial<{ initialPage: number | null; onPage: (page: number, pages: number) => void; onPreferencesChange: (next: unknown) => void }> = {}) => {
	const onPage = props.onPage ?? vi.fn();
	const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
	render(<QueryClientProvider client={client}><PdfReader file={file} courseId="c" initialPage={props.initialPage ?? null} onPage={onPage} preferences={createDefaultPlayerPreferences()} onPreferencesChange={props.onPreferencesChange ?? vi.fn()} /></QueryClientProvider>);
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

	it('moves a page with j and k, jumps with g and G and opens the shortcut sheet with ?', async () => {
		await open();
		const reader = screen.getByRole('group', { name: 'PDF page' });
		fireEvent.keyDown(reader, { key: 'j' });
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('2');
		fireEvent.keyDown(reader, { key: 'k' });
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('1');
		fireEvent.keyDown(reader, { key: 'G' });
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('5');
		fireEvent.keyDown(reader, { key: 'g' });
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('1');
		fireEvent.keyDown(reader, { key: '?' });
		expect(await screen.findByRole('dialog')).toBeTruthy();
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
		fireEvent.click(screen.getByRole('button', { name: 'Annotate' }));
		expect((screen.getByRole('button', { name: 'Highlight' }) as HTMLButtonElement).disabled).toBe(true);
		expect((screen.getByRole('button', { name: 'Underline' }) as HTMLButtonElement).disabled).toBe(true);
		expect(await screen.findByText(/No marks yet/)).toBeTruthy();
	});

	it('searches the text of every page and steps between the pages that match', async () => {
		vi.spyOn(transport, 'listPdfAnnotations').mockResolvedValue([]);
		await open();
		fireEvent.click(screen.getByRole('button', { name: 'Find in the document' }));
		fireEvent.change(screen.getByRole('searchbox', { name: 'Search in the document' }), { target: { value: 'OWL' } });
		fireEvent.click(screen.getByRole('button', { name: 'Search' }));
		expect(await screen.findByText('Found on 1 page: now on page 3.')).toBeTruthy();
		expect((screen.getByLabelText('Page') as HTMLInputElement).value).toBe('3');
		fireEvent.change(screen.getByRole('searchbox', { name: 'Search in the document' }), { target: { value: 'dragon' } });
		fireEvent.click(screen.getByRole('button', { name: 'Search' }));
		expect(await screen.findByText('Nothing found for “dragon”.')).toBeTruthy();
	});

	it('edits the note on a mark when the field loses focus, and changes its colour', async () => {
		vi.spyOn(transport, 'listPdfAnnotations').mockResolvedValue([mark('a1', 2)]);
		const update = vi.spyOn(transport, 'updatePdfAnnotation').mockResolvedValue(mark('a1', 2));
		await open();
		const field = await screen.findByRole('textbox', { name: 'Note for the mark on page 2' });
		fireEvent.change(field, { target: { value: 'revisit this' } });
		fireEvent.blur(field);
		await waitFor(() => expect(update).toHaveBeenCalledWith('a1', { content: 'revisit this' }));
		fireEvent.click(within(screen.getByRole('radiogroup', { name: 'Color of the mark on page 2' })).getByRole('radio', { name: 'Pink' }));
		await waitFor(() => expect(update).toHaveBeenCalledWith('a1', { color: 'pink' }));
	});

	it('shows only the marks of the current page when asked', async () => {
		vi.spyOn(transport, 'listPdfAnnotations').mockResolvedValue([mark('a1', 4), mark('a2', 2)]);
		await open();
		await screen.findByText('quote a1');
		fireEvent.click(screen.getByRole('button', { name: /^This page/ }));
		expect(await screen.findByText('No marks on this page.')).toBeTruthy();
		expect(screen.queryByText('quote a1')).toBeNull();
	});

	it('asks for the text of a note before saving it, then keeps it out of the PDF', async () => {
		vi.spyOn(transport, 'listPdfAnnotations').mockResolvedValue([]);
		const create = vi.spyOn(transport, 'createPdfAnnotation').mockResolvedValue({ ...mark('n1', 1), kind: 'note', content: 'why?' });
		const box = { left: 10, top: 10, width: 100, height: 10 } as DOMRect;
		const range = { collapsed: false, commonAncestorContainer: null, getClientRects: () => [box], toString: () => 'some text' };
		await open();
		fireEvent.click(screen.getByRole('button', { name: 'Annotate' }));
		const layer = document.querySelector('.pdf-text-layer') as HTMLElement;
		const selection = { rangeCount: 1, getRangeAt: () => ({ ...range, commonAncestorContainer: layer }), removeAllRanges: vi.fn() };
		vi.spyOn(window, 'getSelection').mockReturnValue(selection as unknown as Selection);
		vi.spyOn(HTMLCanvasElement.prototype, 'getBoundingClientRect').mockReturnValue({ left: 0, top: 0, width: 600, height: 800 } as DOMRect);
		document.dispatchEvent(new Event('selectionchange'));
		const add = await screen.findByRole('button', { name: 'Add note' });
		await waitFor(() => expect((add as HTMLButtonElement).disabled).toBe(false));
		fireEvent.click(add);
		const save = screen.getByRole('button', { name: 'Save note' }) as HTMLButtonElement;
		expect(save.disabled).toBe(true);
		fireEvent.change(screen.getByLabelText('Note for page 1'), { target: { value: ' why? ' } });
		fireEvent.click(save);
		await waitFor(() => expect(create).toHaveBeenCalledWith('reading', expect.objectContaining({ kind: 'note', content: 'why?', quote: 'some text', page: 1 })));
	});
});
