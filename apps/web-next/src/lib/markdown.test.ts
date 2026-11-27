import { describe, expect, it } from 'vitest';
import { applyFormat, parseMarkdown, plainTextMarkdown } from './markdown';

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

	it('leaves double underscores inside a word alone', () => {
		expect(parseMarkdown('call __init__ now')).toEqual([{ kind: 'paragraph', lines: [[{ text: 'call ' }, { text: 'init', underline: true }, { text: ' now' }]] }]);
		expect(parseMarkdown('snake__case__name')).toEqual([{ kind: 'paragraph', lines: [[{ text: 'snake__case__name' }]] }]);
	});

	it('parses inline code spans and keeps inner markers literal', () => {
		expect(parseMarkdown('Run `docker image prune` or `a * b`')).toEqual([
			{ kind: 'paragraph', lines: [[{ text: 'Run ' }, { text: 'docker image prune', code: true }, { text: ' or ' }, { text: 'a * b', code: true }]] }
		]);
	});

	it('strips block and inline markers for compact plain-text previews', () => {
		expect(plainTextMarkdown('## Summary\nRun **important** `docker image prune`\n- first\n- second')).toBe('Summary Run important docker image prune first · second');
	});

	it('parses fenced code blocks cleanly without interpreting inner markers', () => {
		expect(parseMarkdown('```rs\nlet x = **not bold**;\n```')).toEqual([
			{ kind: 'code', language: 'rs', code: 'let x = **not bold**;' }
		]);
		expect(applyFormat('a\nb', 0, 3, 'code').value).toBe('```\na\nb\n```');
	});
});
