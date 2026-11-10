import { courseProgress, getCourseProgressStatus } from './catalog';
import { defaultLessonPath } from './navigation';
import { findLesson, findLessonLocation, type LessonLocation } from './router';
import { filterCatalog } from './search';
import { courseIdOf, type Catalog, type CatalogNode, type StudyProgress } from './transport';

export type LibraryFilter = 'all' | 'in_progress' | 'not_started' | 'completed' | 'favorites';

export type CourseEntry = {
	course: CatalogNode;
	courseId: string;
	status: 'completed' | 'in_progress' | 'not_started';
	done: number;
	total: number;
	favorite: boolean;
	/** Lesson to open from the card: the resume point, else the first available lesson. */
	next: { path: string; title: string } | null;
};

export type ResumeEntry = { progress: StudyProgress; location: LessonLocation; courseProgress: number };

export type Library = {
	entries: CourseEntry[];
	visible: CourseEntry[];
	counts: Record<LibraryFilter, number>;
	categories: string[];
	resume: ResumeEntry[];
};

/** Everything My learning shows, derived from the catalog, the study history and favourites. */
export function buildLibrary(input: {
	catalog: Catalog | null | undefined;
	recent: StudyProgress[];
	favoriteCourseIds: ReadonlySet<string>;
	query?: string;
	filter?: LibraryFilter;
	category?: string | null;
}): Library {
	const nodes = input.catalog?.nodes ?? [];
	const completed = new Set(input.recent.filter((item) => item.completed).map((item) => item.mediaId));
	const started = new Set(input.recent.filter((item) => !item.completed && item.positionSeconds > 0).map((item) => item.mediaId));
	const recentIds = input.recent.map((item) => item.mediaId);

	const entries = nodes.map((course): CourseEntry => {
		const courseId = courseIdOf(course);
		const { done, total } = courseProgress(course, completed);
		const nextPath = defaultLessonPath([course], recentIds);
		const nextLesson = nextPath ? findLesson(input.catalog ?? null, courseId, nextPath) : null;
		return {
			course,
			courseId,
			status: getCourseProgressStatus(course, completed, started),
			done,
			total,
			favorite: input.favoriteCourseIds.has(courseId),
			next: nextPath && nextLesson ? { path: nextPath, title: nextLesson.title } : null
		};
	});

	const byId = new Map(entries.map((entry) => [entry.courseId, entry]));
	const matching = new Set(filterCatalog(nodes, input.query ?? '').map(courseIdOf));
	const filter = input.filter ?? 'all';
	const visible = entries.filter((entry) =>
		matching.has(entry.courseId) &&
		(filter === 'all' || (filter === 'favorites' ? entry.favorite : entry.status === filter)) &&
		(!input.category || entry.course.sourceFolder === input.category)
	);

	const counts: Record<LibraryFilter, number> = { all: entries.length, in_progress: 0, not_started: 0, completed: 0, favorites: 0 };
	for (const entry of entries) {
		counts[entry.status] += 1;
		if (entry.favorite) counts.favorites += 1;
	}

	const categories = [...new Set(nodes.map((node) => node.sourceFolder).filter((folder): folder is string => Boolean(folder)))].sort((a, b) => a.localeCompare(b));

	// One resume card per course, most recent first, skipping lessons already completed.
	const seen = new Set<string>();
	const resume: ResumeEntry[] = [];
	for (const progress of input.recent) {
		if (progress.completed) continue;
		const location = findLessonLocation(input.catalog ?? null, progress.mediaId);
		if (!location || seen.has(location.courseId)) continue;
		seen.add(location.courseId);
		const entry = byId.get(location.courseId);
		resume.push({ progress, location, courseProgress: entry && entry.total ? entry.done / entry.total : 0 });
		if (resume.length === 4) break;
	}

	return { entries, visible, counts, categories, resume };
}
