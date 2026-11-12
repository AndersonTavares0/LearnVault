import { useMemo } from 'react';
import { Eyebrow } from '@/components/panel';
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
	const summary = tr(
		`${formatCount(total, 'day with study', 'days with study')} in the last year`,
		`${formatCount(total, 'dia com estudo', 'dias com estudo')} no último ano`,
	);
	return (
		<section aria-labelledby="heatmap-heading" className="space-y-3">
			<Eyebrow><span id="heatmap-heading">{tr('Study days, last year', 'Dias de estudo, último ano')}</span></Eyebrow>
			<p className="text-sm text-muted-foreground">{summary}</p>
			<div className="overflow-x-auto pb-2">
				<div aria-hidden="true" className="grid grid-flow-col grid-rows-7 gap-1 w-max">
					{days.map((day) => (
						<span key={day} className={cn('size-3 rounded-[3px]', active.has(day) ? 'bg-primary' : 'bg-muted')} />
					))}
				</div>
			</div>
		</section>
	);
}
