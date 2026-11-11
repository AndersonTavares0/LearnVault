import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { Markdown } from './markdown';
import { NoteEditor } from './note-editor';

afterEach(cleanup);

function Harness() {
	const [value, setValue] = useState('hello world');
	return <><NoteEditor id="n" label="Note" value={value} onChange={setValue} /><Markdown text={value} /></>;
}

describe('NoteEditor', () => {
	it('formats the selected text and shows it formatted', () => {
		render(<Harness />);
		const field = screen.getByLabelText('Note') as HTMLTextAreaElement;
		field.setSelectionRange(0, 5);
		fireEvent.click(screen.getByRole('button', { name: 'Bold' }));
		expect(field.value).toBe('**hello** world');
		expect(screen.getByText('hello').className).toContain('font-bold');
	});

	it('turns a line into a bulleted list', () => {
		render(<Harness />);
		fireEvent.click(screen.getByRole('button', { name: 'Bulleted list' }));
		expect(screen.getByRole('listitem').textContent).toBe('hello world');
	});

	it('takes focus when it opens from a click, so typing goes straight in', () => {
		render(<NoteEditor id="f" label="Note" value="" onChange={() => undefined} focusOnOpen />);
		expect(document.activeElement).toBe(screen.getByLabelText('Note'));
	});
});
