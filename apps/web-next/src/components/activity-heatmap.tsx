import { useEffect, useMemo, useRef, useState } from 'react';
import { formatCount } from '@/lib/catalog';
import { getLocale, tr } from '@/lib/i18n';
import { cn } from '@/lib/utils';

const WEEKS = 53;
const DAY_MS = 86_400_000;

export type DayActivity = {
	score?: number;
	lessons?: number;
	completed?: number;
	focusMinutes?: number;
};

const LEVEL_MIX: Record<1 | 2 | 3, string> = {
	1: 'color-mix(in oklch, var(--primary) 30%, var(--muted))',
	2: 'color-mix(in oklch, var(--primary) 54%, var(--muted))',
	3: 'color-mix(in oklch, var(--primary) 78%, var(--muted))'
};

function dayLevel(isActive: boolean, info?: DayActivity): 0 | 1 | 2 | 3 | 4 {
	const score = info?.score ?? (isActive ? 1 : 0);
	if (!isActive && score <= 0) return 0;
	if (score >= 5) return 4;
	if (score >= 3) return 3;
	if (score >= 2) return 2;
	return 1;
}

function levelStyle(level: 0 | 1 | 2 | 3 | 4): React.CSSProperties | undefined {
	if (level === 1 || level === 2 || level === 3) return { backgroundColor: LEVEL_MIX[level] };
	return undefined;
}

function formatDay(day: string): string {
	return new Date(`${day}T12:00:00Z`).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric', year: 'numeric' });
}

function describeActivity(level: 0 | 1 | 2 | 3 | 4, info?: DayActivity): string {
	if (level === 0) return tr('No study recorded', 'Sem estudo registrado');
	const parts: string[] = [];
	if (info?.lessons) parts.push(formatCount(info.lessons, tr('lesson', 'aula'), tr('lessons', 'aulas')));
	if (info?.focusMinutes) parts.push(tr(`${info.focusMinutes} min focus`, `${info.focusMinutes} min de foco`));
	if (parts.length > 0) return parts.join(', ');
	return level >= 4 ? tr('Deep study', 'Estudo intenso') : level === 3 ? tr('Strong study', 'Bom ritmo') : level === 2 ? tr('Moderate study', 'Estudo moderado') : tr('Studied', 'Estudado');
}

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
 * weekdays down the side, and one cell per day. The table provides the same daily details
 * as the pointer tooltip.
 */
