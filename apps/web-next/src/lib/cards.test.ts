import { describe, expect, it } from 'vitest';
import { cardsToTsv, parseCards } from './cards';

describe('card exchange', () => {
	it('writes one tab-separated line per card and flattens tabs and newlines', () => {
		expect(cardsToTsv([{ front: 'What is\na lifetime?', back: 'A\tname' }, { front: 'q', back: 'a' }])).toBe('What is a lifetime?\tA name\nq\ta');
	});

	it('reads what it writes', () => {
		const cards = [{ front: 'Many readers', back: 'or one writer' }, { front: 'q', back: 'a, b; c' }];
		expect(parseCards(cardsToTsv(cards))).toEqual(cards);
	});

	it('accepts semicolon or comma files and skips comments, blanks and one-sided lines', () => {
		expect(parseCards('#separator:tab\n\nfront;back\nq,a\nonly-front\n;no-front\n')).toEqual([{ front: 'front', back: 'back' }, { front: 'q', back: 'a' }]);
	});
});
