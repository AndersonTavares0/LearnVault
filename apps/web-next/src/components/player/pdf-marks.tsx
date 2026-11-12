import { Download, MessageSquareText, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { marksToMarkdown, PDF_MARK_COLORS } from '@/lib/pdf';
import { tr } from '@/lib/i18n';
import type { PdfAnnotation, PdfAnnotationColor } from '@/lib/transport';
import { cn } from '@/lib/utils';

export const MARK_COLORS = Object.keys(PDF_MARK_COLORS) as PdfAnnotationColor[];
export const colorName = (color: PdfAnnotationColor) => ({ yellow: tr('Yellow', 'Amarelo'), green: tr('Green', 'Verde'), blue: tr('Blue', 'Azul'), pink: tr('Pink', 'Rosa') })[color];
const kindName = (kind: PdfAnnotation['kind']) => ({ highlight: tr('Highlight', 'Destaque'), underline: tr('Underline', 'Sublinhado'), note: tr('Note', 'Nota') })[kind];

/** A row of four round swatches; the chosen one is ringed and announced by `aria-checked`. */
export function ColorSwatches({ value, onChange, label, size = 'md' }: { value: PdfAnnotationColor; onChange: (color: PdfAnnotationColor) => void; label: string; size?: 'sm' | 'md' }) {
	return (
		<div role="radiogroup" aria-label={label} className="flex items-center gap-1">
			{MARK_COLORS.map((color) => (
				<button key={color} type="button" role="radio" aria-checked={value === color} aria-label={colorName(color)} onClick={() => onChange(color)}
					className={cn('grid place-items-center rounded-full focus-visible:ring-3 focus-visible:ring-ring/60 focus-visible:outline-none', 'size-11')}>
					<span className={cn('rounded-full border border-black/15', size === 'md' ? 'size-5' : 'size-4', value === color && 'ring-2 ring-foreground ring-offset-2 ring-offset-card')} style={{ backgroundColor: PDF_MARK_COLORS[color] }} />
				</button>
			))}
		</div>
	);
}

type Props = {
	title: string;
	items: PdfAnnotation[];
	page: number;
	onGo: (page: number) => void;
	onChange: (annotationId: string, change: { color?: PdfAnnotationColor; content?: string }) => void;
	onDelete: (annotationId: string) => void;
	pending: boolean;
};

/**
 * The marks in this PDF, in page order: edit the note and the colour of each, jump to its page,
 * delete it, or keep them all as a Markdown file. None of it changes the PDF itself.
 */
export function PdfMarks({ title, items, page, onGo, onChange, onDelete, pending }: Props) {
	const [scope, setScope] = useState<'page' | 'all'>('all');
	const shown = [...(scope === 'page' ? items.filter((item) => item.page === page) : items)].sort((a, b) => a.page - b.page || a.createdAt - b.createdAt);
	const exportMarks = () => {
		const url = URL.createObjectURL(new Blob([marksToMarkdown(title, items)], { type: 'text/markdown' }));
		const link = document.createElement('a');
		link.href = url;
		link.download = `${title.replace(/\.pdf$/i, '')}-marks.md`;
		link.click();
		URL.revokeObjectURL(url);
	};
	return (
		<div className="space-y-4">
			<div className="flex flex-wrap items-center gap-2">
				<div role="group" aria-label={tr('Show marks', 'Mostrar marcações')} className="flex gap-1 rounded-full bg-muted p-1">
					{([['all', tr(`All (${items.length})`, `Todas (${items.length})`)], ['page', tr(`This page (${items.filter((item) => item.page === page).length})`, `Esta página (${items.filter((item) => item.page === page).length})`)]] as const).map(([value, text]) => (
						<button key={value} type="button" aria-pressed={scope === value} onClick={() => setScope(value)} className={cn('min-h-11 rounded-full px-4 text-sm font-medium focus-visible:ring-3 focus-visible:ring-ring/60 focus-visible:outline-none', scope === value && 'bg-card shadow-sm')}>{text}</button>
					))}
				</div>
				<Button variant="outline" size="sm" className="ml-auto" disabled={items.length === 0} onClick={exportMarks}><Download />{tr('Export as Markdown', 'Exportar em Markdown')}</Button>
			</div>
			{items.length === 0 ? (
				<p className="text-sm text-muted-foreground">{tr('No marks yet. Select text in the document to highlight it, underline it or attach a note.', 'Nenhuma marcação ainda. Selecione um texto no documento para destacá-lo, sublinhá-lo ou anexar uma nota.')}</p>
			) : shown.length === 0 ? (
				<p className="text-sm text-muted-foreground">{tr('No marks on this page.', 'Nenhuma marcação nesta página.')}</p>
			) : (
				<ul className="space-y-3">
					{shown.map((item) => (
						<li key={item.annotationId} className="space-y-3 rounded-2xl border bg-card p-4">
							<div className="flex flex-wrap items-center gap-2">
								<span aria-hidden="true" className="size-3 shrink-0 rounded-full" style={{ backgroundColor: PDF_MARK_COLORS[item.color] }} />
								<span className="text-xs font-semibold tracking-[0.12em] text-muted-foreground uppercase">{kindName(item.kind)}</span>
								<Button variant="link" size="sm" className="h-auto p-0" onClick={() => onGo(item.page)}>{tr(`Page ${item.page}`, `Página ${item.page}`)}</Button>
								<div className="ml-auto flex items-center">
									<ColorSwatches size="sm" label={tr(`Color of the mark on page ${item.page}`, `Cor da marcação da página ${item.page}`)} value={item.color} onChange={(color) => onChange(item.annotationId, { color })} />
									<Button variant="ghost" size="icon" disabled={pending} aria-label={tr(`Delete mark on page ${item.page}`, `Excluir marcação da página ${item.page}`)} onClick={() => onDelete(item.annotationId)}><Trash2 /></Button>
								</div>
							</div>
							{item.quote && <blockquote className="border-l-4 pl-3 text-sm" style={{ borderColor: PDF_MARK_COLORS[item.color] }}>{item.quote}</blockquote>}
							<NoteField item={item} onSave={(content) => onChange(item.annotationId, { content })} />
						</li>
					))}
				</ul>
			)}
		</div>
	);
}

/** A note is saved when the field loses focus, and only if the text changed. A note keeps some text: the server refuses an empty one. */
function NoteField({ item, onSave }: { item: PdfAnnotation; onSave: (content: string) => void }) {
	const [text, setText] = useState(item.content);
	const label = tr(`Note for the mark on page ${item.page}`, `Nota da marcação da página ${item.page}`);
	return (
		<div className="flex items-start gap-2">
			<MessageSquareText className="mt-2.5 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
			<textarea aria-label={label} value={text} rows={item.kind === 'note' ? 3 : 2} maxLength={4000} placeholder={tr('Add a note…', 'Adicionar uma nota…')}
				onChange={(event) => setText(event.target.value)} onBlur={() => (item.kind === 'note' && !text.trim() ? setText(item.content) : text !== item.content && onSave(text))}
				className="min-h-11 w-full resize-y rounded-xl border border-input bg-background px-3 py-2 text-sm focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none" />
		</div>
	);
}
