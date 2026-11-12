import { describe, expect, it } from 'vitest';
import { applyFormat, parseMarkdown } from './markdown';

describe('markdown', () => {
	it('reads headings, lists, paragraphs and inline marks', () => {
		expect(parseMarkdown('## Title\nsome **bold** and *slanted* and __under__\n\n- one\n- two\n1. first')).toEqual([
			{ kind: 'heading', level: 2, inlines: [{ text: 'Title' }] },
			{ kind: 'paragraph', lines: [[{ text: 'some ' }, { text: 'bold', bold: true }, { text: ' and ' }, { text: 'slanted', italic: true }, { text: ' and ' }, { text: 'under', underline: true }]] },
			{ kind: 'list', ordered: false, items: [[{ text: 'one' }], [{ text: 'two' }]] },
			{ kind: 'list', ordered: true, items: [[{ text: 'first' }]] }
		]);
	});

	it('keeps markup-looking text as plain text, never as HTML', () => {
		expect(parseMarkdown('<img src=x onerror=alert(1)>')).toEqual([{ kind: 'paragraph', lines: [[{ text: '<img src=x onerror=alert(1)>' }]] }]);
	});

	it('wraps the selection, or a placeholder, in marks', () => {
		expect(applyFormat('say hello now', 4, 9, 'bold')).toEqual({ value: 'say **hello** now', start: 6, end: 11 });
		expect(applyFormat('', 0, 0, 'italic')).toEqual({ value: '*text*', start: 1, end: 5 });
	});

	it('turns whole lines into a heading or a list, and back', () => {
		expect(applyFormat('a\nb', 0, 3, 'bullets').value).toBe('- a\n- b');
		expect(applyFormat('- a\n- b', 0, 7, 'numbered').value).toBe('1. a\n2. b');
		expect(applyFormat('plain', 0, 0, 'h2').value).toBe('## plain');
	});

	it('keeps a selection that ends at the start of a line to the lines it covers', () => {
		expect(applyFormat('one\ntwo\nthree', 0, 4, 'bullets').value).toBe('- one\ntwo\nthree');
	});

	it('treats a cursor at the very start of text that begins with a line break as that empty line', () => {
		expect(applyFormat('\nabc', 0, 0, 'bullets').value).toBe('- \nabc');
	});
});
