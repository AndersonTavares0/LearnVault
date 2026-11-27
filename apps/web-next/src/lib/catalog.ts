import { tr } from './i18n';
import { mediaPlaybackKind, selectPlayableMedia } from './player';
import type { CatalogNode, FileEntry } from './transport';

export type FileCounts = { videos: number; audios?: number; materials: number };
export type CatalogCounts = FileCounts & { modules: number; lessons: number; missingLessons: number };

/** Study data follows the selected media and retains its catalog ID when playback is unavailable. */
export function lessonStudyMediaId(media: FileEntry[], preferredMediaId?: string): string | null {
	const playable = selectPlayableMedia(media, preferredMediaId);
	if (playable) return playable.mediaId ?? null;
	const preferred = preferredMediaId && media.find((file) => file.mediaId === preferredMediaId);
	return (preferred || media.find((file) => file.mediaId))?.mediaId ?? null;
}

/**
 * Media file that the last scan still finds on disk.
 *
 * The comparison is tolerant (whitespace and case) and a missing `availability` field counts as
 * available — the same tolerant rule already used by the router (`findLesson`) and by the list
 * of loose files: a file without the field is never treated as missing by omission.
 */
function isFileAvailable(file: FileEntry): boolean {
	return file.availability?.trim().toLowerCase() !== 'missing';
}

/**
 * Lesson with no playable media.
 *
 * A lesson is missing when it has at least one media file (`kind === 'media'`) and
 * none of them is available. Only the media decides: subtitles and materials do not reopen a
 * lesson whose media disappeared, because the media is what the row opens in the player — a lesson with
 * the media missing and the subtitle present is still missing for the person who is going to study.
 *
 * A lesson with no media file at all (only material, or empty) is never missing: there is no
 * media to be missing.
 */
export function isLessonMissing(node: CatalogNode): boolean {
	const media = [...node.files, ...node.looseFiles].filter((file) => file.kind === 'media');
	return media.length > 0 && media.every((file) => !isFileAvailable(file));
}

/**
 * Files of the node itself, without descending into the children.
 *
 * Subtitles go into `materials` on purpose: for the person studying, they are a resource of the
 * lesson, not a lesson. This is the count that describes what the lesson row opens in the player —
 * a lesson with bonus subfolders counts the subfolders' files in each child row,
 * not in the parent row.
 */
export function countCatalogFiles(node: CatalogNode): FileCounts {
	const counts: FileCounts = { videos: 0, audios: 0, materials: 0 };
	for (const file of [...node.files, ...node.looseFiles]) {
		if (file.kind === 'media') {
			if (mediaPlaybackKind(file) === 'audio') counts.audios = (counts.audios ?? 0) + 1;
			else counts.videos += 1;
		} else if (file.kind === 'subtitle' || file.kind === 'material') counts.materials += 1;
	}
	return counts;
}

/**
 * Media in the subtree that the last rescan marked as missing.
 *
 * The video count includes missing records on purpose (the catalog does not delete them),
 * but the fact sheet must say how many actually open — otherwise "2 videos" next to
 * "1 lesson" looks like a mistake. Lessons already use the same distinction.
 */
export function countMissingVideos(node: CatalogNode): number {
	let missing = 0;
	function walk(current: CatalogNode): void {
		for (const file of [...current.files, ...current.looseFiles]) {
			if (file.kind === 'media' && !isFileAvailable(file)) missing += 1;
		}
		for (const child of current.children) walk(child);
	}
	walk(node);
	return missing;
}

/**
 * Counts of a node and of its whole subtree.
 *
 * `modules` counts nodes by role; `videos` and `materials` add up the files of the node and of the
 * descendants. The node itself enters the count when it is a lesson, so that a
 * course's meta describes the whole subtree.
 *
 * Lessons are counted in two sums: `lessons` holds those with available media and
 * `missingLessons` those the scan marked as missing. A missing lesson stays
 * catalogued, listed and counted — it just does not mix with the ones the learner can open,
 * and so it does not inflate the count of available ones.
 */
export function countCatalogNode(node: CatalogNode): CatalogCounts {
	const counts: CatalogCounts = { modules: 0, lessons: 0, missingLessons: 0, videos: 0, materials: 0 };
	function walk(current: CatalogNode): void {
		if (current.role === 'module') counts.modules += 1;
		if (current.role === 'lesson') {
			if (isLessonMissing(current)) counts.missingLessons += 1;
			else counts.lessons += 1;
		}
		const files = countCatalogFiles(current);
		counts.videos += files.videos + (files.audios ?? 0);
		counts.materials += files.materials;
		for (const child of current.children) walk(child);
	}
	walk(node);
	return counts;
}

/**
 * Lesson count of a subtree: the available ones and, when there are any, the missing ones.
 *
 * `4 lessons`, `4 lessons · 4 missing`, `1 lesson · 3 missing`, `0 lessons · 4 missing`.
 * The available count always appears so that the catalog total stays legible;
 * the missing one only appears when it exists, so as not to clutter an intact catalog.
 *
 * A lesson folder can contain several distinct media files (the catalog model
 * treats any leaf folder with files as a single lesson, documented in
 * `role_of` in the Rust core). When that happens, `videos` exceeds `lessons +
 * missingLessons`, and the video count appears alongside so that "12 lessons" does not look
 * too low next to a course with 20 videos packed into those 12 folders.
 */
export function formatLessonCounts(counts: CatalogCounts): string {
	return joinParts([
		formatCount(counts.lessons, tr('lesson', 'aula'), tr('lessons', 'aulas')),
		counts.missingLessons > 0 ? formatCount(counts.missingLessons, tr('missing', 'ausente'), tr('missing', 'ausentes')) : '',
		counts.videos > counts.lessons + counts.missingLessons
			? formatCount(counts.videos, tr('video', 'vídeo'), tr('videos', 'vídeos'))
			: ''
	]);
}

