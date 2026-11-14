import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CourseManagement } from '@/components/course-management';
import { transport, type CatalogNode } from '@/lib/transport';

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

const course = (coverVersion?: number): CatalogNode => ({ title: 'Rust from Zero', path: 'rust', role: 'course', courseId: 'c1', coverVersion, files: [], looseFiles: [], children: [] });

function show(node = course(), onRemoved?: () => void) {
	render(
		<QueryClientProvider client={new QueryClient({ defaultOptions: { mutations: { retry: false } } })}>
			<MemoryRouter initialEntries={['/course']}>
				<Routes>
					<Route path="/course" element={<CourseManagement course={node} courseId="c1" lessons={4} compact onRemoved={onRemoved} />} />
					<Route path="/" element={<p>Library home</p>} />
				</Routes>
			</MemoryRouter>
		</QueryClientProvider>
	);
}

const choose = async (name: string) => {
	fireEvent.click(screen.getByRole('button', { name: 'Manage Rust from Zero' }));
	fireEvent.click(await screen.findByRole('menuitem', { name }));
};

describe('CourseManagement', () => {
	it('removes the course only after confirmation and returns to the library', async () => {
		const remove = vi.spyOn(transport, 'removeCourse').mockResolvedValue();
		show();
		await choose('Remove course');
		const dialog = await screen.findByRole('alertdialog');
		expect(remove).not.toHaveBeenCalled();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Remove course' }));
		await waitFor(() => expect(remove).toHaveBeenCalledWith('c1'));
		expect(await screen.findByText('Library home')).toBeTruthy();
	});

	it('uses onRemoved instead of navigating when the caller provides it', async () => {
		vi.spyOn(transport, 'removeCourse').mockResolvedValue();
		const onRemoved = vi.fn();
		show(course(), onRemoved);
		await choose('Remove course');
		fireEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Remove course' }));
		await waitFor(() => expect(onRemoved).toHaveBeenCalled());
		expect(screen.queryByText('Library home')).toBeNull();
	});

	it('keeps the dialog open with a localized message when removal fails', async () => {
		vi.spyOn(transport, 'removeCourse').mockRejectedValue(new Error('EIO /srv/secret/path'));
		show();
		await choose('Remove course');
		fireEvent.click(within(await screen.findByRole('alertdialog')).getByRole('button', { name: 'Remove course' }));
		expect(await screen.findByText('Could not remove the course.')).toBeTruthy();
		expect(screen.queryByText(/secret/)).toBeNull();
		expect(screen.getByRole('alertdialog')).toBeTruthy();
	});

	it('explains a forced password change instead of a generic failure', async () => {
		vi.spyOn(transport, 'resetCourseOrder').mockRejectedValue(new Error('credentials_change_required'));
		show();
		await choose('Restore lesson order');
		expect(await screen.findByText('Change your password on the server before managing courses.')).toBeTruthy();
	});

	it('reports whether the lesson order changed', async () => {
		const reset = vi.spyOn(transport, 'resetCourseOrder').mockResolvedValueOnce(true).mockResolvedValueOnce(false);
		show();
		await choose('Restore lesson order');
		expect(await screen.findByText('Lesson order restored.')).toBeTruthy();
		await choose('Restore lesson order');
		expect(await screen.findByText('Lesson order was already at its default.')).toBeTruthy();
		expect(reset).toHaveBeenCalledWith('c1');
	});

	it('offers Remove cover only when a cover exists', async () => {
		show(course());
		fireEvent.click(screen.getByRole('button', { name: 'Manage Rust from Zero' }));
		expect(await screen.findByRole('menuitem', { name: 'Add cover' })).toBeTruthy();
		expect(screen.queryByRole('menuitem', { name: 'Remove cover' })).toBeNull();
		cleanup();
		show(course(3));
		fireEvent.click(screen.getByRole('button', { name: 'Manage Rust from Zero' }));
		expect(await screen.findByRole('menuitem', { name: 'Change cover' })).toBeTruthy();
		expect(screen.getByRole('menuitem', { name: 'Remove cover' })).toBeTruthy();
	});

	it('removes a custom cover', async () => {
		const removeCover = vi.spyOn(transport, 'removeCourseCover').mockResolvedValue(true);
		show(course(3));
		await choose('Remove cover');
		expect(await screen.findByText('Cover removed.')).toBeTruthy();
		expect(removeCover).toHaveBeenCalledWith('c1');
	});

	it('uploads a valid image and rejects a wrong type or an oversized file before sending', async () => {
		const upload = vi.spyOn(transport, 'importCourseCover').mockResolvedValue(true as never);
		show();
		const input = document.querySelector('input[type="file"]') as HTMLInputElement;
		const pick = (file: File) => fireEvent.change(input, { target: { files: [file] } });

		pick(new File(['x'], 'cover.gif', { type: 'image/gif' }));
		expect(await screen.findByText('Choose a PNG, JPEG or WebP image.')).toBeTruthy();

		const big = new File(['x'], 'big.png', { type: 'image/png' });
		Object.defineProperty(big, 'size', { value: 4 * 1024 * 1024 + 1 });
		pick(big);
		expect(await screen.findByText('The image is larger than 4 MiB.')).toBeTruthy();
		expect(upload).not.toHaveBeenCalled();

		const ok = new File(['x'], 'cover.png', { type: 'image/png' });
		pick(ok);
		await waitFor(() => expect(upload).toHaveBeenCalledWith('c1', ok));
		expect(await screen.findByText('Cover updated.')).toBeTruthy();
	});
});
