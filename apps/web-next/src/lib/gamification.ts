import { tr } from './i18n';
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
	title: string;
	xp: number;
	currentLevelXp: number;
	nextLevelXp: number;
	progress: number;
	xpToNext: number;
};

const LEVEL_TITLES: Array<[string, string]> = [
	['Curious Apprentice', 'Aprendiz Curioso'],
	['Archive Explorer', 'Explorador do Acervo'],
	['Dedicated Student', 'Estudante Dedicado'],
	['Consistent Practitioner', 'Praticante Consistente'],
	['Knowledge Artisan', 'Artesão do Conhecimento'],
	['Master of Discipline', 'Mestre da Disciplina'],
	['Library Sage', 'Sábio da Biblioteca'],
	['Guardian of Knowledge', 'Guardião do Conhecimento']
];

export function levelTitle(level: number): string {
	const index = Math.max(0, level - 1);
	const [en, pt] = LEVEL_TITLES[Math.min(index, LEVEL_TITLES.length - 1)];
	return tr(en, pt);
}

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
		title: levelTitle(level),
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
		lesson: {
			title: tr('Complete a lesson', 'Conclua uma aula'),
			description: tr('Finish one lesson today, even a short one.', 'Feche uma aula hoje, mesmo que seja curta.')
		},
		'focus-two': {
			title: tr('Two focus blocks', 'Dois blocos de foco'),
			description: tr('Complete two focus Pomodoros today.', 'Conclua dois Pomodoros de foco hoje.')
		},
		'focus-fifty': {
			title: tr('Fifty minutes', 'Cinquenta minutos'),
			description: tr('Accumulate 50 focus minutes today.', 'Some 50 minutos de foco no dia.')
		}
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
		badge('first-lesson', tr('First lesson', 'Primeira aula'), tr('One completed lesson already counts.', 'Uma aula concluída já conta.'), completedLessons, 1),
		badge('five-lessons', tr('Five lessons', 'Cinco aulas'), tr('Five lessons completed.', 'Cinco aulas concluídas.'), completedLessons, 5),
		badge('twenty-five-lessons', tr('Twenty-five lessons', 'Vinte e cinco aulas'), tr('A whole course fits in here.', 'Um curso inteiro cabe aqui.'), completedLessons, 25),
		badge('hundred-lessons', tr('One hundred lessons', 'Cem aulas'), tr('One hundred lessons completed.', 'Cem aulas concluídas.'), completedLessons, 100)
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
