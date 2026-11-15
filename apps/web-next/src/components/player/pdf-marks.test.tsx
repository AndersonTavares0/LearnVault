import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { PdfMarks } from '@/components/player/pdf-marks';
import type { PdfAnnotation } from '@/lib/transport';

afterEach(cleanup);

const mark = (id: string, page: number, quote: string): PdfAnnotation => ({
	annotationId: id, mediaId: 'm1', page, kind: 'highlight', color: 'yellow', rects: [{ x: 0.1, y: 0.1, width: 0.5, height: 0.05 }], quote, content: '', createdAt: page, updatedAt: page
});

function show(items = [mark('a1', 2, 'Ownership moves values.'), mark('a2', 5, 'Borrowing is explicit.')]) {
	const handlers = { onGo: vi.fn(), onChange: vi.fn(), onDelete: vi.fn(), onRestore: vi.fn(), onSendToNotes: vi.fn(), onExportPdf: vi.fn() };
	render(<PdfMarks title="Notes.pdf" items={items} page={1} pending={false} {...handlers} />);
	return handlers;
}

describe('PdfMarks', () => {
	it('asks before deleting and deletes nothing when cancelled', async () => {
		const { onDelete } = show();
		fireEvent.click(screen.getByRole('button', { name: 'Delete mark on page 2' }));
		const dialog = await screen.findByRole('alertdialog', { name: 'Delete this mark?' });
		expect(onDelete).not.toHaveBeenCalled();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
		expect(onDelete).not.toHaveBeenCalled();
		expect(screen.queryByText(/deleted\./)).toBeNull();
	});

	it('deletes after confirmation and offers to put the mark back', async () => {
		const items = [mark('a1', 2, 'Ownership moves values.')];
		const { onDelete, onRestore } = show(items);
		fireEvent.click(screen.getByRole('button', { name: 'Delete mark on page 2' }));
		fireEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Delete mark' }));
		expect(onDelete).toHaveBeenCalledWith('a1');

		const notice = await screen.findByText('Mark on page 2 deleted.');
		expect(notice).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Undo' }));
		expect(onRestore).toHaveBeenCalledWith(items[0]);
		expect(screen.queryByText('Mark on page 2 deleted.')).toBeNull();
	});

	it('only the last deleted mark can be put back', async () => {
		const { onRestore } = show();
		for (const page of [2, 5]) {
			fireEvent.click(screen.getByRole('button', { name: `Delete mark on page ${page}` }));
			fireEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Delete mark' }));
		}
		expect(screen.queryByText('Mark on page 2 deleted.')).toBeNull();
		fireEvent.click(await screen.findByRole('button', { name: 'Undo' }));
		expect(onRestore).toHaveBeenCalledTimes(1);
		expect(onRestore.mock.calls[0][0].annotationId).toBe('a2');
	});

	it('names the mark when a jump to its page is asked for', () => {
		const { onGo } = show();
		fireEvent.click(screen.getByRole('button', { name: 'Page 5' }));
		expect(onGo).toHaveBeenCalledWith(5, 'a2');
	});

	it('sends a mark to the notes and exports the annotated PDF', () => {
		const { onSendToNotes, onExportPdf } = show();
		fireEvent.click(screen.getByRole('button', { name: 'Send mark on page 2 to notes' }));
		expect(onSendToNotes).toHaveBeenCalledWith(expect.objectContaining({ annotationId: 'a1' }));
		fireEvent.click(screen.getByRole('button', { name: 'Save PDF with marks' }));
		expect(onExportPdf).toHaveBeenCalled();
	});
});
