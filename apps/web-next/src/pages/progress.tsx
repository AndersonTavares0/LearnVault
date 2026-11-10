import { Award, Check, Lock } from 'lucide-react';
import { useMemo } from 'react';
import { Link } from 'react-router';
import { CourseCover } from '@/components/course-cover';
import { ProgressBar, progressLabel, routerPath } from '@/components/course-card';
import { Button } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { formatCount } from '@/lib/catalog';
import { buildGamification, type Badge } from '@/lib/gamification';
import { getLocale, tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { studySyncSupported } from '@/lib/player';
import { href } from '@/lib/router';
import type { FocusDaySummary, StudyDay } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useCatalog, useClaimMission, useFavorites, useFocusSummary, useRecent, useRewards, useSession } from '@/app/data';
import { PageHeader } from '@/app/shell';

const weekday = (day: string) => new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { weekday: 'short' });
const dayOfMonth = (day: string) => new Date(`${day}T12:00:00`).getDate();

/**
 * Progress: this week as a sentence and seven days, then your courses beside today's missions,
 * then the badges. Habit first and quiet; XP and level stay in one line under the missions.
 */
export function ProgressPage() {
	useLocale();
	const session = useSession();
	const rewards = useRewards();
	const focus = useFocusSummary();
	const catalog = useCatalog();
	const recent = useRecent();
	const favorites = useFavorites();
	const claim = useClaimMission();
	const library = useMemo(() => buildLibrary({ catalog: catalog.data, recent: recent.data ?? [], favoriteCourseIds: new Set(favorites.data?.courseIds ?? []) }), [catalog.data, recent.data, favorites.data]);
	const game = rewards.data ? buildGamification({ rewards: rewards.data, completedCourses: library.entries.filter((entry) => entry.status === 'completed').length }) : null;

	if (!studySyncSupported) return <PageHeader title={tr('Progress', 'Progresso')} description={tr('Study tracking is off on this server.', 'O acompanhamento de estudo está desligado neste servidor.')} />;
	if (rewards.isPending) return <><PageHeader title={tr('Progress', 'Progresso')} /><p role="status" className="text-muted-foreground">{tr('Loading your progress…', 'Carregando seu progresso…')}</p></>;
	// A failed refetch keeps showing the last good data.
	if (!rewards.data || !game) return <><PageHeader title={tr('Progress', 'Progresso')} /><p role="alert" className="text-destructive">{rewards.error?.message ?? tr('Could not load your progress.', 'Não foi possível carregar seu progresso.')}</p></>;

	const study = rewards.data.study;
	const goal = rewards.data.weeklyGoal;
	const profile = session.data?.user?.userId ?? '';
	const started = library.entries.filter((entry) => entry.status !== 'not_started').sort((a, b) => b.done / Math.max(1, b.total) - a.done / Math.max(1, a.total));
	const missing = Math.max(0, goal - study.weekActiveDays);

	return (
		<div className="space-y-10">
			<PageHeader title={tr('Progress', 'Progresso')} description={formatCount(game.completedLessons, tr('lesson completed', 'aula concluída'), tr('lessons completed', 'aulas concluídas'))} />

			<section aria-labelledby="week-heading" className="rounded-2xl border bg-card p-5 sm:p-7">
				<div className="flex flex-wrap items-end justify-between gap-6">
					<div className="min-w-0 space-y-1">
						<h2 id="week-heading" className="font-serif text-2xl font-bold text-balance sm:text-3xl">{formatCount(study.weekActiveDays, tr('day studied this week', 'dia estudado nesta semana'), tr('days studied this week', 'dias estudados nesta semana'))}</h2>
						<p className="text-muted-foreground">
							{missing === 0
								? tr(`You reached your goal of ${goal} days.`, `Você atingiu sua meta de ${goal} dias.`)
								: tr(`${formatCount(missing, 'more day', 'more days')} reaches your goal of ${goal}.`, `Mais ${formatCount(missing, 'dia', 'dias')} atinge sua meta de ${goal}.`)}
							{' '}{study.currentStreak > 0 ? tr(`Current streak: ${study.currentStreak}.`, `Sequência atual: ${study.currentStreak}.`) : ''}
							{' '}{tr(`Best: ${study.longestStreak}.`, `Melhor: ${study.longestStreak}.`)}
						</p>
					</div>
					<div className="w-full max-w-xs space-y-1.5">
						<div className="flex justify-between text-sm text-muted-foreground"><span>{tr(`Weekly goal: ${goal} days`, `Meta semanal: ${goal} dias`)}</span><span className="tabular-nums">{Math.min(study.weekActiveDays, goal)} / {goal}</span></div>
						<div role="progressbar" aria-valuemin={0} aria-valuemax={goal} aria-valuenow={Math.min(study.weekActiveDays, goal)} aria-label={tr('Weekly goal', 'Meta semanal')} className="flex gap-1">
							{Array.from({ length: goal }, (_, index) => <span key={index} className={cn('h-2 flex-1 rounded-full', index < study.weekActiveDays ? 'bg-primary' : 'bg-muted')} />)}
						</div>
					</div>
				</div>
				<Week days={study.days} today={study.today} focusDays={focus.data?.days ?? []} />
			</section>

			<div className="grid gap-10 lg:grid-cols-[1fr_22rem] lg:gap-14">
				<section aria-labelledby="courses-progress-heading" className="min-w-0">
					<h2 id="courses-progress-heading" className="mb-1 text-xl font-semibold">{tr('Your courses', 'Seus cursos')}</h2>
					{started.length === 0 ? (
						<p className="rounded-xl border border-dashed p-6 text-muted-foreground">{tr('Courses you start appear here with their progress.', 'Os cursos que você começar aparecem aqui com o progresso.')}</p>
					) : (
						<ul>
							{started.map((entry) => (
								<li key={entry.courseId} className="relative flex items-center gap-4 border-t py-3 hover:bg-accent/40">
									<CourseCover course={entry.course} profile={profile} done={entry.done} total={entry.total} compact className="w-24 shrink-0" />
									<div className="min-w-0 flex-1">
										<h3 className="truncate font-semibold"><Link to={routerPath(href('course', entry.courseId))} className="after:absolute after:inset-0 after:content-['']">{entry.course.title}</Link></h3>
										<p className="truncate text-sm text-muted-foreground">{progressLabel(entry)}</p>
									</div>
									<span className="flex shrink-0 items-center gap-1 text-sm font-semibold text-primary">{entry.status === 'completed' ? <><Check className="size-4" aria-hidden="true" />{tr('Finished', 'Concluído')}</> : tr('Resume', 'Retomar')}</span>
								</li>
							))}
						</ul>
					)}
				</section>

				<section aria-labelledby="missions-heading" className="min-w-0">
					<h2 id="missions-heading" className="mb-1 text-xl font-semibold">{tr('Today', 'Hoje')}</h2>
					<ul>
						{game.missions.map((mission) => {
							const done = mission.progress >= mission.target;
							return (
								<li key={mission.id} className="flex flex-wrap items-start gap-3 border-t py-3">
									<span className={cn('mt-0.5 grid size-6 shrink-0 place-items-center rounded-full border-2', done ? 'border-primary bg-primary text-primary-foreground' : 'border-muted-foreground/50')}>{done && <Check className="size-4" aria-hidden="true" />}</span>
									<div className="min-w-0 flex-1">
										<p className="font-medium">{mission.title}</p>
										<p className="text-sm text-muted-foreground">{mission.description}</p>
										<ProgressBar value={mission.progress / mission.target} label={`${Math.min(mission.progress, mission.target)} / ${mission.target}`} className="mt-2" />
									</div>
									{mission.claimed ? (
										<p className="flex min-h-11 items-center gap-1 text-sm text-muted-foreground"><Check className="size-4" aria-hidden="true" />{tr(`${mission.rewardXp} XP claimed`, `${mission.rewardXp} XP resgatados`)}</p>
									) : done ? (
										<Button size="sm" variant="secondary" disabled={claim.isPending} onClick={() => claim.mutate(mission.id)}>{tr(`Claim ${mission.rewardXp} XP`, `Resgatar ${mission.rewardXp} XP`)}</Button>
									) : null}
								</li>
							);
						})}
					</ul>
					<p className="border-t pt-3 text-sm text-muted-foreground">{tr(`Level ${game.level.level}, ${game.level.title}: ${game.level.xp} XP, ${game.level.xpToNext} to the next level.`, `Nível ${game.level.level}, ${game.level.title}: ${game.level.xp} XP, faltam ${game.level.xpToNext} para o próximo nível.`)}</p>
				</section>
			</div>

			<section aria-labelledby="badges-heading">
				<h2 id="badges-heading" className="mb-1 text-xl font-semibold">{tr('Badges', 'Emblemas')}</h2>
				<ul className="grid grid-cols-2 border-t sm:grid-cols-3 lg:grid-cols-6">
					{game.badges.map((badge) => <BadgeTile key={badge.id} badge={badge} />)}
				</ul>
			</section>
		</div>
	);
}

