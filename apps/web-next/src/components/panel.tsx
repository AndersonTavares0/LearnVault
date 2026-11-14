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
export function Stat({ label, value, unit, note, icon, tone, bar, progress, children, className }: { label: string; value: ReactNode; unit?: string; note?: string; icon?: ReactNode; tone?: 'amber' | 'violet' | 'green'; bar?: { value: number; max: number }; progress?: number; children?: ReactNode; className?: string }) {
	const percent = Math.round(Math.min(1, Math.max(0, progress ?? 0)) * 100);
	return (
		<div data-panel className={cn('rounded-[1.75rem] border border-border bg-card p-5', tone && `stat-${tone}`, className)}>
			{icon && <div data-stat-icon className="mb-3 grid size-9 place-items-center rounded-xl bg-brand-tint text-primary [&_svg]:size-5" aria-hidden="true">{icon}</div>}
			<Eyebrow>{label}</Eyebrow>
			<p data-stat-value className="mt-2 font-heading text-3xl font-semibold">{value}{unit && <span className="ml-1.5 font-sans text-base font-medium text-muted-foreground">{unit}</span>}</p>
			{bar && (
				<div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={bar.max} aria-valuenow={bar.value} className="mt-3 flex gap-1">
					{Array.from({ length: bar.max }, (_, index) => <span key={index} className={cn('h-1.5 flex-1 rounded-full', index < bar.value ? 'bg-primary' : 'bg-border')} />)}
				</div>
			)}
			{progress !== undefined && (
				<div role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} className="mt-3 h-1.5 overflow-hidden rounded-full bg-border">
					<div className="h-full rounded-full bg-primary" style={{ width: `${percent}%` }} />
				</div>
			)}
			{note && <p className="mt-1 text-sm text-muted-foreground">{note}</p>}
			{children && <div className="mt-3">{children}</div>}
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
				{caption && <span className="mt-1 text-[0.65rem] font-semibold tracking-[0.12em] text-muted-foreground uppercase">{caption}</span>}
			</div>
		</div>
	);
}
