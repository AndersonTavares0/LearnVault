import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { setLocale } from '@/lib/i18n';
import { transport, type Catalog, type Flashcard } from '@/lib/transport';
import { renderApp } from '@/test/render-app';

const lesson = (id: string, title: string) => ({ title, path: `c/${id}`, role: 'lesson' as const, files: [{ name: `${id}.mp4`, path: `c/${id}.mp4`, size: 1, mediaId: id, kind: 'media', availability: 'available' }], looseFiles: [], children: [] });
const catalog: Catalog = { complete: true, looseFiles: [], nodes: [
	{ title: 'Rust', path: 'c', role: 'course', courseId: 'rust', files: [], looseFiles: [], children: [lesson('own', 'Ownership')] },
	{ title: 'Go', path: 'g', role: 'course', courseId: 'go', files: [], looseFiles: [], children: [lesson('chan', 'Channels')] }
] };
const card = (cardId: string, mediaId: string, nextDue: number, positionSeconds: number | null = null): Flashcard => ({ cardId, mediaId, positionSeconds, front: `front ${cardId}`, back: `back ${cardId}`, intervalDays: 0, nextDue, createdAt: 1, updatedAt: 1 });
const future = Math.floor(Date.now() / 1000) + 86_400;

beforeEach(() => {
	vi.spyOn(transport, 'listFlashcardReviews').mockResolvedValue({ items: [], total: 0 });
	vi.spyOn(transport, 'dueFlashcardCount').mockResolvedValue(0);
});

afterEach(() => {
	setLocale('en');
	cleanup();
	vi.restoreAllMocks();
});