/** Seven days of this week. A studied day is filled; focus minutes show under it. */
function Week({ days, today, focusDays }: { days: StudyDay[]; today: string; focusDays: FocusDaySummary[] }) {
	const minutes = new Map(focusDays.map((day) => [day.day, day.minutes]));
	return (
		<ol aria-label={tr('This week', 'Esta semana')} className="mt-6 grid grid-cols-7 gap-1 sm:gap-4">
			{days.map((day) => {
				const focusMinutes = minutes.get(day.day) ?? 0;
				const isToday = day.day === today;
				return (
					<li key={day.day} className={cn('min-w-0 space-y-2 rounded-lg p-1 sm:p-2', isToday && 'ring-2 ring-primary/40')}>
						<p className={cn('flex flex-col items-start text-sm sm:flex-row sm:justify-between', isToday ? 'font-semibold' : 'text-muted-foreground')}>
							<span>{weekday(day.day)}</span><span className="tabular-nums">{dayOfMonth(day.day)}</span>
						</p>
						{/* Phones show a mark only; the words are still read out. From `sm` up there is room for them. */}
						<div className={cn('flex min-h-14 flex-col items-center justify-center gap-0.5 rounded-md px-1 py-1.5 text-xs sm:items-start sm:px-2 sm:text-sm', day.active ? 'bg-primary/15' : 'bg-muted/60')}>
							{day.active && <Check className="size-4 text-primary sm:hidden" aria-hidden="true" />}
							<span className={cn(day.active ? 'font-medium sr-only sm:not-sr-only' : 'sr-only')}>{day.active ? tr('Studied', 'Estudou') : tr('No study', 'Sem estudo')}</span>
							{focusMinutes > 0 && <span className="text-muted-foreground tabular-nums"><span className="sm:hidden">{focusMinutes}m</span><span className="sr-only sm:not-sr-only">{tr(`${focusMinutes} min focus`, `${focusMinutes} min de foco`)}</span></span>}
						</div>
					</li>
				);
			})}
		</ol>
	);
}

/** A badge: filled when earned, a dashed outline with its count while locked. */
function BadgeTile({ badge }: { badge: Badge }) {
	return (
		<li className="flex flex-col items-center gap-1.5 px-2 py-4 text-center">
			<span className={cn('grid size-14 place-items-center rounded-full', badge.earned ? 'bg-primary text-primary-foreground' : 'border-2 border-dashed border-muted-foreground/50 text-muted-foreground')}>
				{badge.earned ? <Award className="size-6" aria-hidden="true" /> : <Lock className="size-5" aria-hidden="true" />}
			</span>
			<p className="text-sm leading-tight font-semibold">{badge.title}<span className="sr-only">{badge.earned ? tr(', earned', ', conquistada') : tr(', locked', ', bloqueada')}</span></p>
			<p className="text-sm text-muted-foreground tabular-nums">{badge.earned ? tr('Earned', 'Conquistada') : `${badge.progress}/${badge.target}`}</p>
		</li>
	);
}
