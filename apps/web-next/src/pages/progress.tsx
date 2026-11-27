import { ArrowRight, Award, ChartNoAxesColumn, Check, Lock } from 'lucide-react';
import { type ReactNode, useMemo, useState } from 'react';
import { Link } from 'react-router';
import { ActivityHeatmap, type DayActivity } from '@/components/activity-heatmap';
import { CourseCover } from '@/components/course-cover';
import { LessonSegments } from '@/components/lesson-segments';
import { Panel } from '@/components/panel';
import { progressLabel, routerPath } from '@/components/course-card';
import { Button, buttonVariants } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { buildGamification, type Badge } from '@/lib/gamification';
import { getLocale, tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { studySyncSupported } from '@/lib/player';
import { href } from '@/lib/router';
import { cn } from '@/lib/utils';
import { useCatalog, useClaimMission, useFavorites, useFocusSessions, useFocusSummary, useRecent, useRewards, useSession } from '@/app/data';
import { PageHeader } from '@/app/shell';
import type { FocusSessionRecord } from '@/lib/transport';

const weekday = (day: string) => new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { weekday: 'short' });
const localDayKey = (seconds: number) => {
	const d = new Date(seconds * 1000);
	return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
};

/**
 * Progress: the week's focus and a glance at the habit, then study days beside today's missions,
 * then the badges. XP and level stay in one line under the page title.
 */