describe('review', () => {
	it('lists one deck per course with what is due, and opens a deck', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1), card('b', 'own', 2), card('c', 'chan', future)]);
		vi.mocked(transport.dueFlashcardCount).mockResolvedValue(2);
		renderApp('/review', { catalog });
		expect(await screen.findByText('2 cards are due')).toBeTruthy();
		expect(await screen.findByRole('link', { name: /^Rust/ })).toBeTruthy();
		expect(screen.getAllByText('2 due · 2 cards').length).toBeGreaterThan(0);
		expect(screen.getAllByText('0 due · 1 card').length).toBeGreaterThan(0);
		expect(transport.listFlashcards).toHaveBeenCalledWith();
		cleanup();
		setLocale('pt-BR');
		renderApp('/review', { catalog });
		expect(await screen.findByText('2 cartões vencem agora')).toBeTruthy();
		expect(screen.getAllByText('2 vencem · 2 cartões').length).toBeGreaterThan(0);
		expect(screen.getAllByText('0 vencem · 1 cartão').length).toBeGreaterThan(0);
		cleanup();
		setLocale('en');
		renderApp('/review?course=rust', { catalog });
		expect(await screen.findByRole('heading', { name: '2 cards are due' })).toBeTruthy();
	});

	it('runs one session through every due card across courses and names each origin', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1), card('b', 'chan', 2), card('c', 'chan', future)]);
		vi.mocked(transport.dueFlashcardCount).mockResolvedValue(2);
		vi.spyOn(transport, 'reviewFlashcard').mockImplementation(async (id) => card(id, id === 'a' ? 'own' : 'chan', future));
		const { router } = renderApp('/review', { catalog });
		fireEvent.click(await screen.findByRole('link', { name: 'Review all due cards (2)' }));
		await waitFor(() => expect(router.state.location.search).toBe('?course=__all'));
		fireEvent.click(await screen.findByRole('button', { name: 'Start review' }));
		expect(screen.getByRole('heading', { name: 'front a' })).toBeTruthy();
		expect(screen.getByText(/Rust · Ownership/)).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Show answer' }));
		fireEvent.click(screen.getByRole('button', { name: /^Good/ }));
		expect(await screen.findByRole('heading', { name: 'front b' })).toBeTruthy();
		expect(screen.getByText(/Go · Channels/)).toBeTruthy();
	});

	it('runs a session with 4-grade keyboard shortcuts and ends when the due cards are done', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1), card('b', 'own', 2), card('c', 'own', 3), card('d', 'own', 4)]);
		const review = vi.spyOn(transport, 'reviewFlashcard').mockImplementation(async (id) => card(id, 'own', future));
		renderApp('/review?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Start review' }));
		expect(screen.getByRole('heading', { name: 'front a' })).toBeTruthy();
		expect(screen.queryByText('back a')).toBeNull();
		fireEvent.click(screen.getByRole('button', { name: 'Show answer' }));
		expect(screen.getByText('back a')).toBeTruthy();
		// The button that had focus is gone, so the session itself takes focus and 1-4 keep working.
		expect(document.activeElement?.getAttribute('tabindex')).toBe('-1');
		fireEvent.keyDown(document.activeElement!, { key: '3' });
		await waitFor(() => expect(review).toHaveBeenCalledWith('a', 'good'));
		expect(await screen.findByRole('heading', { name: 'front b' })).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Show answer' }));
		fireEvent.keyDown(document.activeElement!, { key: '2' });
		await waitFor(() => expect(review).toHaveBeenCalledWith('b', 'hard'));
		expect(await screen.findByRole('heading', { name: 'front c' })).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Show answer' }));
		fireEvent.keyDown(document.activeElement!, { key: '4' });
		await waitFor(() => expect(review).toHaveBeenCalledWith('c', 'easy'));
		expect(await screen.findByRole('heading', { name: 'front d' })).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Show answer' }));
		fireEvent.click(screen.getByRole('button', { name: /^Again/ }));
		await waitFor(() => expect(review).toHaveBeenCalledWith('d', 'again'));
		expect(await screen.findByRole('heading', { name: 'Session done' })).toBeTruthy();
	});

	it('links a card back to the lesson at the moment it was made', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1, 125)]);
		renderApp('/review?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Start review' }));
		fireEvent.click(screen.getByRole('button', { name: 'Show answer' }));
		const link = await screen.findByRole('link', { name: 'Open the lesson at 2:05' });
		expect(link.getAttribute('href')).toBe('/course/rust/c%2Fown?t=125');
	});

	it('explains how to make cards when there are none, and is in the navigation', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([]);
		renderApp('/review', { catalog });
		expect(await screen.findByRole('heading', { name: 'No cards yet' })).toBeTruthy();
		expect(screen.getAllByRole('link', { name: 'Review' })[0].getAttribute('href')).toBe('/review');
	});

	it('shows the due count from the server instead of summing every card', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1)]);
		vi.mocked(transport.dueFlashcardCount).mockResolvedValue(4);
		renderApp('/review', { catalog });
		expect(await screen.findByText('4 cards are due')).toBeTruthy();
	});

	it('shows the compact review history of the card on screen', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1)]);
		vi.mocked(transport.listFlashcardReviews).mockResolvedValue({
			items: [
				{ reviewId: 2, cardId: 'a', rating: 'again', previousIntervalDays: 1, nextDue: future, reviewedAt: 1_760_000_000 },
				{ reviewId: 1, cardId: 'a', rating: 'good', previousIntervalDays: 0, nextDue: 1_760_086_400, reviewedAt: 1_759_900_000 }
			],
			total: 3
		});
		renderApp('/review?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Start review' }));
		expect(await screen.findByText('3 reviews')).toBeTruthy();
		expect(screen.getByText('Again')).toBeTruthy();
		expect(screen.getByText('Good')).toBeTruthy();
		expect(screen.getByText('1 d before')).toBeTruthy();
		expect(transport.listFlashcardReviews).toHaveBeenCalledWith('a', 8);
	});

	it('explains an empty history and surfaces a history error', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1)]);
		renderApp('/review?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Start review' }));
		expect(await screen.findByText('This card has not been reviewed yet.')).toBeTruthy();
		cleanup();

		vi.mocked(transport.listFlashcardReviews).mockRejectedValue(new Error('history offline'));
		renderApp('/review?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('button', { name: 'Start review' }));
		expect(await screen.findByRole('alert')).toHaveProperty('textContent', 'history offline');
	});

	it('lets the learner browse, filter, edit and delete cards inside a deck', async () => {
		vi.spyOn(transport, 'listFlashcards').mockResolvedValue([card('a', 'own', 1), card('c', 'own', future)]);
		const update = vi.spyOn(transport, 'updateFlashcard').mockResolvedValue({ ...card('a', 'own', 1), front: 'front updated', back: 'back updated' });
		const remove = vi.spyOn(transport, 'deleteFlashcard').mockResolvedValue(undefined);
		renderApp('/review?course=rust', { catalog });
		fireEvent.click(await screen.findByRole('tab', { name: 'Browse cards (2)' }));
		expect(screen.getByText('front a')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Due' }));
		expect(screen.queryByText('front c')).toBeNull();
		fireEvent.click(screen.getByRole('button', { name: 'Edit card: front a' }));
		fireEvent.change(screen.getByLabelText('Front'), { target: { value: 'front updated' } });
		fireEvent.change(screen.getByLabelText('Back'), { target: { value: 'back updated' } });
		fireEvent.click(screen.getByRole('button', { name: 'Save card' }));
		await waitFor(() => expect(update).toHaveBeenCalledWith('a', 'front updated', 'back updated'));
		expect(await screen.findByText('front updated')).toBeTruthy();
		fireEvent.click(screen.getByRole('button', { name: 'Delete card: front updated' }));
		fireEvent.click(await screen.findByRole('button', { name: 'Delete card' }));
		await waitFor(() => expect(remove).toHaveBeenCalledWith('a'));
	});
});
