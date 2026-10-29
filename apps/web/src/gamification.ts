import type { RewardSummary } from './transport';

export type Badge = {
	id: string;
	title: string;
	description: string;
	earned: boolean;
	progress: number;
	target: number;
};

export type MissionId = 'lesson' | 'focus-two' | 'focus-fifty';

export type Mission = {
	id: MissionId;
	title: string;
	description: string;
	progress: number;
	target: number;
	claimed: boolean;
	rewardXp: number;
};

export type Level = {
	level: number;
	xp: number;
	currentLevelXp: number;
	nextLevelXp: number;
	progress: number;
	xpToNext: number;
};

export type Gamification = {
	xp: number;
	level: Level;
	badges: Badge[];
	missions: Mission[];
	completedLessons: number;
};

export function levelForXp(xp: number): Level {
	const safe = Math.max(0, Math.floor(xp));
	const level = Math.floor(Math.sqrt(safe / 100)) + 1;
	const currentLevelXp = (level - 1) ** 2 * 100;
	const nextLevelXp = level ** 2 * 100;
	const span = Math.max(1, nextLevelXp - currentLevelXp);
	return {
		level,
		xp: safe,
		currentLevelXp,
		nextLevelXp,
		progress: Math.min(1, Math.max(0, (safe - currentLevelXp) / span)),
		xpToNext: Math.max(0, nextLevelXp - safe)
	};
}

function badge(id: string, title: string, description: string, progress: number, target: number): Badge {
	return { id, title, description, earned: progress >= target, progress: Math.min(progress, target), target };
}

export function buildGamification(input: {
	rewards: RewardSummary;
}): Gamification {
	const descriptions: Record<MissionId, { title: string; description: string }> = {
		lesson: { title: 'Conclua uma aula', description: 'Feche uma aula hoje, mesmo que seja curta.' },
		'focus-two': { title: 'Dois blocos de foco', description: 'Conclua dois Pomodoros de foco hoje.' },
		'focus-fifty': { title: 'Cinquenta minutos', description: 'Some 50 minutos de foco no dia.' }
	};
	const missions = input.rewards.missions.map((mission) => ({
		...descriptions[mission.id],
		id: mission.id,
		progress: mission.progress,
		target: mission.target,
		claimed: mission.claimed,
		rewardXp: 30
	}));
	const completedLessons = input.rewards.completedLessons;

	const badges: Badge[] = [
		badge('first-lesson', 'Primeira aula', 'Uma aula concluída já conta.', completedLessons, 1),
		badge('five-lessons', 'Cinco aulas', 'Cinco aulas concluídas.', completedLessons, 5),
		badge('twenty-five-lessons', 'Vinte e cinco aulas', 'Um curso inteiro cabe aqui.', completedLessons, 25),
		badge('hundred-lessons', 'Cem aulas', 'Cem aulas concluídas.', completedLessons, 100)
	];
	badges.sort((a, b) => Number(b.earned) - Number(a.earned) || a.target - b.target);

	return {
		xp: input.rewards.xp,
		level: levelForXp(input.rewards.xp),
		badges,
		missions,
		completedLessons
	};
}