export function ProgressPage() {
	useLocale();
	const rewards = useRewards();
	const profile = useSession().data?.user?.userId ?? '';
	const focus = useFocusSummary();
	const sessions = useFocusSessions();
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const claim = useClaimMission();
	const [range, setRange] = useState<'week' | 'all'>('week');
	const [showAllSessions, setShowAllSessions] = useState(false);
	const library = useMemo(() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: new Set(favorites.data?.courseIds ?? []) }), [catalog.data, recent.data, favorites.data]);
	const game = rewards.data ? buildGamification({ rewards: rewards.data, completedCourses: library.entries.filter((entry) => entry.status === 'completed').length }) : null;
	const dayActivity = useMemo(() => {
		const map: Record<string, DayActivity> = {};
		const ensure = (day: string) => (map[day] ??= { score: 0, lessons: 0, completed: 0, focusMinutes: 0 });
		for (const item of recent.data ?? []) {
			if (!item.updatedAt) continue;
			const entry = ensure(localDayKey(item.updatedAt));
			entry.lessons = (entry.lessons ?? 0) + 1;
			if (item.completed) {
				entry.completed = (entry.completed ?? 0) + 1;
				entry.score = (entry.score ?? 0) + 2;
			} else {
				entry.score = (entry.score ?? 0) + 1;
			}
		}
		for (const day of focus.data?.days ?? []) {
			if (day.minutes <= 0) continue;
			const entry = ensure(day.day);
			entry.focusMinutes = (entry.focusMinutes ?? 0) + day.minutes;
			entry.score = (entry.score ?? 0) + Math.max(1, Math.ceil(day.minutes / 15));
		}
		for (const day of rewards.data?.study.activeDaysYear ?? []) {
			const entry = ensure(day);
			entry.score = Math.max(1, entry.score ?? 0);
		}
		return map;
	}, [recent.data, focus.data?.days, rewards.data?.study.activeDaysYear]);
	const allTimeDays = useMemo(() => buildAllTimeChartDays(sessions.data ?? [], rewards.data?.study.today ?? ''), [sessions.data, rewards.data?.study.today]);

	if (!studySyncSupported) return <PageHeader title={tr('Your progress', 'Seu progresso')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	if (rewards.isPending) return <><PageHeader title={tr('Your progress', 'Seu progresso')} /><p role="status" className="text-muted-foreground">{tr('Loading your progress…', 'Carregando seu progresso…')}</p></>;
	// A failed refetch keeps showing the last good data.
	if (!rewards.data || !game) return <><PageHeader title={tr('Your progress', 'Seu progresso')} /><p role="alert" className="text-destructive">{rewards.error?.message ?? tr('Could not load your progress.', 'Não foi possível carregar seu progresso.')}</p></>;

	const study = rewards.data.study;
	const goal = rewards.data.weeklyGoal;
	const missing = Math.max(0, goal - study.weekActiveDays);

	const minutes = new Map((focus.data?.days ?? []).map((day) => [day.day, day.minutes]));
	const weekDays = study.days.map((day) => ({ day: day.day, label: weekday(day.day), minutes: minutes.get(day.day) ?? 0, active: day.active }));
	const chartDays = range === 'week' ? weekDays : (allTimeDays.length > 0 ? allTimeDays : weekDays);
	const total = range === 'week' ? focus.data?.weekMinutes ?? weekDays.reduce((sum, day) => sum + day.minutes, 0) : focus.data?.totalMinutes ?? 0;
	const best = chartDays.reduce<(typeof chartDays)[number] | null>((top, day) => (day.minutes > (top?.minutes ?? 0) ? day : top), null);
	const focusFailed = Boolean(focus.error);
	const noFocus = !focusFailed && chartDays.every((day) => day.minutes === 0);
	const lessonsDone = library.entries.reduce((sum, entry) => sum + entry.done, 0);
	const startedCourses = library.entries.filter((entry) => entry.status !== 'not_started').length;
	const resume = library.resume[0];
	const focusHref = resume
		? routerPath(href('course', resume.location.courseId, resume.location.lesson.path, resume.progress.positionSeconds))
		: routerPath(href('home'));

	return (
		<div>
			<PageHeader icon={ChartNoAxesColumn} title={tr('Your progress', 'Seu progresso')} description={game.level.xp > 0
				? <span className="inline-flex flex-wrap gap-x-4 gap-y-1"><span>{tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)}</span><span>{game.level.xp} XP</span><span>{tr(`${game.level.xpToNext} XP to level ${game.level.level + 1}`, `${game.level.xpToNext} XP para o nível ${game.level.level + 1}`)}</span></span>
				: tr(`Level ${game.level.level} · 0 of ${game.level.xpToNext} XP until the next level`, `Nível ${game.level.level} · 0 de ${game.level.xpToNext} XP para o próximo nível`)} />

			<div className="mb-3 flex flex-wrap items-center justify-between gap-2">
				<h2 className="text-xs font-semibold tracking-wider uppercase text-muted-foreground">{tr('At a glance', 'Visão geral')}</h2>
				<div role="group" aria-label={tr('Focus time period', 'Período do tempo de foco')} className="inline-flex rounded-[var(--radius)] border border-[var(--study-rule)] bg-panel p-0.5">
					{([['week', tr('This week', 'Esta semana')], ['all', tr('All time', 'Todo o tempo')]] as const).map(([value, text]) => (
						<button key={value} type="button" aria-pressed={range === value} onClick={() => setRange(value)} className={cn('min-h-11 rounded-[calc(var(--radius)-2px)] px-3 text-xs font-medium transition-colors focus-visible:ring-3 focus-visible:ring-ring/60 focus-visible:outline-none sm:min-h-9 sm:text-sm', range === value ? 'bg-card font-semibold text-foreground shadow-2xs' : 'text-muted-foreground hover:text-foreground')}>{text}</button>
					))}
				</div>
			</div>
			<dl aria-label={tr('At a glance', 'Visão geral')} className="study-surface study-metrics mb-6 grid grid-cols-2 sm:grid-cols-3 [&>*:last-child]:max-sm:col-span-2">
				<Metric label={tr('Lessons completed (all time)', 'Aulas concluídas (total)')} value={String(lessonsDone)} caption={startedCourses
					? tr(`In total, across ${startedCourses} ${startedCourses === 1 ? 'course' : 'courses'} you started.`, `No total, em ${startedCourses} ${startedCourses === 1 ? 'curso iniciado' : 'cursos iniciados'}.`)
					: tr('No courses started yet.', 'Nenhum curso iniciado.')} />
				<Metric label={tr('Active days (this week)', 'Dias ativos (esta semana)')} value={String(Math.min(study.weekActiveDays, goal))} suffix={tr(`of ${goal}-day goal`, `da meta de ${goal} dias`)} caption={study.currentStreak || best
					? `${missing === 0 ? tr('Goal reached. ', 'Meta atingida. ') : ''}${tr(`Streak: ${study.currentStreak} ${study.currentStreak === 1 ? 'day' : 'days'}. Busiest day: ${best ? best.label : 'none yet'}.`, `Sequência: ${study.currentStreak} ${study.currentStreak === 1 ? 'dia' : 'dias'}. Dia com mais foco: ${best ? best.label : 'nenhum ainda'}.`)}`
					: tr('No study streak yet.', 'Sem sequência de estudo.')} />
				<Metric label={range === 'week' ? tr('Focus time (this week)', 'Tempo de foco (esta semana)') : tr('Focus time (all time)', 'Tempo de foco (total)')} value={formatMinutes(total)} caption={`${range === 'week' ? '' : `${tr('In total.', 'No total.')} `}${tr('Only the Pomodoro timer counts here.', 'Só o cronômetro Pomodoro conta aqui.')}`} />
			</dl>

			<div className="grid gap-x-10 lg:grid-cols-12">
				<CoursesPanel entries={library.entries} profile={profile} />

				<Panel variant="open" className="lg:col-span-4"><section aria-labelledby="missions-heading" className="space-y-3">
					<h2 id="missions-heading" className="text-[15px] font-semibold">{tr('Today', 'Hoje')}</h2>
					<ul className="divide-y divide-[var(--study-rule)]">
						{game.missions.map((mission) => {
							const done = mission.progress >= mission.target;
							return (
								<li key={mission.id} className="flex items-center gap-3 py-2.5">
									<span className={cn('grid size-5 shrink-0 place-items-center rounded-full border', done ? 'border-primary bg-primary text-primary-foreground' : 'border-border-strong')}>{done && <Check className="size-3" aria-hidden="true" />}</span>
									<p className="min-w-0 flex-1 text-sm">{mission.title}</p>
									{mission.claimed ? (
										<span className="inline-flex shrink-0 items-center gap-1 text-xs font-medium text-muted-foreground"><Check className="size-3.5" aria-hidden="true" />{mission.rewardXp} XP</span>
									) : done ? (
										<Button size="sm" variant="outline" disabled={claim.isPending} onClick={() => claim.mutate(mission.id)}>{tr(`Claim ${mission.rewardXp} XP`, `Resgatar ${mission.rewardXp} XP`)}</Button>
									) : (
										<span className="study-timecode shrink-0 text-muted-foreground">{Math.min(mission.progress, mission.target)} / {mission.target}</span>
									)}
								</li>
							);
						})}
					</ul>
				</section></Panel>

				<Panel variant="open" className="lg:col-span-12"><ActivityHeatmap today={study.today} activeDays={study.activeDaysYear} activity={dayActivity} /></Panel>

				<Panel variant="open" className="flex flex-col lg:col-span-8"><section aria-labelledby="focus-heading" className="flex flex-1 flex-col space-y-4">
					<h2 id="focus-heading" className="text-[15px] font-semibold">{range === 'week' ? tr('Weekly breakdown', 'Detalhamento semanal') : tr('All-time focus', 'Foco em todo o tempo')}</h2>
					{focus.isPending ? (
						<p role="status" className="text-sm text-muted-foreground">{tr('Loading focus statistics…', 'Carregando estatísticas de foco…')}</p>
					) : focusFailed ? (
						<p role="alert" className="text-sm text-destructive">{tr('Focus statistics could not be loaded.', 'Não foi possível carregar as estatísticas de foco.')}</p>
					) : (
						<FocusChart range={range} days={chartDays} emptyAction={noFocus ? <Link to={focusHref} className={buttonVariants({ variant: 'outline' })}>{tr('Start a focus session', 'Iniciar uma sessão de foco')}</Link> : undefined} />
					)}
					{!focusFailed && (sessions.data?.length ?? 0) > 0 && (
						<FocusSessionLog sessions={sessions.data!} expanded={showAllSessions} onToggle={() => setShowAllSessions((value) => !value)} />
					)}
				</section></Panel>

				<Panel variant="open" className="lg:col-span-4"><section aria-labelledby="badges-heading" className="space-y-3">
					<h2 id="badges-heading" className="text-[15px] font-semibold">{tr('Badges', 'Emblemas')}<span className="ml-2 text-sm font-normal text-muted-foreground tabular-nums">{tr(`${game.badges.filter((badge) => badge.earned).length} of ${game.badges.length}`, `${game.badges.filter((badge) => badge.earned).length} de ${game.badges.length}`)}</span></h2>
					<ul className="grid grid-cols-3 gap-x-2 gap-y-3 lg:grid-cols-2 xl:grid-cols-3">
						{game.badges.map((badge) => <BadgeTile key={badge.id} badge={badge} />)}
					</ul>
				</section></Panel>
			</div>
		</div>
	);
}

