import { Award, BookCheck, CalendarCheck, ChartNoAxesColumn, Check, Lock, Timer } from 'lucide-react';
import { type ReactNode, useMemo, useState } from 'react';
import { Link } from 'react-router';
import { ActivityHeatmap } from '@/components/activity-heatmap';
import { Panel } from '@/components/panel';
import { progressLabel, ProgressBar, routerPath } from '@/components/course-card';
import { Button } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
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
	const focusFailed = Boolean(focus.error);
	const noFocus = !focusFailed && weekDays.every((day) => day.minutes === 0);
	const lessonsDone = library.entries.reduce((sum, entry) => sum + entry.done, 0);
	const startedCourses = library.entries.filter((entry) => entry.status !== 'not_started').length;
	const resume = library.resume[0];
	const focusHref = resume
		? routerPath(href('course', resume.location.courseId, resume.location.lesson.path, resume.progress.positionSeconds))
		: routerPath(href('home'));

	return (
		<div className="space-y-6">
			<PageHeader icon={ChartNoAxesColumn} title={tr('Your progress', 'Seu progresso')} description={`${tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)} · ${game.level.xp} XP`} />

			<dl aria-label={tr('At a glance', 'Visão geral')} className="grid gap-4 sm:grid-cols-3">
				<Metric tone="violet" icon={<BookCheck />} label={tr('Lessons completed (all time)', 'Aulas concluídas (total)')} value={String(lessonsDone)} caption={tr(`In total, across ${startedCourses} ${startedCourses === 1 ? 'course' : 'courses'} you started.`, `No total, em ${startedCourses} ${startedCourses === 1 ? 'curso iniciado' : 'cursos iniciados'}.`)} />
				<Metric tone="green" icon={<CalendarCheck />} progress={goal > 0 ? Math.min(1, study.weekActiveDays / goal) : 0} label={tr('Active days (this week)', 'Dias ativos (esta semana)')} value={String(Math.min(study.weekActiveDays, goal))} suffix={tr(`of ${goal}-day goal`, `da meta de ${goal} dias`)} caption={`${missing === 0 ? tr('Goal reached. ', 'Meta atingida. ') : ''}${tr(`Streak: ${study.currentStreak} ${study.currentStreak === 1 ? 'day' : 'days'}. Busiest day: ${best ? best.label : 'none yet'}.`, `Sequência: ${study.currentStreak} ${study.currentStreak === 1 ? 'dia' : 'dias'}. Dia mais forte: ${best ? best.label : 'nenhum ainda'}.`)}`} />
				<Metric tone="amber" icon={<Timer />} label={range === 'week' ? tr('Focus time (this week)', 'Tempo de foco (esta semana)') : tr('Focus time (all time)', 'Tempo de foco (total)')} value={formatMinutes(total)} caption={`${range === 'week' ? tr('This week.', 'Nesta semana.') : tr('In total.', 'No total.')} ${tr('Only the Pomodoro timer counts here.', 'Só o cronômetro Pomodoro conta aqui.')}`} />
			</dl>

			<div className="grid gap-6 lg:grid-cols-12 lg:items-stretch">
				<div className="order-1 flex min-w-0 flex-col gap-6 lg:col-span-8">
				<Panel className="space-y-4 p-5 sm:p-6"><section aria-labelledby="focus-heading" className="space-y-4">
					<div className="flex flex-wrap items-baseline justify-between gap-2">
						<h2 id="focus-heading" className="text-lg font-bold">{range === 'week' ? tr('Focus time by day', 'Tempo de foco por dia') : tr('Weekly breakdown', 'Detalhamento semanal')}</h2>
						<div role="group" aria-label={tr('Period', 'Período')} className="flex gap-1 rounded-2xl border bg-card p-1">
							{([['week', tr('This week', 'Esta semana')], ['all', tr('All time', 'Todo o tempo')]] as const).map(([value, text]) => (
								<button key={value} type="button" aria-pressed={range === value} onClick={() => setRange(value)} className={cn('min-h-11 rounded-xl px-4 text-sm font-semibold focus-visible:ring-3 focus-visible:ring-ring/60 focus-visible:outline-none', range === value ? 'bg-brand-tint text-foreground ring-1 ring-primary' : 'text-muted-foreground hover:text-foreground')}>{text}</button>
							))}
				</div>
					</div>
					{focus.isPending ? (
						<p role="status" className="text-sm text-muted-foreground">{tr('Loading focus statistics…', 'Carregando estatísticas de foco…')}</p>
					) : focusFailed ? (
						<p role="alert" className="text-sm text-destructive">{tr('Focus statistics could not be loaded.', 'Não foi possível carregar as estatísticas de foco.')}</p>
					) : (
						<FocusChart days={weekDays} />
					)}
					{noFocus && (
						<Link to={focusHref} className="inline-flex h-11 items-center rounded-lg bg-primary px-5 text-sm font-semibold text-primary-foreground hover:bg-brand-deep focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none">
							{tr('Start a focus session', 'Iniciar uma sessão de foco')}
						</Link>
					)}
				</section></Panel>
				<CoursesPanel entries={library.entries} />
				<Panel><ActivityHeatmap today={study.today} activeDays={study.activeDaysYear} /></Panel>
				</div>

				<div className="order-2 flex min-w-0 flex-col gap-6 lg:col-span-4">
				<Panel className="min-w-0 p-5 sm:p-6"><section aria-labelledby="missions-heading" className="space-y-3">
					<h2 id="missions-heading" className="text-lg font-bold">{tr('Today', 'Hoje')}</h2>
					<ul className="divide-y border-y">
						{game.missions.map((mission) => {
							const done = mission.progress >= mission.target;
							return (
								<li key={mission.id} className="flex flex-wrap items-start gap-3 py-3">
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
				<Panel className="flex-1 p-5 sm:p-6"><section aria-labelledby="badges-heading" className="space-y-3">
					<h2 id="badges-heading" className="text-lg font-bold">{tr('Badges', 'Emblemas')}</h2>
					<ul className="grid grid-cols-3 gap-x-2 gap-y-4 lg:grid-cols-2 xl:grid-cols-3">
						{game.badges.map((badge) => <BadgeTile key={badge.id} badge={badge} />)}
					</ul>
				</section></Panel>
				</div>

			</div>
		</div>
	);
}

/** One number with its label and what it counts, so lessons, active days and focus time are never confused. */
function Metric({ label, value, suffix, caption, icon, tone, progress }: { label: string; value: string; suffix?: string; caption: string; icon?: ReactNode; tone?: 'amber' | 'violet' | 'green'; progress?: number }) {
	const percent = typeof progress === 'number' ? Math.round(Math.min(1, Math.max(0, progress)) * 100) : null;
	return (
		<Panel className={cn('flex flex-col justify-between gap-2 p-5', tone && `stat-${tone}`)}>
			<dt className="flex items-center justify-between gap-2 text-sm font-semibold text-foreground/85">
				{label}
				{icon && (
					<span data-stat-icon aria-hidden="true" className="inline-flex size-7 shrink-0 items-center justify-center rounded-lg border border-border/80 bg-panel/70 text-muted-foreground [&>svg]:size-3.5">
						{icon}
					</span>
				)}
			</dt>
			<dd className="space-y-1.5">
				<p className="font-heading text-4xl leading-none font-bold tabular-nums">{value}{suffix && <span className="ml-2 font-sans text-base font-medium text-muted-foreground">{suffix}</span>}</p>
				<p className="text-sm text-muted-foreground">{caption}</p>
				{percent !== null && (
					<div className="h-1.5 overflow-hidden rounded-full bg-muted" aria-hidden="true">
						<div className="h-full rounded-full bg-primary transition-[width] duration-300" style={{ width: `${percent}%` }} />
					</div>
				)}
			</dd>
		</Panel>
	);
}

type LibraryEntries = ReturnType<typeof buildLibrary>['entries'];

/** Where the learner stands in the library: lessons done, courses under way and finished, most recent first. */
function CoursesPanel({ entries }: { entries: LibraryEntries }) {
	const finished = entries.filter((entry) => entry.status === 'completed').length;
	const active = entries.filter((entry) => entry.status === 'in_progress').sort((a, b) => (b.lastStudiedAt ?? 0) - (a.lastStudiedAt ?? 0));
	return (
		<Panel className="space-y-4 p-5 sm:p-6"><section aria-labelledby="courses-heading" className="space-y-4">
			<div className="flex flex-wrap items-end justify-between gap-2">
				<div><h2 id="courses-heading" className="text-lg font-bold">{tr('Courses', 'Cursos')}</h2><p className="text-sm text-muted-foreground">{tr(`${active.length} in progress · ${finished} finished`, `${active.length} em andamento · ${finished} concluídos`)}</p></div>
				<Link to={routerPath(href('home'))} className="inline-flex min-h-11 items-center text-sm font-semibold text-primary hover:underline">{tr('All courses', 'Todos os cursos')}</Link>
			</div>
			{active.length === 0 ? (
				<p className="text-sm text-muted-foreground">{tr('No course under way. Open a lesson and it shows up here.', 'Nenhum curso em andamento. Abra uma aula e ele aparece aqui.')}</p>
			) : (
				<ul className="divide-y">
					{active.slice(0, 6).map((entry) => (
						<li key={entry.courseId} className="relative space-y-2 py-3 first:pt-0 last:pb-0">
							<Link to={routerPath(href('course', entry.courseId))} title={entry.course.title} className="line-clamp-1 font-semibold after:absolute after:inset-0 after:content-[''] hover:underline">{entry.course.title}</Link>
							<ProgressBar value={entry.done / Math.max(1, entry.total)} label={progressLabel(entry)} />
						</li>
					))}
				</ul>
			)}
		</section></Panel>
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
	const height = 170;
	const top = 28;
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
			<table className="sr-only" aria-label={tr('Focus minutes by day', 'Minutos de foco por dia')}>
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
