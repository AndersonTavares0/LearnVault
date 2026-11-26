import { Award, ChartNoAxesColumn, Check, Lock } from 'lucide-react';
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
import { useCatalog, useClaimMission, useFavorites, useFocusSummary, useRecent, useRewards, useSession } from '@/app/data';
import { PageHeader } from '@/app/shell';

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
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const claim = useClaimMission();
	const [range, setRange] = useState<'week' | 'all'>('week');
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
		<div>
			<PageHeader icon={ChartNoAxesColumn} title={tr('Your progress', 'Seu progresso')} description={<span className="inline-flex flex-wrap gap-x-4 gap-y-1"><span>{tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)}</span><span>{game.level.xp} XP</span><span>{tr(`${game.level.xpToNext} XP to level ${game.level.level + 1}`, `${game.level.xpToNext} XP para o nível ${game.level.level + 1}`)}</span></span>} />

			<dl aria-label={tr('At a glance', 'Visão geral')} className="grid divide-y divide-[var(--study-rule)] sm:grid-cols-3 sm:divide-x sm:divide-y-0">
				<Metric label={tr('Lessons completed (all time)', 'Aulas concluídas (total)')} value={String(lessonsDone)} caption={tr(`In total, across ${startedCourses} ${startedCourses === 1 ? 'course' : 'courses'} you started.`, `No total, em ${startedCourses} ${startedCourses === 1 ? 'curso iniciado' : 'cursos iniciados'}.`)} />
				<Metric label={tr('Active days (this week)', 'Dias ativos (esta semana)')} value={String(Math.min(study.weekActiveDays, goal))} suffix={tr(`of ${goal}-day goal`, `da meta de ${goal} dias`)} caption={`${missing === 0 ? tr('Goal reached. ', 'Meta atingida. ') : ''}${tr(`Streak: ${study.currentStreak} ${study.currentStreak === 1 ? 'day' : 'days'}. Busiest day: ${best ? best.label : 'none yet'}.`, `Sequência: ${study.currentStreak} ${study.currentStreak === 1 ? 'dia' : 'dias'}. Dia mais forte: ${best ? best.label : 'nenhum ainda'}.`)}`} />
				<Metric label={range === 'week' ? tr('Focus time (this week)', 'Tempo de foco (esta semana)') : tr('Focus time (all time)', 'Tempo de foco (total)')} value={formatMinutes(total)} caption={`${range === 'week' ? '' : `${tr('In total.', 'No total.')} `}${tr('Only the Pomodoro timer counts here.', 'Só o cronômetro Pomodoro conta aqui.')}`}>
					<div role="group" aria-label={tr('Focus time period', 'Período do tempo de foco')} className="inline-flex rounded-[var(--radius)] bg-muted p-0.5">
						{([['week', tr('This week', 'Esta semana')], ['all', tr('All time', 'Todo o tempo')]] as const).map(([value, text]) => (
							<button key={value} type="button" aria-pressed={range === value} onClick={() => setRange(value)} className={cn('min-h-11 rounded-[calc(var(--radius)-2px)] px-3 text-sm transition-colors focus-visible:ring-3 focus-visible:ring-ring/60 focus-visible:outline-none', range === value ? 'bg-card font-semibold text-foreground shadow-2xs' : 'font-medium text-muted-foreground hover:text-foreground')}>{text}</button>
						))}
					</div>
				</Metric>
			</dl>

			<div className="grid gap-x-10 lg:grid-cols-12">
				<CoursesPanel entries={library.entries} profile={profile} />

				<Panel variant="open" className="lg:col-span-4"><section aria-labelledby="missions-heading" className="space-y-3">
					<h2 id="missions-heading" className="text-[15px] font-semibold">{tr('Today', 'Hoje')}</h2>
					<ul className="divide-y divide-[var(--study-rule)]">
						{game.missions.map((mission) => {
							const done = mission.progress >= mission.target;
							return (
								<li key={mission.id} className="flex items-center gap-3 py-3">
									<span className={cn('grid size-5 shrink-0 place-items-center rounded-full border', done ? 'border-success bg-success text-background' : 'border-border-strong')}>{done && <Check className="size-3" aria-hidden="true" />}</span>
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
					<h2 id="focus-heading" className="text-[15px] font-semibold">{tr('Weekly breakdown', 'Detalhamento semanal')}</h2>
					{focus.isPending ? (
						<p role="status" className="text-sm text-muted-foreground">{tr('Loading focus statistics…', 'Carregando estatísticas de foco…')}</p>
					) : focusFailed ? (
						<p role="alert" className="text-sm text-destructive">{tr('Focus statistics could not be loaded.', 'Não foi possível carregar as estatísticas de foco.')}</p>
					) : (
						<FocusChart days={weekDays} emptyAction={noFocus ? <Link to={focusHref} className={buttonVariants({ variant: 'outline' })}>{tr('Start a focus session', 'Iniciar uma sessão de foco')}</Link> : undefined} />
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
		<div className="min-w-0 space-y-2 py-5 sm:px-6 sm:first:pl-0">
			<dt className="text-[0.8125rem] font-medium text-muted-foreground">{label}</dt>
			<dd className="space-y-1.5">
				<div className="flex flex-wrap items-end justify-between gap-x-4 gap-y-3">
					<p className="font-heading text-4xl leading-none font-semibold tabular-nums">{value}{suffix && <span className="ml-2 font-sans text-base font-medium text-muted-foreground">{suffix}</span>}</p>
					{children}
				</div>
				<p className="text-xs text-muted-foreground">{caption}</p>
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
			<div className="flex flex-wrap items-end justify-between gap-2">
				<div><h2 id="courses-heading" className="text-[15px] font-semibold">{tr('Courses', 'Cursos')}</h2><p className="text-sm text-muted-foreground">{tr(`${active.length} in progress · ${finished} finished`, `${active.length} em andamento · ${finished} concluídos`)}</p></div>
				<Link to={routerPath(href('home'))} className="inline-flex min-h-11 items-center text-sm font-medium text-foreground underline-offset-4 hover:underline">{tr('All courses', 'Todos os cursos')}</Link>
			</div>
			{active.length === 0 ? (
				<div className="space-y-4">
					<p className="text-sm text-muted-foreground">{tr('No course under way. Open a lesson and it shows up here.', 'Nenhum curso em andamento. Abra uma aula e ele aparece aqui.')}</p>
					{firstCourse && (
						<Link to={routerPath(href('course', firstCourse.courseId))} className={buttonVariants({ variant: 'outline' })}>
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
 * Focus minutes for each day of the week as a smooth area, with the busiest day labelled. The
 * numbers are also in a table for screen readers, so nothing depends on seeing the curve.
 */
function FocusChart({ days, emptyAction }: { days: ChartDay[]; emptyAction?: ReactNode }) {
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
		<div className="flex flex-1 flex-col">
			{!empty ? (
				<svg viewBox={`0 0 ${width} ${height + 24}`} role="img" aria-label={tr('Chart of focus minutes for each day this week; the same numbers are in the table below.', 'Gráfico dos minutos de foco em cada dia desta semana; os mesmos números estão na tabela abaixo.')} className="mt-2 w-full">
				<path d={area} className="fill-success-tint" />
				<path d={line} fill="none" strokeWidth="2.5" strokeLinecap="round" className="stroke-success" />
				{peak >= 0 && (
					<g>
						<circle cx={points[peak].x} cy={points[peak].y} r="4.5" className="fill-card stroke-success" strokeWidth="2.5" />
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

/** A badge: filled when earned, a dashed outline with its count while locked. */
function BadgeTile({ badge }: { badge: Badge }) {
	return (
		<li className="flex flex-col items-center gap-1.5 px-1.5 py-2 text-center">
			<span className={cn('grid size-11 place-items-center rounded-full border', badge.earned ? 'border-success/50 bg-success-tint text-success' : 'border-[var(--study-rule)] text-muted-foreground')}>
				{badge.earned ? <Award className="size-5" aria-hidden="true" /> : <Lock className="size-4 opacity-70" aria-hidden="true" />}
			</span>
			<p className="text-xs leading-tight font-semibold">{badge.title}<span className="sr-only">{badge.earned ? tr(', earned', ', conquistada') : tr(', locked', ', bloqueada')}</span></p>
			<p className="text-xs text-muted-foreground tabular-nums">{badge.earned ? tr('Earned', 'Conquistada') : `${badge.progress}/${badge.target}`}</p>
		</li>
	);
}