/** One number with its label and what it counts, so lessons, active days and focus time are never confused. */
function Metric({ label, value, suffix, caption, children }: { label: string; value: string; suffix?: string; caption: string; children?: ReactNode }) {
	return (
		<div className="min-w-0 space-y-2 px-4 py-4 sm:p-5">
			<dt className="text-[0.8125rem] font-medium text-muted-foreground">{label}</dt>
			<dd className="space-y-1.5">
				<div className="flex flex-wrap items-end justify-between gap-x-4 gap-y-3">
					<p className="font-heading text-[clamp(1.65rem,5vw,2.25rem)] leading-none font-semibold tabular-nums">{value}{suffix && <span className="ml-1.5 font-sans text-xs leading-snug font-medium whitespace-nowrap text-muted-foreground sm:text-sm">{suffix}</span>}</p>
					{children}
				</div>
				<p className="max-w-[42ch] text-sm leading-snug text-muted-foreground">{caption}</p>
			</dd>
		</div>
	);
}

type LibraryEntries = ReturnType<typeof buildLibrary>['entries'];

/** Where the learner stands in the library: lessons done, courses under way and finished, most recent first. */
function CoursesPanel({ entries, profile }: { entries: LibraryEntries; profile: string }) {
	const finished = entries.filter((entry) => entry.status === 'completed').length;
	const active = entries.filter((entry) => entry.status === 'in_progress').sort((a, b) => (b.lastStudiedAt ?? 0) - (a.lastStudiedAt ?? 0));
	const firstCourse = entries[0];
	return (
		<Panel variant="open" className="lg:col-span-8"><section aria-labelledby="courses-heading" className="space-y-4">
			<div className="flex flex-wrap items-center justify-between gap-2">
				<div><h2 id="courses-heading" className="text-[15px] font-semibold">{tr('Courses', 'Cursos')}</h2><p className="text-sm text-muted-foreground">{tr(`${active.length} in progress · ${finished} finished`, `${active.length} em andamento · ${finished} concluídos`)}</p></div>
				<Link to={routerPath(href('home'))} className={buttonVariants({ variant: 'ghost', size: 'sm' })}>{tr('All courses', 'Todos os cursos')}<ArrowRight aria-hidden="true" /></Link>
			</div>
			{active.length === 0 ? (
				<div className="space-y-4">
					<p className="text-sm text-muted-foreground">{tr('No course under way. Open a lesson and it shows up here.', 'Nenhum curso em andamento. Abra uma aula e ele aparece aqui.')}</p>
					{firstCourse && (
						<Link to={routerPath(href('course', firstCourse.courseId))} className={buttonVariants({ variant: 'default' })}>
							{tr(`Open ${firstCourse.course.title}`, `Abrir ${firstCourse.course.title}`)}
						</Link>
					)}
				</div>
			) : (
				<ul className="divide-y divide-[var(--study-rule)]">
					{active.slice(0, 6).map((entry) => {
						const studied = entry.lastStudiedAt ? new Date(entry.lastStudiedAt * 1000).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric' }) : null;
						const context = [studied && tr(`Last studied ${studied}`, `Estudado ${studied}`), entry.next && `${tr('next:', 'próxima:')} ${entry.next.title}`].filter(Boolean).join(' · ');
						return (
							<li key={entry.courseId} className="flex items-center gap-4 py-4 first:pt-0 last:pb-0">
								<div className="w-24 shrink-0 sm:w-36">
									<CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} compact className="aspect-video w-full" />
								</div>
								<div className="min-w-0 flex-1 space-y-2">
									<div className="min-w-0 space-y-0.5">
										<Link to={routerPath(href('course', entry.courseId))} title={entry.course.title} className="line-clamp-1 font-semibold text-foreground underline-offset-4 hover:underline">{entry.course.title}</Link>
										<p className="text-xs text-muted-foreground">{progressLabel(entry)}</p>
										{context && <p className="truncate text-xs text-muted-foreground">{context}</p>}
									</div>
									<LessonSegments done={entry.done} total={entry.total} current={entry.done} className="max-w-sm" />
								</div>
								{entry.next && (
									<Link to={routerPath(href('course', entry.courseId, entry.next.path))} className={cn(buttonVariants({ variant: 'outline' }), 'min-h-11 shrink-0 px-4')}>
										{tr('Resume', 'Retomar')}
									</Link>
								)}
							</li>
						);
					})}
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
 * Buckets historical completed focus sessions for the All-time chart: daily up to 14 distinct days,
 * or monthly across longer history, capped cleanly to readable chart points.
 */
function buildAllTimeChartDays(sessions: FocusSessionRecord[], today: string): ChartDay[] {
	const completed = sessions.filter((session) => session.kind === 'focus' && session.completed && session.endedAt > 0);
	if (completed.length === 0) return [];
	const byDay = new Map<string, number>();
	for (const session of completed) {
		const day = localDayKey(session.endedAt);
		byDay.set(day, (byDay.get(day) ?? 0) + Math.floor(session.actualSeconds / 60));
	}
	const sortedDays = [...byDay.keys()].sort();
	const useMonths = sortedDays.length > 14;
	if (!useMonths) {
		const recent = sortedDays.slice(-10);
		return recent.map((day) => ({
			day,
			label: new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric' }),
			minutes: byDay.get(day) ?? 0,
			active: day === today || (byDay.get(day) ?? 0) > 0
		}));
	}
	const byMonth = new Map<string, number>();
	for (const [day, mins] of byDay.entries()) {
		const month = day.slice(0, 7);
		byMonth.set(month, (byMonth.get(month) ?? 0) + mins);
	}
	return [...byMonth.entries()]
		.sort(([a], [b]) => a.localeCompare(b))
		.slice(-12)
		.map(([month, mins]) => ({
			day: month,
			label: new Date(`${month}-15T12:00:00`).toLocaleDateString(getLocale(), { month: 'short' }),
			minutes: mins,
			active: mins > 0
		}));
}

/**
 * Focus minutes for each day of the week (or all-time buckets) as a smooth area, with the peak labelled.
 * The numbers are also in a table for screen readers, so nothing depends on seeing the curve.
 */
function FocusChart({ range, days, emptyAction }: { range: 'week' | 'all'; days: ChartDay[]; emptyAction?: ReactNode }) {
	const width = 700;
	const empty = days.every((day) => day.minutes === 0);
	const height = 170;
	const top = 28;
	const max = Math.max(1, ...days.map((day) => day.minutes));
	const step = days.length > 1 ? (width - 40) / (days.length - 1) : 0;
	const points = days.map((day, index) => ({ x: days.length === 1 ? width / 2 : 20 + index * step, y: top + (1 - day.minutes / max) * (height - top - 8) }));
	// A smooth line through the points: each segment's control points sit a third of the way along.
	const line = points.map((point, index) => {
		if (index === 0) return `M${point.x},${point.y}`;
		const before = points[index - 1];
		const dx = (point.x - before.x) / 3;
		return `C${before.x + dx},${before.y} ${point.x - dx},${point.y} ${point.x},${point.y}`;
	}).join(' ');
	const area = points.length > 1 ? `${line} L${points.at(-1)!.x},${height} L${points[0].x},${height} Z` : '';
	const peak = days.findIndex((day) => day.minutes === max && day.minutes > 0);
	const chartLabel = range === 'week'
		? tr('Chart of focus minutes for each day this week; the same numbers are in the table below.', 'Gráfico dos minutos de foco em cada dia desta semana; os mesmos números estão na tabela abaixo.')
		: tr('Chart of focus minutes across recorded study periods; the same numbers are in the table below.', 'Gráfico dos minutos de foco ao longo dos períodos registrados; os mesmos números estão na tabela abaixo.');
	return (
		<div className="flex flex-1 flex-col">
			{!empty ? (
				<svg viewBox={`0 0 ${width} ${height + 24}`} role="img" aria-label={chartLabel} className="mt-2 w-full">
				{area && <path d={area} className="fill-brand-tint" />}
				{points.length > 1 && <path d={line} fill="none" strokeWidth="2.5" strokeLinecap="round" className="stroke-primary" />}
				{peak >= 0 && (
					<g>
						<circle cx={points[peak].x} cy={points[peak].y} r="4.5" className="fill-card stroke-primary" strokeWidth="2.5" />
						<text x={points[peak].x} y={Math.max(14, points[peak].y - 12)} textAnchor="middle" fontSize="13" fontWeight="700" className="fill-foreground">{formatMinutes(days[peak].minutes)}</text>
					</g>
				)}
				{days.map((day, index) => (
					<g key={day.day}>
						<text x={points[index].x} y={height + 16} textAnchor="middle" fontSize="12" className={day.active ? 'fill-foreground font-bold' : 'fill-muted-foreground'}>{day.label}</text>
					</g>
				))}
			</svg>
			) : (
				<div className="flex flex-1 flex-col justify-between gap-4 pt-2">
					<div className="flex flex-col items-start gap-2.5 py-2">
						<p className="font-semibold">{tr('No focus time yet', 'Ainda sem tempo de foco')}</p>
						<p className="max-w-md text-sm text-muted-foreground">{tr('Start the timer in a lesson and each session shows up here, day by day.', 'Inicie o cronômetro numa aula e cada sessão aparece aqui, dia a dia.')}</p>
						{emptyAction && <div className="pt-1">{emptyAction}</div>}
					</div>
					{days.length > 0 && (
						<div aria-hidden="true" className="grid grid-cols-7 gap-2 border-t border-[var(--study-rule)] pt-2.5 text-center text-xs text-muted-foreground">
							{days.map((day) => <span key={day.day}>{day.label}</span>)}
						</div>
					)}
				</div>
			)}
			<div className="sr-only">
				<table aria-label={tr('Focus minutes by day', 'Minutos de foco por dia')}>
					<caption>{tr('Focus minutes by day', 'Minutos de foco por dia')}</caption>
					<thead><tr><th scope="col">{tr('Day', 'Dia')}</th><th scope="col">{tr('Minutes', 'Minutos')}</th></tr></thead>
					<tbody>{days.map((day) => <tr key={day.day}><th scope="row">{day.label}</th><td>{day.minutes}</td></tr>)}</tbody>
				</table>
			</div>
		</div>
	);
}

function FocusSessionLog({ sessions, expanded, onToggle }: { sessions: FocusSessionRecord[]; expanded: boolean; onToggle: () => void }) {
	const visible = expanded ? sessions.slice(0, 20) : sessions.slice(0, 5);
	const kindLabel = (kind: FocusSessionRecord['kind']) =>
		kind === 'focus' ? tr('Focus', 'Foco') : kind === 'short' ? tr('Short break', 'Pausa curta') : tr('Long break', 'Pausa longa');
	return (
		<div className="border-t border-[var(--study-rule)] pt-3">
			<div className="flex items-center justify-between gap-2">
				<h3 className="text-xs font-semibold tracking-wider uppercase text-muted-foreground">{tr('Recent sessions', 'Sessões recentes')}</h3>
				{sessions.length > 5 && (
					<Button variant="ghost" size="sm" onClick={onToggle}>{expanded ? tr('Show less', 'Mostrar menos') : tr(`Show ${Math.min(20, sessions.length)}`, `Mostrar ${Math.min(20, sessions.length)}`)}</Button>
				)}
			</div>
			<ul className="mt-2 divide-y divide-[var(--study-rule)] text-sm">
				{visible.map((session) => (
					<li key={session.id} className="flex flex-wrap items-center gap-x-3 gap-y-1 py-2">
						<span className="font-medium text-foreground">{kindLabel(session.kind)}</span>
						<span className="study-timecode text-xs text-muted-foreground">{formatMinutes(session.actualSeconds / 60)}</span>
						<span className={cn('text-xs', session.completed ? 'text-primary' : 'text-muted-foreground')}>
							{session.completed ? tr('Completed', 'Concluída') : tr('Stopped early', 'Encerrada antes')}
						</span>
						<time dateTime={new Date(session.endedAt * 1000).toISOString()} className="ml-auto text-xs text-muted-foreground">
							{new Date(session.endedAt * 1000).toLocaleDateString(getLocale(), { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
						</time>
					</li>
				))}
			</ul>
		</div>
	);
}

/** A badge: filled when earned, a dashed outline with its count while locked. */
function BadgeTile({ badge }: { badge: Badge }) {
	return (
		<li className="flex flex-col items-center gap-1.5 px-1.5 py-2 text-center">
			<span className={cn('grid size-11 place-items-center rounded-full border', badge.earned ? 'border-primary/40 bg-brand-soft text-brand-ink' : 'border-[var(--study-rule)] text-muted-foreground')}>
				{badge.earned ? <Award className="size-5" aria-hidden="true" /> : <Lock className="size-4 opacity-70" aria-hidden="true" />}
			</span>
			<p className="text-xs leading-tight font-semibold">{badge.title}<span className="sr-only">{badge.earned ? tr(', earned', ', conquistada') : tr(', locked', ', bloqueada')}</span></p>
			<p className="text-xs text-muted-foreground tabular-nums">{badge.earned ? tr('Earned', 'Conquistada') : `${badge.progress}/${badge.target}`}</p>
		</li>
	);
}
