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
	it('regroups notes by date, newest first, naming each lesson', async () => {
		const at = (id: string, updatedAt: number): StudyNote => ({ ...note(id, `note ${id}`), updatedAt });
		vi.mocked(transport.listNotes).mockResolvedValue([at('old', new Date(2026, 9, 1, 10).getTime() / 1000), at('new', new Date(2026, 10, 5, 10).getTime() / 1000)]);
		renderApp('/notes?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'By date' }));
		const days = await screen.findAllByRole('region');
		expect(days).toHaveLength(2);
		expect(within(days[0]).getByText('note new')).toBeTruthy();
		expect(within(days[0]).getByText('Rust · Ownership')).toBeTruthy();
		expect(within(days[1]).getByText('note old')).toBeTruthy();
	});

	it('exports the notes on screen as a Markdown file', async () => {
		const create = vi.fn(() => 'blob:notes');
		vi.stubGlobal('URL', Object.assign(URL, { createObjectURL: create, revokeObjectURL: vi.fn() }));
		const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
		renderApp('/notes?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Export as Markdown' }));
		expect(create).toHaveBeenCalledOnce();
		const blob = (create.mock.calls[0] as unknown as [Blob])[0];
		expect(await blob.text()).toContain('## Rust\n\n### Ownership\n\n- **1:15** Borrow checker rules');
		expect(click).toHaveBeenCalled();
		vi.unstubAllGlobals();
	});

	it('groups notes by lesson, links to the moment and searches without accents', async () => {
		renderApp('/notes?course=rust', { catalog });
		const lesson = await screen.findByRole('region', { name: 'Ownership' });
		expect(within(lesson).getAllByRole('link', { name: 'Open Ownership at 1:15' })[0].getAttribute('href')).toBe('/course/rust/r%2Fown?t=75');
		expect(within(lesson).getByRole('link', { name: 'Open lesson' }).getAttribute('href')).toBe('/course/rust/r%2Fown');
		fireEvent.change(screen.getByRole('searchbox', { name: 'Search notes' }), { target: { value: 'acao' } });
		expect(screen.getByText('Ação de exemplo')).toBeTruthy();
		expect(screen.queryByText('Borrow checker rules')).toBeNull();
	});

	it('orders a lesson by its timeline and leaves other courses out', async () => {
		const at = (id: string, positionSeconds: number, updatedAt: number, mediaId = 'own'): StudyNote => ({ ...note(id, `note ${id}`, mediaId), positionSeconds, updatedAt });
		vi.mocked(transport.listNotes).mockResolvedValue([at('late', 300, 50), at('early', 10, 40), at('mid', 100, 30), at('oldest', 200, 5), at('lost', 1, 60, 'gone')]);
		renderApp('/notes?course=rust', { catalog });
		const lesson = await screen.findByRole('region', { name: 'Ownership' });
		expect(within(lesson).getAllByRole('listitem').map((item) => item.textContent?.match(/note \w+/)?.[0])).toEqual(['note early', 'note mid', 'note oldest', 'note late']);
		expect(screen.queryByText('note lost')).toBeNull();
	});

	it('starts from the courses, with the note count of each, and opens one', async () => {
		renderApp('/notes', { catalog });
		expect(await screen.findByRole('heading', { level: 1, name: /Choose a course to open its Notes/ })).toBeTruthy();
		const link = await screen.findByRole('link', { name: /^Rust/ });
		expect(link.getAttribute('href')).toBe('/notes?course=rust');
		expect(screen.getAllByText('2 notes').length).toBeGreaterThan(0);
		expect(screen.getByText(/No pinned courses yet/)).toBeTruthy();
		fireEvent.change(screen.getByRole('searchbox', { name: 'Filter courses' }), { target: { value: 'zzz' } });
		expect(screen.getByText('No courses match your search.')).toBeTruthy();
	});

	it('edits a note in place', async () => {
		const update = vi.spyOn(transport, 'updateNote').mockImplementation(async (id, content) => ({ ...note(id, content) }));
		renderApp('/notes?course=rust', { catalog });
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
