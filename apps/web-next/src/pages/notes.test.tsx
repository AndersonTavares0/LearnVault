import { cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { pomodoro } from '@/lib/pomodoro-store';
import { transport, type Catalog, type StudyNote } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const catalog: Catalog = {
	complete: true,
	looseFiles: [],
	nodes: [{ title: 'Rust', path: 'r', role: 'course', courseId: 'rust', files: [], looseFiles: [], children: [
		{ title: 'Ownership', path: 'r/own', role: 'lesson', files: [{ name: 'own.mp4', path: 'r/own.mp4', size: 1, mediaId: 'own', kind: 'media', availability: 'available' }], looseFiles: [], children: [] }
	] }]
};
const note = (noteId: string, content: string, mediaId = 'own'): StudyNote => ({ noteId, mediaId, positionSeconds: 75, content, createdAt: 1, updatedAt: 1_794_200_000 });

beforeEach(() => {
	vi.spyOn(transport, 'listNotes').mockResolvedValue([note('n1', 'Borrow checker rules'), note('n2', 'Ação de exemplo')]);
});
afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('notes', () => {
	it('groups notes by lesson, links to the moment and searches without accents', async () => {
		renderApp('/notes', { catalog });
		const lesson = await screen.findByRole('region', { name: 'Rust: Ownership' });
		expect(within(lesson).getAllByRole('link', { name: 'Open Ownership at 1:15' })[0].getAttribute('href')).toBe('/course/rust/r%2Fown?t=75');
		expect(within(lesson).getByRole('link', { name: 'Open lesson' }).getAttribute('href')).toBe('/course/rust/r%2Fown');
		fireEvent.change(screen.getByRole('searchbox', { name: 'Search notes' }), { target: { value: 'acao' } });
		expect(screen.getByText('Ação de exemplo')).toBeTruthy();
		expect(screen.queryByText('Borrow checker rules')).toBeNull();
	});

	it('orders a lesson by its timeline, filters by course and brings an old note back', async () => {
		const at = (id: string, positionSeconds: number, updatedAt: number, mediaId = 'own'): StudyNote => ({ ...note(id, `note ${id}`, mediaId), positionSeconds, updatedAt });
		vi.mocked(transport.listNotes).mockResolvedValue([at('late', 300, 50), at('early', 10, 40), at('mid', 100, 30), at('oldest', 200, 5), at('lost', 1, 60, 'gone')]);
		renderApp('/notes', { catalog });
		const lesson = await screen.findByRole('region', { name: 'Rust: Ownership' });
		expect(within(lesson).getAllByRole('listitem').map((item) => item.textContent?.match(/note \w+/)?.[0])).toEqual(['note early', 'note mid', 'note oldest', 'note late']);
		expect(within(screen.getByRole('complementary', { name: 'A note to revisit' })).getByText('note oldest')).toBeTruthy();
		fireEvent.click(within(screen.getByRole('group', { name: 'Course' })).getByRole('button', { name: 'Rust' }));
		expect(screen.queryByText('note lost')).toBeNull();
	});

	it('edits a note in place', async () => {
		const update = vi.spyOn(transport, 'updateNote').mockImplementation(async (id, content) => ({ ...note(id, content) }));
		renderApp('/notes', { catalog });
		fireEvent.click((await screen.findAllByRole('button', { name: 'Edit note' }))[0]);
		fireEvent.change(screen.getByLabelText('Note text'), { target: { value: 'Updated' } });
		fireEvent.click(screen.getByRole('button', { name: 'Save' }));
		await waitFor(() => expect(update).toHaveBeenCalledWith(expect.any(String), 'Updated'));
	});
});

describe('Pomodoro menu', () => {
	it('opens from the topbar and starts and pauses the timer', async () => {
		renderApp('/', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: /^Pomodoro:/ }));
		const dialog = await screen.findByRole('dialog', { name: 'Pomodoro' });
		expect(within(dialog).getByRole('group', { name: 'Timer mode' })).toBeTruthy();
		fireEvent.click(within(dialog).getByRole('button', { name: 'Start' }));
		expect(pomodoro.get().status).toBe('running');
		fireEvent.click(within(dialog).getByRole('button', { name: 'Pause' }));
		expect(pomodoro.get().status).toBe('paused');
	});
});
