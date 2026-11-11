import { Fragment } from 'react';
import { parseMarkdown, type Inline } from '@/lib/markdown';
import { cn } from '@/lib/utils';

const Inlines = ({ items }: { items: Inline[] }) => (
	<>
		{items.map((item, index) => (
			<span key={index} className={cn(item.bold && 'font-bold', item.italic && 'italic', item.underline && 'underline underline-offset-2')}>{item.text}</span>
		))}
	</>
);

/** Note text as headings, lists and paragraphs. React draws every piece, so it is never raw HTML. */
export function Markdown({ text, className }: { text: string; className?: string }) {
	return (
		<div className={cn('space-y-2', className)}>
			{parseMarkdown(text).map((block, index) => {
				if (block.kind === 'heading') return block.level === 2 ? <h3 key={index} className="font-heading text-lg font-bold"><Inlines items={block.inlines} /></h3> : <h4 key={index} className="font-heading font-bold"><Inlines items={block.inlines} /></h4>;
				if (block.kind === 'list') {
					const List = block.ordered ? 'ol' : 'ul';
					return <List key={index} className={cn('ml-5 space-y-1', block.ordered ? 'list-decimal' : 'list-disc')}>{block.items.map((item, i) => <li key={i}><Inlines items={item} /></li>)}</List>;
				}
				return <p key={index}>{block.lines.map((line, i) => <Fragment key={i}>{i > 0 && <br />}<Inlines items={line} /></Fragment>)}</p>;
			})}
		</div>
	);
}
