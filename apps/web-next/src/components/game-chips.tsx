import { Flame, Star } from 'lucide-react';
import { Link } from 'react-router';
import { useLocale } from '@/hooks/use-locale';
import { useStore } from '@/hooks/use-store';
import { appearanceStore } from '@/lib/appearance';
import { buildGamification } from '@/lib/gamification';
import { tr } from '@/lib/i18n';
import { studySyncSupported } from '@/lib/player';
import { useRewards } from '@/app/data';

/** Streak and level in the top bar, so the habit is in sight on every page. Each can be hidden in Settings. */
export function GameChips() {
	useLocale();
	const look = useStore(appearanceStore);
	const rewards = useRewards();
	if (!studySyncSupported || !rewards.data || (!look.showStreak && !look.showXp)) return null;
	const streak = rewards.data.study.currentStreak;
	const level = buildGamification({ rewards: rewards.data, completedCourses: 0 }).level;
	return (
		<Link to="/progress" className="hidden min-h-11 items-center gap-3 rounded-lg px-2 text-sm font-semibold hover:bg-muted lg:flex">
			{look.showStreak && <span className="flex items-center gap-1 text-reward"><Flame className="size-5" aria-hidden="true" />{streak}<span className="sr-only">{tr(streak === 1 ? ' day streak' : ' day streak', ' dias seguidos')}</span></span>}
			{look.showXp && <span className="flex items-center gap-1.5"><span className="grid size-6 place-items-center rounded-full bg-reward-fill text-xs text-reward-fill-foreground"><Star className="size-3.5" aria-hidden="true" /></span>{tr(`Lv ${level.level}`, `Nv ${level.level}`)}<span className="sr-only">{tr(`, ${level.xp} XP`, `, ${level.xp} XP`)}</span></span>}
		</Link>
	);
}
