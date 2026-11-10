import { describe, expect, it } from 'vitest';
import { dayKey, notesThisWeek, notesToMarkdown } from './notes-export';

describe('notes export', () => {
	it('writes a heading per course and lesson with the moment of each note', () => {
		const markdown = notesToMarkdown([
			{ course: 'Rust', lesson: 'Borrowing', positionSeconds: 192, updatedAt: 1, content: 'Owner drops\nat scope end' },
			{ course: 'Rust', lesson: 'Borrowing', positionSeconds: 48, updatedAt: 2, content: 'Readers or one writer' },
			{ course: 'Photo', lesson: 'Exposure', positionSeconds: 80, updatedAt: 3, content: 'ISO last' }
		]);
		expect(markdown).toBe('# LearnVault notes\n\n## Rust\n\n### Borrowing\n\n- **0:48** Readers or one writer\n- **3:12** Owner drops\n  at scope end\n\n## Photo\n\n### Exposure\n\n- **1:20** ISO last\n');
	});

	it('keys a note by the local day it was written', () => {
		expect(dayKey(new Date(2026, 10, 9, 23, 30).getTime() / 1000)).toBe('2026-11-09');
	});

	it('counts the notes written since Monday of the current week', () => {
		const wednesday = new Date(2026, 10, 11, 12);
		const at = (day: number) => new Date(2026, 10, day, 9).getTime() / 1000;
		expect(notesThisWeek([{ updatedAt: at(9) }, { updatedAt: at(11) }, { updatedAt: at(8) }], wednesday)).toBe(2);
	});
});
