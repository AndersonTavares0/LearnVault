import { Trash2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { PDF_MARK_COLORS } from '@/lib/pdf';
import { tr } from '@/lib/i18n';
import type { PdfAnnotation } from '@/lib/transport';

/**
 * The marks in this PDF, in page order. Each one jumps to its page; delete asks the server to
 * remove the mark. The PDF itself is never changed by either action.
 */
export function PdfMarks({ items, onGo, onDelete, pending }: { items: PdfAnnotation[]; onGo: (page: number) => void; onDelete: (annotationId: string) => void; pending: boolean }) {
	if (items.length === 0) return <p className="text-sm text-muted-foreground">{tr('No marks yet. Select text in the document to highlight it.', 'Nenhuma marcação ainda. Selecione um texto no documento para destacá-lo.')}</p>;
	return (
		<ul className="divide-y rounded-xl border bg-card">
			{items.map((item) => (
				<li key={item.annotationId} className="flex items-start gap-3 p-3">
					<span aria-hidden="true" className="mt-1.5 size-3 shrink-0 rounded-full" style={{ backgroundColor: PDF_MARK_COLORS[item.color] }} />
					<div className="min-w-0 flex-1 space-y-1">
						<Button variant="link" size="sm" className="h-auto p-0" onClick={() => onGo(item.page)}>{tr(`Page ${item.page}`, `Página ${item.page}`)}</Button>
						<p className="line-clamp-3 text-sm">{item.quote || tr('(no text selected)', '(sem texto selecionado)')}</p>
					</div>
					<Button variant="ghost" size="icon" disabled={pending} aria-label={tr(`Delete mark on page ${item.page}`, `Excluir marcação da página ${item.page}`)} onClick={() => onDelete(item.annotationId)}>
						<Trash2 />
					</Button>
				</li>
			))}
		</ul>
	);
}
