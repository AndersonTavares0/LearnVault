import { Award, ChartNoAxesColumn, Check, Lock } from 'lucide-react';
import { useMemo, useState } from 'react';
import { Link } from 'react-router';
import { ActivityHeatmap } from '@/components/activity-heatmap';
import { Eyebrow, Panel, Stat } from '@/components/panel';
import { ProgressBar, routerPath } from '@/components/course-card';
import { Button } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { formatCount } from '@/lib/catalog';
import { buildGamification, type Badge } from '@/lib/gamification';
import { getLocale, tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { studySyncSupported } from '@/lib/player';
import { href } from '@/lib/router';
import { cn } from '@/lib/utils';
import { useCatalog, useClaimMission, useFavorites, useFocusSummary, useRecent, useRewards } from '@/app/data';
import { PageHeader } from '@/app/shell';

const weekday = (day: string) => new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { weekday: 'short' });

/**
 * Progress: the week's focus and a glance at the habit, then study days beside today's missions,
 * then the badges. XP and level stay in one line under the page title.
 */
export function ProgressPage() {
	useLocale();
	const rewards = useRewards();
	const focus = useFocusSummary();
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const claim = useClaimMission();
	const [range, setRange] = useState<'week' | 'all'>('week');
	const library = useMemo(() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: new Set(favorites.data?.courseIds ?? []) }), [catalog.data, recent.data, favorites.data]);
	const game = rewards.data ? buildGamification({ rewards: rewards.data, completedCourses: library.entries.filter((entry) => entry.status === 'completed').length }) : null;

	if (!studySyncSupported) return <PageHeader title={tr('Your progress', 'Seu progresso')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	if (rewards.isPending) return <><PageHeader title={tr('Your progress', 'Seu progresso')} /><p role="status" className="text-muted-foreground">{tr('Loading your progress…', 'Carregando seu progresso…')}</p></>;
	// A failed refetch keeps showing the last good data.
	if (!rewards.data || !game) return <><PageHeader title={tr('Your progress', 'Seu progresso')} /><p role="alert" className="text-destructive">{rewards.error?.message ?? tr('Could not load your progress.', 'Não foi possível carregar seu progresso.')}</p></>;

	const study = rewards.data.study;
	const goal = rewards.data.weeklyGoal;
	const missing = Math.max(0, goal - study.weekActiveDays);

	const minutes = new Map((focus.data?.days ?? []).map((day) => [day.day, day.minutes]));
	const weekDays = study.days.map((day) => ({ day: day.day, label: weekday(day.day), minutes: minutes.get(day.day) ?? 0, active: day.active }));
	const total = range === 'week' ? focus.data?.weekMinutes ?? weekDays.reduce((sum, day) => sum + day.minutes, 0) : focus.data?.totalMinutes ?? 0;
	const best = weekDays.reduce<(typeof weekDays)[number] | null>((top, day) => (day.minutes > (top?.minutes ?? 0) ? day : top), null);
	const noFocus = weekDays.every((day) => day.minutes === 0);
	const resume = library.resume[0];
	const focusHref = resume
		? routerPath(href('course', resume.location.courseId, resume.location.lesson.path, resume.progress.positionSeconds))
		: routerPath(href('home'));

	return (
		<div className="space-y-6">
			<PageHeader icon={ChartNoAxesColumn} title={tr('Your progress', 'Seu progresso')} description={`${tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)} · ${game.level.xp} XP`} />

			<div className="grid items-start gap-6 lg:grid-cols-12">
				<Panel className="space-y-6 lg:col-span-8">
					<div className="flex flex-wrap items-start justify-between gap-4">
						<div className="min-w-0">
							<Eyebrow>{tr('This week', 'Esta semana')}</Eyebrow>
							<p className="mt-2 font-heading text-4xl font-semibold tabular-nums sm:text-5xl">{formatMinutes(total)}<span className="ml-3 font-sans text-base font-medium text-muted-foreground">{range === 'week' ? tr('of focus this week', 'de foco nesta semana') : tr('of focus in all', 'de foco no total')}</span></p>
							<h2 id="week-heading" className="mt-3 text-base font-medium">{tr(`${Math.min(study.weekActiveDays, goal)} of ${goal} days this week`, `${Math.min(study.weekActiveDays, goal)} de ${goal} dias nesta semana`)}</h2>
							<p className="text-sm text-muted-foreground">{missing === 0 ? tr('You reached your weekly goal.', 'Você atingiu a meta semanal.') : tr(`${formatCount(missing, 'more day', 'more days')} to reach your goal.`, `Mais ${formatCount(missing, 'dia', 'dias')} para a meta.`)}</p>
							<div role="progressbar" aria-valuemin={0} aria-valuemax={goal} aria-valuenow={Math.min(study.weekActiveDays, goal)} aria-label={tr('Weekly goal', 'Meta semanal')} className="mt-4 flex w-full max-w-xs gap-1">
								{Array.from({ length: goal }, (_, index) => <span key={index} className={cn('h-2 flex-1 rounded-full', index < study.weekActiveDays ? 'bg-primary' : 'bg-muted')} />)}
							</div>
						</div>
						<div role="group" aria-label={tr('Period', 'Período')} className="flex gap-1 rounded-2xl border bg-card p-1">
							{([['week', tr('This week', 'Esta semana')], ['all', tr('All time', 'Todo o tempo')]] as const).map(([value, text]) => (
								<button key={value} type="button" aria-pressed={range === value} onClick={() => setRange(value)} className={cn('min-h-11 rounded-xl px-4 text-sm font-semibold focus-visible:ring-3 focus-visible:ring-ring/60 focus-visible:outline-none', range === value ? 'bg-brand-tint text-foreground ring-1 ring-primary/40' : 'text-muted-foreground hover:text-foreground')}>{text}</button>
							))}
						</div>
					</div>

					<div className="rounded-[1.75rem] border border-panel-edge bg-panel p-5">
						<Eyebrow>{tr('Focus time, this week', 'Tempo de foco, nesta semana')}</Eyebrow>
						<FocusChart days={weekDays} />
						{noFocus && (
							<Link to={focusHref} className="mt-4 inline-flex h-11 items-center rounded-lg bg-primary px-5 text-sm font-semibold text-primary-foreground hover:bg-brand-deep focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
								{tr('Start a focus session', 'Iniciar uma sessão de foco')}
							</Link>
						)}
					</div>
				</Panel>

				<Panel className="space-y-4 lg:col-span-4">
					<Eyebrow>{tr('At a glance', 'Visão geral')}</Eyebrow>
					<div className="grid gap-3">
						<Stat tone="amber" label={tr('Busiest day', 'Dia mais forte')} value={best ? best.label : '–'} note={best && best.minutes > 0 ? formatMinutes(best.minutes) : undefined} />
						<Stat tone="violet" label={tr('Days studied', 'Dias estudados')} value={study.weekActiveDays} unit={tr('of 7', 'de 7')} />
						<Stat tone="green" label={tr('Day streak', 'Dias seguidos')} value={study.currentStreak} unit={study.currentStreak === 1 ? tr('day', 'dia') : tr('days', 'dias')} />
					</div>
				</Panel>
			</div>

			<div className="grid items-start gap-6 lg:grid-cols-12">
				<Panel className="lg:col-span-8">
					<ActivityHeatmap today={study.today} activeDays={study.activeDaysYear} />
				</Panel>

				<Panel className="min-w-0 lg:col-span-4"><section aria-labelledby="missions-heading" className="space-y-3">
					<h2 id="missions-heading" className="font-heading text-2xl font-semibold">{tr('Today', 'Hoje')}</h2>
					<ul className="divide-y border-y">
						{game.missions.map((mission) => {
							const done = mission.progress >= mission.target;
							return (
								<li key={mission.id} className="flex flex-wrap items-start gap-3 py-4">
									<span className={cn('mt-0.5 grid size-6 shrink-0 place-items-center rounded-full border-2', done ? 'border-success bg-success text-white dark:text-[var(--background)]' : 'border-border-strong')}>{done && <Check className="size-4" aria-hidden="true" />}</span>
									<div className="min-w-0 flex-1">
										<p className="font-semibold">{mission.title}</p>
										<ProgressBar value={mission.progress / mission.target} label={`${Math.min(mission.progress, mission.target)} / ${mission.target}`} className="mt-2" />
									</div>
									{mission.claimed ? (
										<p className="flex min-h-11 items-center gap-1 text-sm text-muted-foreground"><Check className="size-4" aria-hidden="true" />{tr(`${mission.rewardXp} XP`, `${mission.rewardXp} XP`)}</p>
									) : done ? (
										<Button size="sm" disabled={claim.isPending} onClick={() => claim.mutate(mission.id)}>{tr(`Claim ${mission.rewardXp} XP`, `Resgatar ${mission.rewardXp} XP`)}</Button>
									) : null}
								</li>
							);
						})}
					</ul>
				</section></Panel>
			</div>

			<Panel><section aria-labelledby="badges-heading" className="space-y-3">
				<h2 id="badges-heading" className="font-heading text-2xl font-semibold">{tr('Badges', 'Emblemas')}</h2>
				<ul className="grid grid-cols-2 gap-x-4 sm:grid-cols-3 lg:grid-cols-6">
					{game.badges.map((badge) => <BadgeTile key={badge.id} badge={badge} />)}
				</ul>
			</section></Panel>
		</div>
	);
}

