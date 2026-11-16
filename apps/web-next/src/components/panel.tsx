import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

/** The look every page shares: wide rounded cards on the cream page, warm hairlines, small tracked labels. */
export const Panel = ({ children, className }: { children: ReactNode; className?: string }) => (
	<div data-panel className={cn('min-w-0 rounded-[2rem] border bg-card p-6 sm:p-8', className)}>{children}</div>
);

/** A tinted card for the one thing a page wants you to do first. */
export const Hero = ({ children, className }: { children: ReactNode; className?: string }) => (
	<div data-panel className={cn('rounded-[2rem] border border-brand-soft bg-panel p-6 sm:p-10', className)}>{children}</div>
);

export const Eyebrow = ({ children, className }: { children: ReactNode; className?: string }) => (
	<p className={cn('text-sm font-medium text-muted-foreground', className)}>{children}</p>
);

/** A small tinted number card: the label, the value, and optional extra context. */
export function Stat({ label, value, unit, note, icon, tone, bar, progress, compact = false, children, className }: { label: string; value: ReactNode; unit?: string; note?: string; icon?: ReactNode; tone?: 'amber' | 'violet' | 'green'; bar?: { value: number; max: number }; progress?: number; compact?: boolean; children?: ReactNode; className?: string }) {
	const percent = Math.round(Math.min(1, Math.max(0, progress ?? 0)) * 100);
	return (
		<div data-panel className={cn('rounded-2xl border border-border bg-card', compact ? 'p-3.5' : 'p-4', tone && `stat-${tone}`, className)}>
			<div className="flex items-center justify-between gap-2">
				<p className={cn('min-w-0 truncate font-semibold text-foreground/85', compact ? 'text-xs' : 'text-[0.875rem]')}>{label}</p>
				{icon && (
					<span data-stat-icon aria-hidden="true" className={cn('inline-flex shrink-0 items-center justify-center rounded-lg border border-border/80 bg-panel/70 text-muted-foreground', compact ? 'size-6 [&>svg]:size-3' : 'size-7 [&>svg]:size-3.5')}>
						{icon}
					</span>
				)}
			</div>
			<p data-stat-value className={cn('font-heading leading-none font-bold tabular-nums', compact ? 'mt-1.5 text-2xl' : 'mt-2 text-3xl')}>{value}{unit && <span className={cn('ml-1 font-sans font-medium text-muted-foreground', compact ? 'text-xs' : 'text-base')}>{unit}</span>}</p>
			{bar && (
				<div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={bar.max} aria-valuenow={bar.value} className={cn('flex gap-1', compact ? 'mt-2' : 'mt-3')}>
					{Array.from({ length: bar.max }, (_, index) => <span key={index} className={cn('h-1.5 flex-1 rounded-full', index < bar.value ? 'bg-primary' : 'bg-border')} />)}
				</div>
			)}
			{progress !== undefined && (
				<div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} className={cn('h-1.5 overflow-hidden rounded-full bg-border', compact ? 'mt-2' : 'mt-3')}>
					<div className="h-full rounded-full bg-primary" style={{ width: `${percent}%` }} />
				</div>
			)}
			{note && <p className="mt-1.5 truncate text-xs text-muted-foreground">{note}</p>}
			{children && <div className={compact ? 'mt-2' : 'mt-2.5'}>{children}</div>}
		</div>
	);
}

/** Circular progress with the value in the middle. The label is the accessible name. */
export function Ring({ value, label, caption, center, size = 132 }: { value: number; label: string; caption?: string; center?: ReactNode; size?: number }) {
	const percent = Math.round(Math.min(1, Math.max(0, value)) * 100);
	const radius = 52;
	const circumference = 2 * Math.PI * radius;
	return (
		<div role="img" aria-label={label} className="relative shrink-0" style={{ width: size, height: size }}>
			<svg viewBox="0 0 128 128" className="size-full -rotate-90" aria-hidden="true">
				<circle cx="64" cy="64" r={radius} fill="none" strokeWidth="11" className="stroke-brand-soft" />
				<circle cx="64" cy="64" r={radius} fill="none" strokeWidth="11" strokeLinecap="round" className="stroke-primary" strokeDasharray={`${(percent / 100) * circumference} ${circumference}`} />
			</svg>
			<div className="absolute inset-0 grid place-content-center text-center" aria-hidden="true">
				{center ?? <span className="font-heading text-3xl leading-none font-bold">{percent}<span className="text-base">%</span></span>}
				{caption && <span className="mt-1 text-xs font-semibold tracking-[0.1em] text-muted-foreground uppercase">{caption}</span>}
			</div>
		</div>
	);
}
