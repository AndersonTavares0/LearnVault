import { Award, Check, Flame, Target, Timer } from 'lucide-react';
import { useId, useMemo, type ReactNode } from 'react';
import { Link } from 'react-router';
import { Button } from '@/components/ui/button';
import { Label } from '@/components/ui/label';
import { useLocale } from '@/hooks/use-locale';
import { formatCount } from '@/lib/catalog';
import { buildGamification } from '@/lib/gamification';
import { getLocale, tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { studySyncSupported } from '@/lib/player';
import { MAX_WEEKLY_GOAL, MIN_WEEKLY_GOAL } from '@/lib/statistics';
import type { StudyDay } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useCatalog, useClaimMission, useFavorites, useFocusSummary, useRecent, useRewards, useSetWeeklyGoal } from '@/app/data';
import { PageHeader } from '@/app/shell';

const weekday = (day: string) => new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { weekday: 'short' });

/**
 * Progress (docs/design/redesign.md): habit first — streak and weekly goal — then the
 * course list, with XP, missions and achievements kept quiet below.
 */
export function ProgressPage() {
	useLocale();
	const rewards = useRewards();
	const focus = useFocusSummary();
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const claim = useClaimMission();
	const setGoal = useSetWeeklyGoal();
	const library = useMemo(() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: new Set(favorites.data?.courseIds ?? []) }), [catalog.data, recent.data, favorites.data]);
	const game = rewards.data ? buildGamification({ rewards: rewards.data, completedCourses: library.entries.filter((entry) => entry.status === 'completed').length }) : null;

	if (!studySyncSupported) return <PageHeader title={tr('Progress', 'Progresso')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	if (rewards.isPending) return <><PageHeader title={tr('Progress', 'Progresso')} /><p role="status" className="text-muted-foreground">{tr('Loading your progress…', 'Carregando seu progresso…')}</p></>;
	// A failed refetch keeps showing the last good data.
	if (!rewards.data || !game) return <><PageHeader title={tr('Progress', 'Progresso')} /><p role="alert" className="text-destructive">{rewards.error?.message ?? tr('Could not load your progress.', 'Não foi possível carregar seu progresso.')}</p></>;

	const study = rewards.data.study;
	const goal = rewards.data.weeklyGoal;
	const started = library.entries.filter((entry) => entry.status !== 'not_started').sort((a, b) => b.done / Math.max(1, b.total) - a.done / Math.max(1, a.total));

	return (
		<div className="space-y-10">
			<PageHeader title={tr('Progress', 'Progresso')} description={formatCount(game.completedLessons, tr('lesson completed', 'aula concluída'), tr('lessons completed', 'aulas concluídas'))} />

			<section aria-labelledby="habit-heading" className="grid gap-4 md:grid-cols-2">
				<h2 id="habit-heading" className="sr-only">{tr('Study habit', 'Hábito de estudo')}</h2>
				<Panel icon={<Flame />} title={tr('Streak', 'Sequência')}>
					<p className="text-3xl font-semibold tabular-nums">{formatCount(study.currentStreak, tr('day', 'dia'), tr('days', 'dias'))}</p>
					<p className="text-sm text-muted-foreground">
						{study.atRisk
							? tr('Study today to keep your streak.', 'Estude hoje para manter a sequência.')
							: study.todayActive ? tr('Today counts already.', 'Hoje já conta.') : tr('Any lesson today starts a new streak.', 'Qualquer aula hoje começa uma nova sequência.')}
						{' '}{tr(`Best: ${study.longestStreak}.`, `Melhor: ${study.longestStreak}.`)}
					</p>
				</Panel>
				<Panel icon={<Target />} title={tr('Weekly goal', 'Meta semanal')}>
					<p className="text-3xl font-semibold tabular-nums">{tr(`${study.weekActiveDays} of ${goal} days`, `${study.weekActiveDays} de ${goal} dias`)}</p>
					<WeekStrip days={study.days} />
					<div className="flex items-center gap-2">
						<Label htmlFor="weekly-goal">{tr('Days per week', 'Dias por semana')}</Label>
						<select id="weekly-goal" value={goal} disabled={setGoal.isPending} onChange={(event) => setGoal.mutate(Number(event.target.value))} className="h-11 rounded-lg border border-input bg-background px-2 text-sm">
							{Array.from({ length: MAX_WEEKLY_GOAL - MIN_WEEKLY_GOAL + 1 }, (_, index) => index + MIN_WEEKLY_GOAL).map((value) => <option key={value} value={value}>{value}</option>)}
						</select>
					</div>
				</Panel>
			</section>

			<section aria-labelledby="courses-progress-heading" className="space-y-4">
				<h2 id="courses-progress-heading" className="text-lg font-semibold">{tr('Your courses', 'Seus cursos')}</h2>
				{started.length === 0 ? (
					<p className="text-muted-foreground">{tr('Courses you start appear here with their progress.', 'Os cursos que você começar aparecem aqui com o progresso.')}</p>
				) : (
					<ul className="divide-y rounded-lg border bg-card">
						{started.map((entry) => (
							<li key={entry.courseId} className="flex flex-wrap items-center gap-4 p-4">
								<div className="min-w-0 flex-1 space-y-2">
									<Link to={`/course/${encodeURIComponent(entry.courseId)}`} className="font-medium hover:underline">{entry.course.title}</Link>
									<Bar value={entry.done / Math.max(1, entry.total)} label={tr(`${entry.done} of ${entry.total} lessons`, `${entry.done} de ${entry.total} aulas`)} />
								</div>
								{entry.status === 'completed' && <p className="flex items-center gap-1 text-sm font-medium text-primary"><Check className="size-4" aria-hidden="true" />{tr('Finished', 'Concluído')}</p>}
							</li>
						))}
					</ul>
				)}
			</section>

			{focus.data && (
				<section aria-labelledby="focus-heading" className="space-y-2">
					<h2 id="focus-heading" className="flex items-center gap-2 text-lg font-semibold"><Timer className="size-5" aria-hidden="true" />{tr('Focus', 'Foco')}</h2>
					<p className="text-muted-foreground">
						{tr(`${focus.data.todayMinutes} min today · ${focus.data.weekMinutes} min in ${focus.data.weekSessions} sessions this week`, `${focus.data.todayMinutes} min hoje · ${focus.data.weekMinutes} min em ${focus.data.weekSessions} sessões nesta semana`)}
					</p>
				</section>
			)}

			<section aria-labelledby="missions-heading" className="space-y-4">
				<h2 id="missions-heading" className="text-lg font-semibold">{tr('Today’s missions', 'Missões de hoje')}</h2>
				<ul className="grid gap-3 md:grid-cols-3">
					{game.missions.map((mission) => {
						const done = mission.progress >= mission.target;
						return (
							<li key={mission.id} className="space-y-2 rounded-lg border bg-card p-4">
								<p className="font-medium">{mission.title}</p>
								<p className="text-sm text-muted-foreground">{mission.description}</p>
								<Bar value={mission.progress / mission.target} label={`${Math.min(mission.progress, mission.target)} / ${mission.target}`} />
								{mission.claimed ? (
									<p className="flex min-h-11 items-center gap-1 text-sm text-muted-foreground"><Check className="size-4" aria-hidden="true" />{tr(`${mission.rewardXp} XP claimed`, `${mission.rewardXp} XP resgatados`)}</p>
								) : (
									<Button size="sm" variant={done ? 'default' : 'outline'} disabled={!done || claim.isPending} onClick={() => claim.mutate(mission.id)}>{tr(`Claim ${mission.rewardXp} XP`, `Resgatar ${mission.rewardXp} XP`)}</Button>
								)}
							</li>
						);
					})}
				</ul>
			</section>

			<section aria-labelledby="level-heading" className="grid gap-4 md:grid-cols-2">
				<Panel icon={<Award />} title={tr(`Level ${game.level.level} · ${game.level.title}`, `Nível ${game.level.level} · ${game.level.title}`)} headingId="level-heading">
					<Bar value={game.level.progress} label={tr(`${game.level.xp} XP · ${game.level.xpToNext} XP to the next level`, `${game.level.xp} XP · faltam ${game.level.xpToNext} XP para o próximo nível`)} />
				</Panel>
				<Panel icon={<Award />} title={tr('Achievements', 'Conquistas')}>
					<ul className="space-y-2">
						{game.badges.map((badge) => (
							<li key={badge.id} className={cn('flex items-center gap-3', !badge.earned && 'text-muted-foreground')}>
								{badge.earned ? <Check className="size-4 shrink-0 text-primary" aria-hidden="true" /> : <span className="size-4 shrink-0" aria-hidden="true" />}
								<span className="min-w-0 flex-1">{badge.title}<span className="sr-only">{badge.earned ? tr(', earned', ', conquistada') : ''}</span></span>
								<span className="text-sm tabular-nums">{badge.progress}/{badge.target}</span>
							</li>
						))}
					</ul>
				</Panel>
			</section>
		</div>
	);
}