type ChartDay = { day: string; label: string; minutes: number; active: boolean };

/** Hours and minutes, short: "1h 5m", "35m". */
function formatMinutes(total: number): string {
	const minutes = Math.max(0, Math.round(total));
	const hours = Math.floor(minutes / 60);
	return hours > 0 ? `${hours}h ${minutes % 60}m` : `${minutes}m`;
}

/**
 * Focus minutes for each day of the week as a smooth area, with the busiest day labelled. The
 * numbers are also in a table for screen readers, so nothing depends on seeing the curve.
 */
function FocusChart({ days }: { days: ChartDay[] }) {
	const width = 700;
	const empty = days.every((day) => day.minutes === 0);
	const height = empty ? 24 : 170;
	const top = empty ? 8 : 28;
	const max = Math.max(1, ...days.map((day) => day.minutes));
	const step = days.length > 1 ? (width - 40) / (days.length - 1) : 0;
	const points = days.map((day, index) => ({ x: 20 + index * step, y: top + (1 - day.minutes / max) * (height - top - 8) }));
	// A smooth line through the points: each segment's control points sit a third of the way along.
	const line = points.map((point, index) => {
		if (index === 0) return `M${point.x},${point.y}`;
		const before = points[index - 1];
		const dx = (point.x - before.x) / 3;
		return `C${before.x + dx},${before.y} ${point.x - dx},${point.y} ${point.x},${point.y}`;
	}).join(' ');
	const area = points.length ? `${line} L${points.at(-1)!.x},${height} L${points[0].x},${height} Z` : '';
	const peak = days.findIndex((day) => day.minutes === max && day.minutes > 0);
	return (
		<div>
			{!empty && (
				<svg viewBox={`0 0 ${width} ${height + 24}`} role="img" aria-label={tr('Chart of focus minutes for each day this week; the same numbers are in the table below.', 'Gráfico dos minutos de foco em cada dia desta semana; os mesmos números estão na tabela abaixo.')} className="mt-2 w-full">
				<path d={area} className="fill-brand-soft" />
				<path d={line} fill="none" strokeWidth="3" strokeLinecap="round" className="stroke-primary" />
				{peak >= 0 && (
					<g>
						<circle cx={points[peak].x} cy={points[peak].y} r="5" className="fill-card stroke-primary" strokeWidth="3" />
						<text x={points[peak].x} y={Math.max(14, points[peak].y - 12)} textAnchor="middle" fontSize="13" fontWeight="700" className="fill-foreground">{formatMinutes(days[peak].minutes)}</text>
					</g>
				)}
				{days.map((day, index) => (
					<g key={day.day}>
						<text x={points[index].x} y={height + 16} textAnchor="middle" fontSize="12" className={day.active ? 'fill-foreground font-bold' : 'fill-muted-foreground'}>{day.label}</text>
					</g>
				))}
			</svg>
			)}
			{empty && <p className="mt-2 text-sm text-muted-foreground">{tr('No focus time yet. Start the timer in a lesson and it will show up here.', 'Ainda sem tempo de foco. Inicie o cronômetro numa aula e ele aparece aqui.')}</p>}
			<table className="sr-only">
				<caption>{tr('Focus minutes by day', 'Minutos de foco por dia')}</caption>
				<thead><tr><th scope="col">{tr('Day', 'Dia')}</th><th scope="col">{tr('Minutes', 'Minutos')}</th></tr></thead>
				<tbody>{days.map((day) => <tr key={day.day}><th scope="row">{day.label}</th><td>{day.minutes}</td></tr>)}</tbody>
			</table>
		</div>
	);
}

/** A badge: filled when earned, a dashed outline with its count while locked. */
function BadgeTile({ badge }: { badge: Badge }) {
	return (
		<li className="flex flex-col items-center gap-1.5 px-2 py-4 text-center">
			<span className={cn('grid size-14 place-items-center rounded-full', badge.earned ? 'bg-reward-fill text-reward-fill-foreground ring-1 ring-reward-fill/40' : 'border-2 border-dashed border-border-strong text-muted-foreground')}>
				{badge.earned ? <Award className="size-6" aria-hidden="true" /> : <Lock className="size-5" aria-hidden="true" />}
			</span>
			<p className="text-sm leading-tight font-semibold">{badge.title}<span className="sr-only">{badge.earned ? tr(', earned', ', conquistada') : tr(', locked', ', bloqueada')}</span></p>
			<p className="text-sm text-muted-foreground tabular-nums">{badge.earned ? tr('Earned', 'Conquistada') : `${badge.progress}/${badge.target}`}</p>
		</li>
	);
}
