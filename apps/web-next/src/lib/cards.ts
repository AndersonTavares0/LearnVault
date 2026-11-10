/** Anki's plain-text exchange: one card per line, front and back separated by a tab. */

export type CardText = { front: string; back: string };

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
