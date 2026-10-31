import { describe, expect, it } from 'vitest';
import { buildStatistics, DEFAULT_WEEKLY_GOAL, parseWeeklyGoal, persistWeeklyGoal, readWeeklyGoal, weeklyGoalProgress } from './statistics';
import type { Catalog, CatalogNode, StudyProgress } from './transport';

function media(name: string, id: string) {
	return { name, path: name, size: 10, mediaId: id, kind: 'media', availability: 'available' };
}

function lesson(title: string, id: string): CatalogNode {
	const file = media(`${id}.mp4`, id);
	return { title, path: `${id}.mp4`, role: 'lesson', files: [file], looseFiles: [], children: [] };
}

function course(title: string, id: string, lessons: CatalogNode[]): CatalogNode {
	return {
		title,
		path: id,
		role: 'course',
		courseId: id,
		files: [],
		looseFiles: [],
		children: [{ title: 'Módulo', path: `${id}/mod`, role: 'module', files: [], looseFiles: [], children: lessons }]
	};
}

const catalog: Catalog = {
	complete: true,
	looseFiles: [],
	nodes: [
		course('Curso curto', 'short', [lesson('Aula 1', 'm1')]),
		course('Curso longo', 'long', [lesson('Aula 2', 'm2'), lesson('Aula 3', 'm3')])
	]
};

function progress(mediaId: string, completed: boolean, positionSeconds: number, updatedAt: number | null): StudyProgress {
	return { mediaId, completed, positionSeconds, updatedAt, recorded: true };
}

describe('buildStatistics', () => {
	it('aggregates catalog totals and orders courses by lesson volume', () => {
		const result = buildStatistics(catalog, []);
		expect(result.totals).toEqual({ courses: 2, modules: 2, lessons: 3, missingLessons: 0, videos: 3, materials: 0 });
		expect(result.courses.map((entry) => entry.course.title)).toEqual(['Curso longo', 'Curso curto']);
	});

	it('counts completed, in-progress and the last seven days from recent history', () => {
		const now = 1_800_000_000_000;
		const result = buildStatistics(catalog, [
			progress('m1', true, 120, now / 1000 - 60),
			progress('m2', false, 90, now / 1000 - 9 * 86_400),
			progress('m3', false, 0, now / 1000 - 60)
		], now);
		expect(result.recent.completed).toBe(1);
		expect(result.recent.inProgress).toBe(1);
		expect(result.recent.activeLast7Days).toBe(2);
		expect(result.activity).toHaveLength(3);
	});

	it('ignores recent media that no longer resolves to a catalog lesson', () => {
		const result = buildStatistics(catalog, [progress('removed', false, 10, 1_800_000_000)]);
		expect(result.activity).toEqual([]);
		expect(result.recent).toEqual({ completed: 0, inProgress: 0, activeLast7Days: 0 });
	});
});

describe('weekly goal', () => {
	it('accepts integers from 1 to 7 and falls back to 3', () => {
		expect(parseWeeklyGoal(1)).toBe(1);
		expect(parseWeeklyGoal('7')).toBe(7);
		expect(parseWeeklyGoal(3)).toBe(3);
		expect(parseWeeklyGoal(0)).toBe(DEFAULT_WEEKLY_GOAL);
		expect(parseWeeklyGoal(8)).toBe(DEFAULT_WEEKLY_GOAL);
		expect(parseWeeklyGoal('3.5')).toBe(DEFAULT_WEEKLY_GOAL);
		expect(parseWeeklyGoal(null)).toBe(DEFAULT_WEEKLY_GOAL);
	});

	it('reads and persists the preference without throwing on broken storage', () => {
		const values = new Map<string, string>();
		const storage = {
			getItem: (key: string) => values.get(key) ?? null,
			setItem: (key: string, value: string) => values.set(key, value)
		};
		expect(readWeeklyGoal(storage)).toBe(DEFAULT_WEEKLY_GOAL);
		expect(persistWeeklyGoal(storage, 5)).toBe(true);
		expect(readWeeklyGoal(storage)).toBe(5);
		expect(persistWeeklyGoal(storage, 99)).toBe(true);
		expect(readWeeklyGoal(storage)).toBe(DEFAULT_WEEKLY_GOAL);
		expect(readWeeklyGoal({ getItem: () => { throw new Error('blocked'); } })).toBe(DEFAULT_WEEKLY_GOAL);
	});

	it('clamps progress to the goal and to zero', () => {
		expect(weeklyGoalProgress(2, 3)).toBe(2);
		expect(weeklyGoalProgress(5, 3)).toBe(3);
		expect(weeklyGoalProgress(-1, 3)).toBe(0);
		expect(weeklyGoalProgress(2, 1)).toBe(1);
	});
});
