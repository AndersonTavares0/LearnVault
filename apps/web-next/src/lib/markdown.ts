/**
 * The small slice of Markdown notes use: **bold**, *italic*, __underline__, ## and ### headings,
 * "- " and "1. " lists. It is parsed into plain data and drawn by React (components/markdown.tsx),
 * so note text never becomes HTML.
 */
export type Inline = { text: string; bold?: boolean; italic?: boolean; underline?: boolean };
export type Block =
	| { kind: 'paragraph'; lines: Inline[][] }
	| { kind: 'heading'; level: 2 | 3; inlines: Inline[] }
	| { kind: 'list'; ordered: boolean; items: Inline[][] };

const TOKEN = /(\*\*[^*]+\*\*|__[^_]+__|\*[^*\s][^*]*\*)/g;

export function parseInline(text: string): Inline[] {
	return text.split(TOKEN).filter(Boolean).map((part) => {
		if (part.startsWith('**') && part.endsWith('**') && part.length > 4) return { text: part.slice(2, -2), bold: true };
		if (part.startsWith('__') && part.endsWith('__') && part.length > 4) return { text: part.slice(2, -2), underline: true };
		if (part.startsWith('*') && part.endsWith('*') && part.length > 2) return { text: part.slice(1, -1), italic: true };
		return { text: part };
	});
}

export function parseMarkdown(source: string): Block[] {
	const blocks: Block[] = [];
	for (const line of source.replace(/\r\n?/g, '\n').split('\n')) {
		const heading = /^(#{2,3}) (.+)$/.exec(line);
		const bullet = /^[-*] (.+)$/.exec(line);
		const numbered = /^\d+\. (.+)$/.exec(line);
		const last = blocks.at(-1);
		if (heading) blocks.push({ kind: 'heading', level: heading[1].length as 2 | 3, inlines: parseInline(heading[2]) });
		else if (bullet || numbered) {
			const ordered = Boolean(numbered);
			const item = parseInline((bullet ?? numbered)![1]);
			if (last?.kind === 'list' && last.ordered === ordered) last.items.push(item);
			else blocks.push({ kind: 'list', ordered, items: [item] });
		} else if (line.trim() === '') {
			blocks.push({ kind: 'paragraph', lines: [] });
		} else if (last?.kind === 'paragraph' && last.lines.length > 0) last.lines.push(parseInline(line));
		else if (last?.kind === 'paragraph') last.lines.push(parseInline(line));
		else blocks.push({ kind: 'paragraph', lines: [parseInline(line)] });
	}
	return blocks.filter((block) => block.kind !== 'paragraph' || block.lines.length > 0);
}

export type Format = 'bold' | 'italic' | 'underline' | 'h2' | 'h3' | 'bullets' | 'numbered';

/** Applies a format to the selected text, returning the new text and where the selection ends up. */
export function applyFormat(value: string, start: number, end: number, format: Format): { value: string; start: number; end: number } {
	const wrap = (mark: string) => {
		const selected = value.slice(start, end) || '';
		const inner = selected || 'text';
		return { value: `${value.slice(0, start)}${mark}${inner}${mark}${value.slice(end)}`, start: start + mark.length, end: start + mark.length + inner.length };
	};
	if (format === 'bold') return wrap('**');
	if (format === 'italic') return wrap('*');
	if (format === 'underline') return wrap('__');
	// Line formats work on whole lines touched by the selection.
	const lineStart = value.lastIndexOf('\n', start - 1) + 1;
	const nextBreak = value.indexOf('\n', end);
	const lineEnd = nextBreak === -1 ? value.length : nextBreak;
	const lines = value.slice(lineStart, lineEnd).split('\n');
	const strip = (line: string) => line.replace(/^(#{2,3} |[-*] |\d+\. )/, '');
	const prefixed = lines.map((line, index) => `${format === 'h2' ? '## ' : format === 'h3' ? '### ' : format === 'bullets' ? '- ' : `${index + 1}. `}${strip(line)}`).join('\n');
	return { value: `${value.slice(0, lineStart)}${prefixed}${value.slice(lineEnd)}`, start: lineStart, end: lineStart + prefixed.length };
}
