import { Bold, Heading2, Heading3, Italic, List, ListOrdered, Underline } from 'lucide-react';
import { useEffect, useRef, type KeyboardEvent, type MouseEvent } from 'react';
import { Button } from '@/components/ui/button';
import { tr } from '@/lib/i18n';
import { applyFormat, type Format } from '@/lib/markdown';

const TOOLS = (): { format: Format; label: string; icon: typeof Bold }[] => [
	{ format: 'bold', label: tr('Bold', 'Negrito'), icon: Bold },
	{ format: 'italic', label: tr('Italic', 'Itálico'), icon: Italic },
	{ format: 'underline', label: tr('Underline', 'Sublinhado'), icon: Underline },
	{ format: 'h2', label: tr('Heading', 'Título'), icon: Heading2 },
	{ format: 'h3', label: tr('Subheading', 'Subtítulo'), icon: Heading3 },
	{ format: 'bullets', label: tr('Bulleted list', 'Lista com marcadores'), icon: List },
	{ format: 'numbered', label: tr('Numbered list', 'Lista numerada'), icon: ListOrdered }
];

type Props = { /** Focus the field when it appears: the editor opens from a click, so focus follows the person's action. */ focusOnOpen?: boolean; id: string; value: string; onChange: (value: string) => void; onKeyDown?: (event: KeyboardEvent) => void; describedBy?: string; rows?: number; label?: string };

/** A text box with a small formatting bar. The note stays plain text with Markdown marks. */
export function NoteEditor({ id, value, onChange, onKeyDown, describedBy, rows = 4, label, focusOnOpen = false }: Props) {
	const field = useRef<HTMLTextAreaElement>(null);
	useEffect(() => { if (focusOnOpen) field.current?.focus(); }, [focusOnOpen]);
	const format = (kind: Format) => {
		const element = field.current;
		if (!element) return;
		const next = applyFormat(value, element.selectionStart, element.selectionEnd, kind);
		onChange(next.value);
		// The field keeps the selection on the formatted text once React has written the new value.
		requestAnimationFrame(() => { element.focus(); element.setSelectionRange(next.start, next.end); });
	};
	return (
		<div className="overflow-hidden rounded-2xl border border-input bg-background focus-within:ring-3 focus-within:ring-ring/50">
			<div role="toolbar" aria-label={tr('Formatting', 'Formatação')} aria-controls={id} className="flex flex-wrap gap-0.5 border-b bg-panel p-1">
				{TOOLS().map(({ format: kind, label: text, icon: Icon }) => (
					// Mouse-down is cancelled so pressing a tool keeps the selection in the field.
					<Button key={kind} type="button" variant="ghost" size="icon" className="size-11" aria-label={text} onMouseDown={(event: MouseEvent) => event.preventDefault()} onClick={() => format(kind)}><Icon /></Button>
				))}
			</div>
			<textarea ref={field} id={id} aria-label={label} aria-describedby={describedBy} rows={rows} maxLength={10_000} value={value} onChange={(event) => onChange(event.target.value)} onKeyDown={onKeyDown} className="block w-full resize-y bg-transparent p-3 text-sm outline-none" />
		</div>
	);
}
