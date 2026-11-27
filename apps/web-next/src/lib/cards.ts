import { tr } from '@/lib/i18n';
import type { ReviewRating } from '@/lib/transport';

/** Anki's plain-text exchange: one card per line, front and back separated by a tab. */

export type CardText = { front: string; back: string };

export const REVIEW_RATINGS: readonly ReviewRating[] = ['again', 'hard', 'good', 'easy'] as const;

export function ratingLabel(rating: ReviewRating): string {
	switch (rating) {
		case 'again': return tr('Again', 'De novo');
		case 'hard': return tr('Hard', 'Difícil');
		case 'good': return tr('Good', 'Bom');
		case 'easy': return tr('Easy', 'Fácil');
	}
}

/** Matches backend `schedule_review` rules so rating buttons preview the exact next interval. */
export function previewIntervalLabel(intervalDays: number, rating: ReviewRating): string {
	if (rating === 'again') return tr('10m', '10 min');
	if (rating === 'hard') {
		if (intervalDays === 0) return tr('12h', '12 h');
		const next = Math.min(365, Math.max(intervalDays, Math.floor((intervalDays * 6) / 5)));
		return tr(`${next}d`, `${next} d`);
	}
	if (rating === 'good') {
		const next = intervalDays === 0 ? 1 : Math.min(365, intervalDays * 2);
		return tr(`${next}d`, `${next} d`);
	}
	const next = intervalDays === 0 ? 4 : Math.min(365, intervalDays * 3);
	return tr(`${next}d`, `${next} d`);
}

/** Most cards one import creates; a larger file is cut here and the reader is told. */
export const MAX_IMPORT_CARDS = 500;

// Anki treats tabs and newlines as separators, so they become spaces inside a field.
const field = (text: string) => text.replace(/[\t\r\n]+/g, ' ').trim();

export function cardsToTsv(cards: CardText[]): string {
	return cards.map((card) => `${field(card.front)}\t${field(card.back)}`).join('\n');
}

/**
 * Reads tab-separated lines (the Anki export) or `front;back` / `front,back` when a line has no
 * tab. Comment lines (`#`) and lines without both sides are skipped.
 */
export function parseCards(text: string): CardText[] {
	const cards: CardText[] = [];
	for (const line of text.split(/\r?\n/)) {
		if (!line.trim() || line.startsWith('#')) continue;
		const separator = line.includes('\t') ? '\t' : line.includes(';') ? ';' : ',';
		const index = line.indexOf(separator);
		if (index < 0) continue;
		const front = line.slice(0, index).trim().slice(0, 4000);
		const back = line.slice(index + 1).trim().slice(0, 4000);
		if (front && back) cards.push({ front, back });
	}
	return cards;
}
