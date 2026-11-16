import { useId } from 'react';
import { cn } from '@/lib/utils';

/** Accessible full-width switch card backed by a real checkbox input. */
export function SwitchCard({ label, checked, onChange }: { label: string; checked: boolean; onChange: (checked: boolean) => void }) {
	const id = useId();
	return (
		<label
			htmlFor={id}
			className={cn(
				'flex min-h-12 cursor-pointer items-center justify-between gap-3 rounded-xl border px-3.5 py-2.5 text-sm font-medium transition-colors duration-150 focus-within:ring-3 focus-within:ring-ring/50',
				checked ? 'border-primary/40 bg-brand-tint text-foreground' : 'border-border bg-background text-muted-foreground hover:bg-muted/60'
			)}
		>
			<span>{label}</span>
			<span className="relative inline-flex shrink-0 items-center">
				<input id={id} type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} className="peer sr-only" />
				<span aria-hidden="true" className="h-5 w-9 rounded-full bg-border-strong/50 transition-colors duration-150 peer-checked:bg-primary after:absolute after:top-0.5 after:left-0.5 after:size-4 after:rounded-full after:bg-card after:shadow-xs after:transition-transform after:duration-150 after:content-[''] peer-checked:after:translate-x-4" />
			</span>
		</label>
	);
}
