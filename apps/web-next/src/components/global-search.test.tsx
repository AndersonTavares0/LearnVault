import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { transport, type SearchResults } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const results: SearchResults = {
	notes: [{
		noteId: 'n1',
		mediaId: 'own',
		courseId: 'rust',
		courseTitle: 'Rust',
		lessonPath: 'r/own.mp4',
		lessonTitle: 'Ownership',
		positionSeconds: 75,
		updatedAt: 1_794_200_000,
		snippet: 'the <mark>borrow</mark> checker rules'
	}],
	courses: [{ courseId: 'rust', title: 'Rust' }],
	lessons: [{ courseId: 'rust', courseTitle: 'Rust', mediaId: 'own', path: 'r/own.mp4', title: 'Ownership' }]
};

const searchbox = () => screen.getByRole('combobox', { name: 'Search notes, courses and lessons' });

beforeEach(() => {
	vi.spyOn(transport, 'search').mockResolvedValue(results);
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	vi.unstubAllGlobals();
});

describe('global search', () => {
	it('groups notes, courses and lessons and opens a note at its timestamp', async () => {
		const { router } = renderApp('/', {});
		fireEvent.change(searchbox(), { target: { value: 'borrow' } });

		expect(await screen.findByRole('group', { name: 'Notes' })).toBeTruthy();
		expect(screen.getByRole('group', { name: 'Courses' })).toBeTruthy();
		expect(screen.getByRole('group', { name: 'Lessons' })).toBeTruthy();
		const highlighted = screen.getByText('borrow');
		expect(highlighted.tagName).toBe('MARK');
		expect(screen.getByText(/checker rules/)).toBeTruthy();

		fireEvent.keyDown(searchbox(), { key: 'ArrowDown' });
		fireEvent.keyDown(searchbox(), { key: 'Enter' });

		await waitFor(() => expect(router.state.location.pathname).toBe('/course/rust/r%2Fown.mp4'));
		expect(router.state.location.search).toBe('?t=75');
	});

	it('opens course and lesson results from their options', async () => {
		const { router } = renderApp('/', {});
		fireEvent.change(searchbox(), { target: { value: 'own' } });
		const options = await screen.findAllByRole('option');
		fireEvent.click(options.find((option) => option.textContent === 'RustCourse')!);
		await waitFor(() => expect(router.state.location.pathname).toBe('/course/rust'));

		fireEvent.change(searchbox(), { target: { value: 'own' } });
		const lessons = await screen.findAllByRole('option');
		fireEvent.click(lessons.find((option) => option.textContent === 'OwnershipRust')!);
		await waitFor(() => expect(router.state.location.pathname).toBe('/course/rust/r%2Fown.mp4'));
	});

	it('focuses from / and Ctrl+K and closes on Escape', async () => {
		renderApp('/', {});
		fireEvent.keyDown(window, { key: '/' });
		expect(document.activeElement).toBe(searchbox());
		fireEvent.keyDown(searchbox(), { key: 'Escape' });
		expect(document.activeElement).not.toBe(searchbox());

		fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
		expect(document.activeElement).toBe(searchbox());
	});

	it('opens the same palette from the mobile tab bar tab', async () => {
		renderApp('/', {});
		fireEvent.click(screen.getByRole('button', { name: 'Search' }));
		expect(document.activeElement).toBe(searchbox());
		fireEvent.change(searchbox(), { target: { value: 'borrow' } });
		expect(await screen.findByRole('group', { name: 'Notes' })).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Close search' }));
		expect(document.activeElement).not.toBe(searchbox());
		expect((searchbox() as HTMLInputElement).value).toBe('');
	});

	it('shows loading, empty and error states', async () => {
		vi.mocked(transport.search).mockReturnValue(new Promise(() => {}));
		renderApp('/', {});
		fireEvent.change(searchbox(), { target: { value: 'ru' } });
		expect(await screen.findByText('Searching…')).toBeTruthy();
		cleanup();
		vi.mocked(transport.search).mockResolvedValue({ notes: [], courses: [], lessons: [] });
		renderApp('/', {});
		fireEvent.change(searchbox(), { target: { value: 'zz' } });
		expect(await screen.findByText(/No results for/)).toBeTruthy();

		cleanup();
		vi.mocked(transport.search).mockRejectedValue(new Error('offline'));
		renderApp('/', {});
		fireEvent.change(searchbox(), { target: { value: 'ru' } });
		expect(await screen.findByRole('alert')).toBeTruthy();
	});

	it('does not search before two characters', async () => {
		renderApp('/', {});
		fireEvent.change(searchbox(), { target: { value: 'r' } });
		await waitFor(() => expect(transport.search).not.toHaveBeenCalled());
	});

	it('opens the full-screen phone palette from / when the rail field is hidden', async () => {
		vi.stubGlobal('matchMedia', vi.fn(() => ({ matches: false })));
		renderApp('/', {});
		fireEvent.keyDown(window, { key: '/' });
		expect(document.activeElement).toBe(searchbox());
		expect(searchbox().getAttribute('placeholder')).toBe('Search courses, lessons and notes');
	});

	it('returns focus to the tab bar Search tab when the phone palette is dismissed from its close button', async () => {
		renderApp('/', {});
		const tab = screen.getByRole('button', { name: 'Search' });
		tab.focus();
		fireEvent.click(tab);
		expect(document.activeElement).toBe(searchbox());
		fireEvent.keyDown(screen.getByRole('button', { name: 'Close search' }), { key: 'Escape' });
		expect(document.activeElement).toBe(tab);
		expect(searchbox().getAttribute('placeholder')).toBe('Search');
	});

	it('keeps Tab inside the phone palette', async () => {
		renderApp('/', {});
		fireEvent.click(screen.getByRole('button', { name: 'Search' }));
		const close = screen.getByRole('button', { name: 'Close search' });
		close.focus();
		fireEvent.keyDown(close, { key: 'Tab' });
		expect(document.activeElement).toBe(searchbox());
		fireEvent.keyDown(searchbox(), { key: 'Tab', shiftKey: true });
		expect(document.activeElement).toBe(close);
	});

	it('renders flashcards and marks groups and scrolls active option into view on ArrowDown', async () => {
		vi.mocked(transport.search).mockResolvedValue({
			...results,
			flashcards: [{
				cardId: 'c1',
				mediaId: 'own',
				courseId: 'rust',
				courseTitle: 'Rust',
				lessonPath: 'r/own.mp4',
				lessonTitle: 'Ownership',
				positionSeconds: 42,
				snippet: 'Question <mark>borrow</mark>'
			}],
			bookmarks: [{
				bookmarkId: 'b1',
				kind: 'pdf',
				mediaId: 'pdf1',
				courseId: 'rust',
				courseTitle: 'Rust',
				lessonPath: 'r/book.pdf',
				lessonTitle: 'Handbook',
				positionSeconds: 7,
				snippet: '<mark>borrow</mark> rules p.7'
			}]
		});
		const scrollSpy = vi.fn();
		Element.prototype.scrollIntoView = scrollSpy;
		renderApp('/', {});
		fireEvent.change(searchbox(), { target: { value: 'borrow' } });
		expect(await screen.findByRole('group', { name: 'Flashcards' })).toBeTruthy();
		expect(screen.getByRole('group', { name: 'Marks' })).toBeTruthy();
		expect(screen.getByText('Rust · Page 7')).toBeTruthy();
		fireEvent.keyDown(searchbox(), { key: 'ArrowDown' });
		expect(scrollSpy).toHaveBeenCalledWith({ block: 'nearest' });
	});
});
