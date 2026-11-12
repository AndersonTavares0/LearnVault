import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { transport, type Catalog, type Flashcard } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const lesson = (id: string, title: string) => ({ title, path: `c/${id}`, role: 'lesson' as const, files: [{ name: `${id}.mp4`, path: `c/${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' }], looseFiles: [], children: [] });
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [
	{ title: 'Rust', path: 'c', role: 'course', courseId: 'rust', files: [], looseFiles: [], children: [lesson('own', 'Ownership')] },
	{ title: 'Go', path: 'g', role: 'course', courseId: 'go', files: [], looseFiles: [], children: [lesson('chan', 'Channels')] }
] };
const card = (cardId: string, mediaId: string, nextDue: number): Flashcard => ({ cardId, mediaId, front: `front ${cardId}`, back: `back ${cardId}`, intervalDays: 0, nextDue, createdAt: 1, updatedAt: 1 });
const future = Math.floor(Date.now() / 1000) + 86_400;

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
});

describe('review', () => {
	it('lists one deck per course with what is due, and opens a deck', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1), card('b', 'own', 2), card('c', 'chan', future)]);
		renderApp('/review', { catalog });
		expect(await screen.findByText('2 cards are due')).toBeTruthy();
		expect(await screen.findByRole('link', { name: /^Rust/ })).toBeTruthy();
		expect(screen.getAllByText('2 due · 2 cards').length).toBeGreaterThan(0);
		expect(screen.getAllByText('0 due · 1 cards').length).toBeGreaterThan(0);
		expect(transport.listFlashcards).toHaveBeenCalledWith();
		cleanup();
		renderApp('/review?course=rust', { catalog });
		expect(await screen.findByRole('heading', { name: '2 cards are due' })).toBeTruthy();
	});

	it('runs a session with the keyboard and ends when the due cards are done', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1), card('b', 'own', 2)]);
		const review = vi.spyOn(transport, 'reviewFlashcard').mockImplementation(async (id) => card(id, 'own', future));
		renderApp('/review?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Start review' }));
		expect(screen.getByRole('heading', { name: 'front a' })).toBeTruthy();
		expect(screen.queryByText('back a')).toBeNull();
		fireEvent.keyDown(screen.getByRole('button', { name: 'Show answer' }), { key: ' ' });
		expect(screen.getByText('back a')).toBeTruthy();
		// The button that had focus is gone, so the session itself takes focus and 1 and 2 keep working.
		expect(document.activeElement?.getAttribute('tabindex')).toBe('-1');
		fireEvent.keyDown(document.activeElement!, { key: '2' });
		await waitFor(() => expect(review).toHaveBeenCalledWith('a', 'good'));
		expect(await screen.findByRole('heading', { name: 'front b' })).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Show answer' }));
		fireEvent.click(screen.getByRole('button', { name: 'Again' }));
		await waitFor(() => expect(review).toHaveBeenCalledWith('b', 'again'));
		expect(await screen.findByRole('heading', { name: 'Session done' })).toBeTruthy();
	});

	it('explains how to make cards when there are none, and is in the navigation', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([]);
		renderApp('/review', { catalog });
		expect(await screen.findByText('No cards yet')).toBeTruthy();
		expect(screen.getAllByRole('link', { name: 'Review' })[0].getAttribute('href')).toBe('/review');
	});
});