export function formatCount(value: number, singular: string, plural: string): string {
	return `${value} ${value === 1 ? singular : plural}`;
}

export type CourseProgressStatus = 'all' | 'in_progress' | 'not_started' | 'completed';

/**
 * Determines the study status of a course based on completed and in-progress lessons.
 */
export function getCourseProgressStatus(
	courseNode: CatalogNode,
	completedMediaIds: Set<string>,
	inProgressMediaIds?: Set<string>
): 'completed' | 'in_progress' | 'not_started' {
	let totalLessons = 0;
	let completedLessons = 0;
	let hasInProgress = false;

	function walk(current: CatalogNode): void {
		if (current.role === 'lesson') {
			if (!isLessonMissing(current)) {
				const mediaFiles = [...current.files, ...current.looseFiles].filter((f) => f.kind === 'media' && f.mediaId);
				if (mediaFiles.length > 0) {
					totalLessons += 1;
					const isCompleted = mediaFiles.some((f) => f.mediaId && completedMediaIds.has(f.mediaId));
					if (isCompleted) {
						completedLessons += 1;
					} else if (inProgressMediaIds && mediaFiles.some((f) => f.mediaId && inProgressMediaIds.has(f.mediaId))) {
						hasInProgress = true;
					}
				}
			}
		}
		for (const child of current.children) walk(child);
	}

	walk(courseNode);

	if (totalLessons > 0 && completedLessons === totalLessons) return 'completed';
	if (completedLessons > 0 || hasInProgress) return 'in_progress';
	return 'not_started';
}

/** Completed and total playable lessons of a course; a lesson counts once even with several videos. */
export function courseProgress(courseNode: CatalogNode, completedMediaIds: ReadonlySet<string>): { done: number; total: number } {
	let done = 0;
	let total = 0;
	const walk = (current: CatalogNode): void => {
		if (current.role === 'lesson' && !isLessonMissing(current)) {
			const ids = [...current.files, ...current.looseFiles].flatMap((file) => (file.kind === 'media' && file.mediaId ? [file.mediaId] : []));
			if (ids.length > 0) {
				total += 1;
				if (ids.some((id) => completedMediaIds.has(id))) done += 1;
			}
		}
		current.children.forEach(walk);
	};
	walk(courseNode);
	return { done, total };
}

function joinParts(parts: string[]): string {
	return parts.filter(Boolean).join(' · ');
}

/** Course metadata: modules, lessons (available and missing), and materials; zero counts are omitted. */
export function formatCourseMeta(counts: CatalogCounts): string {
	return joinParts([
		counts.modules > 0 ? formatCount(counts.modules, tr('module', 'módulo'), tr('modules', 'módulos')) : '',
		counts.lessons > 0 || counts.missingLessons > 0 ? formatLessonCounts(counts) : '',
		counts.materials > 0 ? formatCount(counts.materials, tr('material', 'material'), tr('materials', 'materiais')) : ''
	]);
}

/** Short metadata for a course card: the essentials for choosing what to study. */
export function formatCourseCardMeta(counts: CatalogCounts): string {
	return joinParts([
		counts.modules > 0 ? formatCount(counts.modules, tr('module', 'módulo'), tr('modules', 'módulos')) : '',
		counts.lessons > 0 || counts.missingLessons > 0 ? formatLessonCounts(counts) : ''
	]);
}

/** Lesson row metadata: videos, audios, and materials in the node itself, omitting zero counts. */
export function formatLessonMeta(counts: FileCounts): string {
	const audios = counts.audios ?? 0;
	return joinParts([
		counts.videos > 0 ? formatCount(counts.videos, tr('video', 'vídeo'), tr('videos', 'vídeos')) : '',
		audios > 0 ? formatCount(audios, tr('audio', 'áudio'), tr('audios', 'áudios')) : '',
		counts.materials > 0 ? formatCount(counts.materials, tr('material', 'material'), tr('materials', 'materiais')) : ''
	]);
}

/**
 * Materials (not media) of a whole course, without repeating the same path.
 *
 * The course fact sheet shows everything that accompanies the lessons, including what is inside
 * modules, in a single list. Subtitles and support files go in; videos are left
 * out because they are already the lessons.
 */
export function collectMaterials(node: CatalogNode): FileEntry[] {
	const byPath = new Map<string, FileEntry>();
	function walk(current: CatalogNode): void {
		for (const file of [...current.files, ...current.looseFiles]) {
			if (file.kind !== 'media' && !byPath.has(file.path)) byPath.set(file.path, file);
		}
		for (const child of current.children) walk(child);
	}
	walk(node);
	return [...byPath.values()];
}

/** Origin context of a recent item: the course and, when there is one, the module. */
export function formatLocationContext(courseTitle: string, moduleTitle: string | null): string {
	return joinParts([courseTitle, moduleTitle ?? '']);
}

/** Keep a course's own labels; only substitute the module for a generic media filename. */
export function displayLessonTitle(title: string, moduleTitle?: string | null): string {
	return moduleTitle && /^(lesson|aula)$/i.test(title.trim()) ? moduleTitle : title;
}

/**
 * Removes numeric prefixes and separators of category folders for a readable display.
 *
 * Ex: "01 - Programming and Development" -> "Programming and Development"
 *     "02. Interface Design" -> "Interface Design"
 */
export function formatCategoryName(folder: string): string {
	const trimmed = folder.trim();
	const cleaned = trimmed.replace(/^\d+[\s._-]+/, '').trim();
	return cleaned || trimmed;
}
