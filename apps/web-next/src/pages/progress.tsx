import { Award, Check, Lock } from 'lucide-react';
import { useMemo } from 'react';
import { Band } from '@/components/band';
import { ProgressBar } from '@/components/course-card';
import { Button } from '@/components/ui/button';
import { useLocale } from '@/hooks/use-locale';
import { formatCount } from '@/lib/catalog';
import { buildGamification, type Badge } from '@/lib/gamification';
import { getLocale, tr } from '@/lib/i18n';
import { buildLibrary } from '@/lib/library';
import { studySyncSupported } from '@/lib/player';
import type { FocusDaySummary, StudyDay } from '@/lib/transport';
import { cn } from '@/lib/utils';
import { useCatalog, useClaimMission, useFavorites, useFocusSummary, useRecent, useRewards } from '@/app/data';
import { PageHeader } from '@/app/shell';

const weekday = (day: string) => new Date(`${day}T12:00:00`).toLocaleDateString(getLocale(), { weekday: 'short' });

/**
 * Progress: this week as a sentence and seven days, then your courses beside today's missions,
 * then the badges. Habit first and quiet; XP and level stay in one line under the missions.
 */
export function ProgressPage() {
	useLocale();
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
	const missing = Math.max(0, goal - study.weekActiveDays);

	return (
		<div>
			<Band>
				<div className="space-y-2 pb-8">
					<h1 className="text-3xl font-bold sm:text-4xl">{tr('Progress', 'Progresso')}</h1>
					<p className="text-ink-muted">
						{tr(`Level ${game.level.level}`, `Nível ${game.level.level}`)} · {game.level.xp} XP · {formatCount(study.currentStreak, tr('day streak', 'dia seguido'), tr('day streak', 'dias seguidos'))} · {formatCount(game.completedLessons, tr('lesson completed', 'aula concluída'), tr('lessons completed', 'aulas concluídas'))}
					</p>
				</div>
			</Band>

			<div className="grid gap-12 lg:grid-cols-[minmax(0,1fr)_24rem]">
				<div className="min-w-0 space-y-12">
					<section aria-labelledby="week-heading" className="space-y-5">
						<div className="flex flex-wrap items-end justify-between gap-3">
							<div>
								<h2 id="week-heading" className="text-2xl font-bold">{tr(`${Math.min(study.weekActiveDays, goal)} of ${goal} days this week`, `${Math.min(study.weekActiveDays, goal)} de ${goal} dias nesta semana`)}</h2>
								<p className="text-muted-foreground">{missing === 0 ? tr('You reached your weekly goal.', 'Você atingiu a meta semanal.') : tr(`${formatCount(missing, 'more day', 'more days')} to reach your goal.`, `Mais ${formatCount(missing, 'dia', 'dias')} para a meta.`)}</p>
							</div>
							<div role="progressbar" aria-valuemin={0} aria-valuemax={goal} aria-valuenow={Math.min(study.weekActiveDays, goal)} aria-label={tr('Weekly goal', 'Meta semanal')} className="flex w-48 gap-1">
								{Array.from({ length: goal }, (_, index) => <span key={index} className={cn('h-2 flex-1 rounded-full', index < study.weekActiveDays ? 'bg-primary' : 'bg-muted')} />)}
							</div>
						</div>
						<Week days={study.days} today={study.today} focusDays={focus.data?.days ?? []} />
					</section>

					<section aria-labelledby="badges-heading" className="space-y-3">
						<h2 id="badges-heading" className="text-2xl font-bold">{tr('Badges', 'Emblemas')}</h2>
						<ul className="grid grid-cols-2 gap-x-4 sm:grid-cols-3 lg:grid-cols-4">
							{game.badges.map((badge) => <BadgeTile key={badge.id} badge={badge} />)}
						</ul>
					</section>
				</div>

				<section aria-labelledby="missions-heading" className="min-w-0 space-y-3">
					<h2 id="missions-heading" className="text-2xl font-bold">{tr('Today', 'Hoje')}</h2>
					<ul className="divide-y border-y">
						{game.missions.map((mission) => {
							const done = mission.progress >= mission.target;
							return (
								<li key={mission.id} className="flex flex-wrap items-start gap-3 py-4">
									<span className={cn('mt-0.5 grid size-6 shrink-0 place-items-center rounded-full border-2', done ? 'border-primary bg-primary text-primary-foreground' : 'border-muted-foreground/50')}>{done && <Check className="size-4" aria-hidden="true" />}</span>
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
				</section>
			</div>
		</div>
	);
}

/** Seven days of this week: a filled mark for a studied day, focus minutes under it. */
function Week({ days, today, focusDays }: { days: StudyDay[]; today: string; focusDays: FocusDaySummary[] }) {
	const minutes = new Map(focusDays.map((day) => [day.day, day.minutes]));
	return (
		<ol aria-label={tr('This week', 'Esta semana')} className="grid grid-cols-7 gap-2">
			{days.map((day) => {
				const focusMinutes = minutes.get(day.day) ?? 0;
				const isToday = day.day === today;
				return (
					<li key={day.day} className="flex min-w-0 flex-col items-center gap-2 text-sm">
						<span className={cn(isToday ? 'font-bold' : 'text-muted-foreground')}>{weekday(day.day)}</span>
						<span className={cn('grid size-10 place-items-center rounded-full border-2', day.active ? 'border-primary bg-primary text-primary-foreground' : 'border-border', isToday && !day.active && 'border-primary')}>
							{day.active && <Check className="size-5" aria-hidden="true" />}
							<span className="sr-only">{day.active ? tr('Studied', 'Estudou') : tr('No study', 'Sem estudo')}</span>
						</span>
						<span className="h-4 text-xs text-muted-foreground tabular-nums">{focusMinutes > 0 ? tr(`${focusMinutes} min`, `${focusMinutes} min`) : ''}</span>
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