function Panel({ icon, title, children, headingId }: { icon: ReactNode; title: string; children: ReactNode; headingId?: string }) {
	return (
		<div className="space-y-3 rounded-xl border bg-card p-5">
			<h3 id={headingId} className="flex items-center gap-2 font-semibold [&_svg]:size-5 [&_svg]:text-primary"><span aria-hidden="true">{icon}</span>{title}</h3>
			{children}
		</div>
	);
}

function Bar({ value, label }: { value: number; label: string }) {
	const id = useId();
	const percent = Math.round(Math.min(1, Math.max(0, value)) * 100);
	return (
		<div className="space-y-1">
			<div role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-labelledby={id} className="h-2 overflow-hidden rounded-full bg-muted">
				<div className="h-full rounded-full bg-primary" style={{ width: `${percent}%` }} />
			</div>
			<p id={id} className="text-xs text-muted-foreground">{label}</p>
		</div>
	);
}

/** This week, Monday first; active days are filled and labelled, not only colored (WCAG 1.4.1). */
function WeekStrip({ days }: { days: StudyDay[] }) {
	return (
		<ol className="flex gap-2" aria-label={tr('This week', 'Esta semana')}>
			{days.map((day) => (
				<li key={day.day} className="flex flex-col items-center gap-1 text-xs text-muted-foreground">
					<span className={cn('grid size-8 place-items-center rounded-full border', day.active && 'border-primary bg-primary text-primary-foreground')}>
						{day.active ? <Check className="size-4" aria-hidden="true" /> : null}
					</span>
					<span>{weekday(day.day)}<span className="sr-only">{day.active ? tr(': studied', ': estudou') : tr(': no study', ': sem estudo')}</span></span>
				</li>
			))}
		</ol>
	);
}