export function ActivityHeatmap({ today, activeDays, activity }: { today: string; activeDays: string[]; activity?: Record<string, DayActivity> }) {
	const active = useMemo(() => new Set(activeDays), [activeDays]);
	const days = useMemo(() => lastWeeks(today), [today]);
	const containerRef = useRef<HTMLDivElement>(null);
	const [showDetails, setShowDetails] = useState(false);
	const [hovered, setHovered] = useState<{ day: string; x: number; y: number; width: number } | null>(null);
	const months = useMemo(() => {
		const found: { key: string; label: string; week: number }[] = [];
		days.forEach((day, index) => {
			const key = day.slice(0, 7);
			const week = Math.floor(index / 7);
			if (week >= 51 || found.at(-1)?.key === key) return;
			const label = new Date(`${day}T12:00:00Z`).toLocaleDateString(getLocale(), { month: 'short' });
			if (found.length === 1 && week - found[0].week < 3) found[0] = { key, label, week };
			else found.push({ key, label, week });
		});
		return found;
	}, [days]);
	const total = days.filter((day) => active.has(day) || (activity?.[day]?.score ?? 0) > 0).length;
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
	const hoveredInfo = hovered ? activity?.[hovered.day] : undefined;
	const hoveredLevel = hovered ? dayLevel(active.has(hovered.day), hoveredInfo) : 0;
	const tipTransform = hovered
		? hovered.x < 104
			? 'translate(0, calc(-100% - 8px))'
			: hovered.width - hovered.x < 104
				? 'translate(-100%, calc(-100% - 8px))'
				: 'translate(-50%, calc(-100% - 8px))'
		: undefined;
	const tipLeft = hovered
		? hovered.x < 104
			? Math.max(0, hovered.x - 8)
			: hovered.width - hovered.x < 104
				? Math.min(hovered.width, hovered.x + 8)
				: hovered.x
		: 0;
	return (
		<section aria-labelledby="heatmap-heading" className="space-y-3">
			<div className="flex flex-wrap items-baseline justify-between gap-2">
				<h2 id="heatmap-heading" className="text-base font-medium">{tr('Study days', 'Dias de estudo')}</h2>
				<p className="text-sm text-muted-foreground">{summary}</p>
			</div>
			<div ref={containerRef} className="relative flex gap-2">
				<div aria-hidden="true" className="mt-7 mb-2 grid w-6 shrink-0 grid-rows-7 gap-[3px] text-xs leading-none text-muted-foreground">
					{weekdays.map((day, index) => <span key={index} className="flex items-center">{day}</span>)}
				</div>
				<div
					ref={scroller}
					role="region"
					aria-label={tr('Study activity heatmap', 'Mapa de atividade de estudo')}
					// eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex -- scrollable region must be keyboard-focusable
					tabIndex={0}
					className="min-w-0 flex-1 overflow-x-auto rounded-lg pb-2 focus-visible:outline-none focus-visible:ring-3 focus-visible:ring-ring"
				>
					<div aria-hidden="true" className="w-max sm:w-full">
						<div className="relative mb-3 h-4 text-xs whitespace-nowrap text-muted-foreground">
							{months.map((month) => <span key={month.key} className="absolute" style={{ left: `${(month.week / WEEKS) * 100}%` }}>{month.label}</span>)}
						</div>
						<div data-heatmap-grid onMouseLeave={() => setHovered(null)} className="grid grid-flow-col grid-rows-7 gap-[3px] sm:auto-cols-fr">
							{days.map((day) => {
								const info = activity?.[day];
								const level = dayLevel(active.has(day), info);
								return (
									<span
										key={day}
										data-day={day}
										data-level={level}
										style={levelStyle(level)}
										onMouseEnter={(event) => {
											const box = containerRef.current?.getBoundingClientRect();
											const cell = event.currentTarget.getBoundingClientRect();
											setHovered({
												day,
												x: box ? cell.left - box.left + cell.width / 2 : 0,
												y: box ? cell.top - box.top : 0,
												width: box?.width ?? 0
											});
										}}
										className={cn(
											'size-3 rounded-[3px] transition-[transform,box-shadow] duration-100 hover:ring-2 hover:ring-foreground/60 sm:size-auto sm:aspect-square',
											level > 0 ? 'bg-primary' : 'bg-muted ring-1 ring-inset ring-border/60'
										)}
									/>
								);
							})}
						</div>
					</div>
				</div>
				{hovered && (
					<div role="tooltip" className="pointer-events-none absolute z-20 rounded-lg bg-foreground px-2.5 py-1.5 text-xs font-medium whitespace-nowrap text-background shadow-md" style={{ left: tipLeft, top: hovered.y, transform: tipTransform }}>
						{formatDay(hovered.day)} · {describeActivity(hoveredLevel, hoveredInfo)}
					</div>
				)}
			</div>
			<div aria-hidden="true" className="flex items-center justify-end gap-1.5 text-xs text-muted-foreground">
				<span>{tr('Less', 'Menos')}</span>
				<span className="size-3 rounded-[3px] bg-muted ring-1 ring-inset ring-border/60" />
				{([1, 2, 3, 4] as const).map((level) => (
					<span key={level} data-level={level} style={levelStyle(level)} className="size-3 rounded-[3px] bg-primary" />
				))}
				<span>{tr('More', 'Mais')}</span>
			</div>
			<details onToggle={(event) => setShowDetails(event.currentTarget.open)}>
				<summary className="w-fit min-h-11 cursor-pointer rounded-md px-2 py-3 text-sm font-medium text-foreground focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-foreground">
					{tr('View daily study activity', 'Ver atividade diária de estudo')}
				</summary>
				{showDetails && (
					<table className="w-full text-left text-sm text-foreground">
						<caption className="sr-only">{tr('Daily study activity', 'Atividade diária de estudo')}</caption>
						<thead>
							<tr className="border-b">
								<th scope="col" className="px-2 py-1.5">{tr('Date', 'Data')}</th>
								<th scope="col" className="px-2 py-1.5">{tr('Activity', 'Atividade')}</th>
							</tr>
						</thead>
						<tbody>
							{days.filter((day) => day <= today).reverse().map((day) => {
								const info = activity?.[day];
								return (
									<tr key={day} className="border-b">
										<th scope="row" className="px-2 py-1.5 font-normal">{formatDay(day)}</th>
										<td className="px-2 py-1.5">{describeActivity(dayLevel(active.has(day), info), info)}</td>
									</tr>
								);
							})}
						</tbody>
					</table>
				)}
			</details>
		</section>
	);
}
