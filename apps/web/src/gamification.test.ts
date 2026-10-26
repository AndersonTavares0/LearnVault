import { describe, expect, it } from 'vitest';
import { buildGamification, levelForXp } from './gamification';
import type { StudySummary, RewardSummary } from './transport';

const NOW = new Date(2026, 9, 26, 12);
const TODAY = '2026-10-26';

function study(overrides: Partial<StudySummary> = {}): StudySummary {
	return {
		currentStreak: 0,
		longestStreak: 0,
		today: TODAY,
		todayActive: false,
		atRisk: false,
		weekActiveDays: 0,
		completedLessons: 0,
		days: ['2026-10-26', '2026-10-27', '2026-10-28', '2026-10-29', '2026-10-30', '2026-10-31', '2026-11-01'].map((day) => ({ day, active: false })),
		...overrides
	};
}

function rewards(completedLessons: number, xp = completedLessons * 40): RewardSummary {
	return {
		xp,
		completedLessons,
		weeklyGoal: 3,
		missions: [
			{ id: 'lesson', progress: 0, target: 1, claimed: false },
			{ id: 'focus-two', progress: 0, target: 2, claimed: false },
			{ id: 'focus-fifty', progress: 0, target: 50, claimed: false }
		],
		study: study({ completedLessons })
	};
}

describe('levelForXp', () => {
	it('starts at level 1 and grows on the square curve', () => {
		expect(levelForXp(0)).toMatchObject({ level: 1, currentLevelXp: 0, nextLevelXp: 100 });
		expect(levelForXp(100)).toMatchObject({ level: 2, currentLevelXp: 100, nextLevelXp: 400 });
		expect(levelForXp(400)).toMatchObject({ level: 3, currentLevelXp: 400, nextLevelXp: 900 });
	});

	it('reports progress to the next level', () => {
		expect(levelForXp(250).progress).toBeCloseTo(0.5);
		expect(levelForXp(250).xpToNext).toBe(150);
	});
});

describe('buildGamification', () => {
	it('uses persisted XP and all-time completed lessons for progress achievements', () => {
		const result = buildGamification({ rewards: rewards(2, 140) });
		expect(result.completedLessons).toBe(2);
		expect(result.xp).toBe(140);
		expect(result.missions.find((mission) => mission.id === 'lesson')).toMatchObject({ progress: 0, claimed: false, rewardXp: 30 });
		expect(result.badges.find((badge) => badge.id === 'first-lesson')?.earned).toBe(true);
		expect(result.badges.map((badge) => badge.id)).toEqual(['first-lesson', 'five-lessons', 'twenty-five-lessons', 'hundred-lessons']);
	});

	it('keeps missions open and badges locked without activity', () => {
		const result = buildGamification({ rewards: rewards(0, 0) });
		expect(result.xp).toBe(0);
		expect(result.missions.every((mission) => mission.progress === 0)).toBe(true);
		expect(result.badges.every((badge) => !badge.earned)).toBe(true);
	});

	it('unlocks lesson milestones from the full summary even when recent activity is empty', () => {
		const result = buildGamification({ rewards: rewards(100, 4_000) });
		expect(result.xp).toBe(4_000);
		expect(result.badges.at(-1)).toMatchObject({ id: 'hundred-lessons', earned: true, progress: 100, target: 100 });
	});
});
