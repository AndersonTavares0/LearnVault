import { countCatalogNode, type CatalogCounts } from './catalog';
import { findLessonLocation, type LessonLocation } from './router';
import type { Catalog, CatalogNode, StudyProgress } from './transport';

export type CourseVolume = { course: CatalogNode; counts: CatalogCounts };

export type RecentActivity = { progress: StudyProgress; location: LessonLocation };

export type Statistics = {
	totals: CatalogCounts & { courses: number };
	courses: CourseVolume[];
	recent: { completed: number; inProgress: number; activeLast7Days: number };
	activity: RecentActivity[];
};

const DAY_SECONDS = 86_400;

/** Agrega o catálogo e o histórico recente sem consultar o servidor de novo. */
export function buildStatistics(catalog: Catalog | null, recent: StudyProgress[], now = Date.now()): Statistics {
	const courses = (catalog?.nodes ?? []).map((course) => ({ course, counts: countCatalogNode(course) }));
	const totals = courses.reduce<Statistics['totals']>(
		(sum, entry) => ({
			courses: sum.courses + 1,
			modules: sum.modules + entry.counts.modules,
			lessons: sum.lessons + entry.counts.lessons,
			missingLessons: sum.missingLessons + entry.counts.missingLessons,
			videos: sum.videos + entry.counts.videos,
			materials: sum.materials + entry.counts.materials
		}),
		{ courses: 0, modules: 0, lessons: 0, missingLessons: 0, videos: 0, materials: 0 }
	);

	const activity: RecentActivity[] = [];
	for (const progress of recent) {
		const location = findLessonLocation(catalog, progress.mediaId);
		if (location) activity.push({ progress, location });
	}

	const since = Math.floor(now / 1000) - 7 * DAY_SECONDS;
	const recentStats = {
		completed: activity.filter((entry) => entry.progress.completed).length,
		inProgress: activity.filter((entry) => !entry.progress.completed && entry.progress.positionSeconds > 0).length,
		activeLast7Days: activity.filter((entry) => entry.progress.updatedAt !== null && entry.progress.updatedAt >= since).length
	};

	courses.sort((left, right) => right.counts.lessons - left.counts.lessons || left.course.title.localeCompare(right.course.title, 'pt-BR'));

	return { totals, courses, recent: recentStats, activity };
}

export const WEEKLY_GOAL_STORAGE_KEY = 'learnvault-weekly-goal';
export const DEFAULT_WEEKLY_GOAL = 3;
export const MIN_WEEKLY_GOAL = 1;
export const MAX_WEEKLY_GOAL = 7;

/** Aceita apenas inteiros de 1 a 7; qualquer outro valor cai no padrão. */
export function parseWeeklyGoal(value: unknown): number {
	const parsed = typeof value === 'number' ? value : typeof value === 'string' ? Number(value) : Number.NaN;
	return Number.isInteger(parsed) && parsed >= MIN_WEEKLY_GOAL && parsed <= MAX_WEEKLY_GOAL ? parsed : DEFAULT_WEEKLY_GOAL;
}

export function readWeeklyGoal(storage: Pick<Storage, 'getItem'> | null | undefined): number {
	try {
		return parseWeeklyGoal(storage?.getItem(WEEKLY_GOAL_STORAGE_KEY));
	} catch {
		return DEFAULT_WEEKLY_GOAL;
	}
}

export function persistWeeklyGoal(storage: Pick<Storage, 'setItem'> | null | undefined, goal: number): boolean {
	if (!storage) return false;
	try {
		storage.setItem(WEEKLY_GOAL_STORAGE_KEY, String(parseWeeklyGoal(goal)));
		return true;
	} catch {
		return false;
	}
}

export function weeklyGoalProgress(activeDays: number, goal: number): number {
	return Math.max(0, Math.min(activeDays, parseWeeklyGoal(goal)));
}
