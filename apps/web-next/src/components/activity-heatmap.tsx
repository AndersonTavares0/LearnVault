import { useEffect, useMemo, useRef } from 'react';
import { formatCount } from '@/lib/catalog';
import { getLocale, tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';

const WEEKS = 53;
const DAY_MS = 86_400_000;

/**
 * The last 53 weeks as whole Sunday-to-Saturday weeks, oldest first, as `YYYY-MM-DD` in UTC.
 * The final week includes the days after `today`; they stay empty.
 */
function lastWeeks(today: string): string[] {
	const end = Date.parse(`${today}T00:00:00Z`);
	const endOfWeek = end + (6 - new Date(end).getUTCDay()) * DAY_MS;
	const start = endOfWeek - (WEEKS * 7 - 1) * DAY_MS;
	return Array.from({ length: WEEKS * 7 }, (_, index) => new Date(start + index * DAY_MS).toISOString().slice(0, 10));
}

/**
 * Study days over the last year, read like a contribution graph: months across the top,
 * weekdays down the side, and one cell per day. The grid is decorative: the sentence above
 * it carries the same information.
 */
export function ActivityHeatmap({ today, activeDays }: { today: string; activeDays: string[] }) {
	const active = useMemo(() => new Set(activeDays), [activeDays]);
	const days = useMemo(() => lastWeeks(today), [today]);
	const months = useMemo(() => {
		const found: { key: string; label: string; week: number }[] = [];
		days.forEach((day, index) => {
			const key = day.slice(0, 7);
			if (found.at(-1)?.key !== key) found.push({ key, label: new Date(`${day}T12:00:00Z`).toLocaleDateString(getLocale(), { month: 'short' }), week: Math.floor(index / 7) });
		});
		return found;
	}, [days]);
	const total = days.filter((day) => active.has(day)).length;
	// On narrow screens the grid scrolls; start at the newest weeks, on the right.
	const scroller = useRef<HTMLDivElement>(null);
	useEffect(() => {
		if (scroller.current) scroller.current.scrollLeft = scroller.current.scrollWidth;
	}, []);
	const summary = tr(
		`${formatCount(total, 'day with study', 'days with study')} in the last year`,
		`${formatCount(total, 'dia com estudo', 'dias com estudo')} no último ano`,
	);
	const weekdays = ['', tr('Mon', 'Seg'), '', tr('Wed', 'Qua'), '', tr('Fri', 'Sex'), ''];
	return (
		<section aria-labelledby="heatmap-heading" className="space-y-3">
			<div className="space-y-1">
				<h2 id="heatmap-heading" className="text-base font-medium">{tr('Study days', 'Dias de estudo')}</h2>
				<p className="text-sm text-muted-foreground">{summary}</p>
			</div>
			<div className="flex gap-2">
				<div aria-hidden="true" className="mt-7 mb-2 grid w-6 shrink-0 grid-rows-7 gap-[3px] text-xs leading-none text-muted-foreground">
					{weekdays.map((day, index) => <span key={index} className="flex items-center">{day}</span>)}
				</div>
				<div ref={scroller} className="min-w-0 flex-1 overflow-x-auto pb-2">
					<div aria-hidden="true" className="w-max sm:w-full">
						<div className="relative mb-3 h-4 text-xs whitespace-nowrap text-muted-foreground">
							{months.map((month) => <span key={month.key} className="absolute" style={{ left: `${(month.week / WEEKS) * 100}%` }}>{month.label}</span>)}
						</div>
						<div data-heatmap-grid className="grid grid-flow-col grid-rows-7 gap-[3px] sm:auto-cols-fr">
							{days.map((day) => (
								<span key={day} className={cn('size-3 rounded-[3px] sm:size-auto sm:aspect-square', active.has(day) ? 'bg-primary' : 'bg-muted ring-1 ring-inset ring-border-strong')} />
							))}
						</div>
					</div>
				</div>
			</div>
			<div aria-hidden="true" className="flex items-center justify-end gap-1.5 text-xs text-muted-foreground">
				<span>{tr('Less', 'Menos')}</span>
				<span className="size-3 rounded-[3px] bg-muted ring-1 ring-inset ring-border-strong" />
				<span className="size-3 rounded-[3px] bg-primary" />
				<span>{tr('More', 'Mais')}</span>
			</div>
		</section>
	);
}
