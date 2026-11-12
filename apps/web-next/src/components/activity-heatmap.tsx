import { useEffect, useMemo, useRef } from 'react';
import { formatCount } from '@/lib/catalog';
import { tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';

const WEEKS = 52;
const DAY_MS = 86_400_000;

/** The last `count` calendar days ending on `today`, oldest first, as `YYYY-MM-DD` in UTC. */
function lastDays(today: string, count: number): string[] {
	const end = Date.parse(`${today}T00:00:00Z`);
	return Array.from({ length: count }, (_, index) => new Date(end - (count - 1 - index) * DAY_MS).toISOString().slice(0, 10));
}

/**
 * Study days over the last year. Each column is a week, and each cell is a day, oldest at
 * the top. The grid is decorative: the sentence above it carries the same information.
 */
export function ActivityHeatmap({ today, activeDays }: { today: string; activeDays: string[] }) {
	const active = useMemo(() => new Set(activeDays), [activeDays]);
	const days = useMemo(() => lastDays(today, WEEKS * 7), [today]);
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
	return (
		<section aria-labelledby="heatmap-heading" className="space-y-3">
			<div className="space-y-1">
				<h2 id="heatmap-heading" className="text-base font-medium">{tr('Study days', 'Dias de estudo')}</h2>
				<p className="text-sm text-muted-foreground">{summary}</p>
			</div>
			<div ref={scroller} className="overflow-x-auto pb-2">
				<div aria-hidden="true" className="grid w-max grid-flow-col grid-rows-7 gap-1 sm:w-full sm:auto-cols-fr">
					{days.map((day) => (
						<span key={day} className={cn('size-3 rounded-[3px] sm:size-auto sm:aspect-square', active.has(day) ? 'bg-primary' : 'bg-card ring-1 ring-inset ring-border')} />
					))}
				</div>
			</div>
		</section>
	);
}
