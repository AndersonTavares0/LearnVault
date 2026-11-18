import type { CatalogNode, FileEntry } from './transport';

export type LessonRef = { title: string; path: string };
export type LessonSection = { title: string | null; lessons: LessonRef[] };

type IndexedLesson = { ref: LessonRef; ancestors: CatalogNode[] };

/**
 * Index of the lessons in the catalog's canonical order.
 *
 * The order is that of the tree itself (depth-first search), which is the order the Rust
 * catalog already delivers: category, course, module and lesson, with the modules before the
 * course's loose lessons. Walking the tree instead of the projected categories keeps a single
 * source of truth — the canonical tree — and avoids duplicating lessons when both
 * projections exist. Each lesson keeps its chain of ancestors so that the grouping
 * knows which course or module it belongs to without a second traversal.
 */
function indexLessons(nodes: CatalogNode[]): IndexedLesson[] {
	const indexed: IndexedLesson[] = [];
	function walk(list: CatalogNode[], ancestors: CatalogNode[]): void {
		if (!Array.isArray(list)) return;
		for (const node of list) {
			if (!node) continue;
			if (node.role === 'lesson') indexed.push({ ref: { title: node.title, path: node.path }, ancestors });
			if (Array.isArray(node.children) && node.children.length > 0) walk(node.children, [...ancestors, node]);
		}
	}
	walk(nodes, []);
	return indexed;
}

/** All the catalog's lessons, in canonical order, with none repeated. */
export function lessonSequence(nodes: CatalogNode[]): LessonRef[] {
	return indexLessons(nodes).map((entry) => entry.ref);
}

/**
 * Previous and next lesson in the catalog's canonical order.
 *
 * The limits return `null` instead of wrapping around, so that the interface hides the
 * control instead of promising a navigation that does not exist. A missing or
 * unknown path (for example, after a reindex that removed the lesson) also
 * returns both sides null: the player does not invent neighbours for a lesson that vanished.
 */
export function neighboringLessons(nodes: CatalogNode[], lessonPath: string | null): { previous: LessonRef | null; next: LessonRef | null } {
	if (lessonPath === null) return { previous: null, next: null };
	const sequence = lessonSequence(nodes);
	const position = sequence.findIndex((ref) => ref.path === lessonPath);
	if (position < 0) return { previous: null, next: null };
	return { previous: sequence[position - 1] ?? null, next: sequence[position + 1] ?? null };
}

/**
 * Section container: the nearest ancestor that groups lessons — module, course or
 * categoria.
 *
 * The section is the lesson's nearest visible grouping: `category/course/module/lesson`
 * sits under the module; without a module, under the course; without a course, under the category. A lesson with no
 * container (an atypical tree, without categories) falls into a section with no title, so the index
 * stays useful instead of empty.
 */
function sectionContainer(ancestors: CatalogNode[]): CatalogNode | null {
	return [...ancestors].reverse().find((node) => node.role === 'course' || node.role === 'module' || node.role === 'category') ?? null;
}

/**
 * Sections of the lesson index: all the catalog's lessons grouped by the nearest
 * container, in canonical order.
 *
 * Containers are identified by the tree node (full path): two courses with the
 * same title at different paths do not merge. The order of the sections is that of the first
 * lesson of each, so the index never falls out of the catalog's order.
 */
export function lessonSections(nodes: CatalogNode[]): LessonSection[] {
	const sections: LessonSection[] = [];
	const byContainer = new Map<CatalogNode | null, LessonSection>();
	for (const entry of indexLessons(nodes)) {
		const container = sectionContainer(entry.ancestors);
		let section = byContainer.get(container);
		if (!section) {
			section = { title: container?.title ?? null, lessons: [] };
			byContainer.set(container, section);
			sections.push(section);
		}
		section.lessons.push(entry.ref);
	}
	return sections;
}

// Playable media: a media file that the last scan did not mark as missing.
function playableMedia(file: FileEntry): boolean {
	return file.kind === 'media' && file.availability?.trim().toLowerCase() !== 'missing';
}

// Playable lesson: at least one playable media among the lesson's own files.
function playableLesson(node: CatalogNode): boolean {
	return [...node.files, ...node.looseFiles].some(playableMedia);
}

/**
 * First available lesson in canonical order.
 *
 * Lessons take precedence over loose media, because the lesson is what the side list highlights.
 * With no playable lesson, the first playable loose file will do too: a course
 * with only loose media still opens something instead of an empty screen.
 */
export function firstAvailableLessonPath(nodes: CatalogNode[]): string | null {
	function findLesson(list: CatalogNode[]): string | null {
		for (const node of list) {
			if (!node) continue;
			if (node.role === 'lesson' && playableLesson(node)) return node.path;
			const found = findLesson(node.children);
			if (found) return found;
		}
		return null;
	}
	function findLoose(list: CatalogNode[]): string | null {
		for (const node of list) {
			if (!node) continue;
			const loose = node.looseFiles.find(playableMedia);
			if (loose) return loose.path;
			const found = findLoose(node.children);
			if (found) return found;
		}
		return null;
	}
	return findLesson(nodes) ?? findLoose(nodes);
}

/**
 * Path that contains a `media_id` in the subtree: the lesson of a lesson media or the
 * loose file of a container.
 *
 * Only returns paths that `findLesson` can open and ignores media marked as
 * missing, so that the recent history never picks a lesson without a file.
 */
export function lessonPathForMedia(nodes: CatalogNode[], mediaId: string | null): string | null {
	if (!mediaId) return null;
	function visit(list: CatalogNode[]): string | null {
		for (const node of list) {
			if (!node) continue;
			if (node.role === 'lesson' && [...node.files, ...node.looseFiles].some((file) => playableMedia(file) && file.mediaId === mediaId)) return node.path;
			const loose = node.looseFiles.find((file) => playableMedia(file) && file.mediaId === mediaId);
			if (node.role !== 'lesson' && loose) return loose.path;
			const found = visit(node.children);
			if (found) return found;
		}
		return null;
	}
	return visit(nodes);
}

/**
 * Initial path of a course: the most recent media of the history that still exists in the
 * course or, with no usable history, the first available lesson.
 *
 * `mediaIds` arrives from newest to oldest. The history belongs to the user, not to the
 * course, so media of other courses simply find no path here.
 */
export function defaultLessonPath(nodes: CatalogNode[], mediaIds: string[]): string | null {
	for (const mediaId of mediaIds) {
		const path = lessonPathForMedia(nodes, mediaId);
		if (path) return path;
	}
	return firstAvailableLessonPath(nodes);
}

/**
 * Nearest visible container of a lesson or loose media: module or category.
 *
 * It serves to show the materials of the open lesson's module outside the playlist, without repeating
 * those materials on every row of the side list. A lesson directly under the course
 * has no container and returns `null`.
 */
export function lessonContainer(nodes: CatalogNode[], lessonPath: string | null): CatalogNode | null {
	if (!lessonPath) return null;
	function visit(list: CatalogNode[], container: CatalogNode | null): CatalogNode | null {
		for (const node of list) {
			if (!node) continue;
			const next = node.role === 'module' || node.role === 'category' ? node : container;
			if (node.path === lessonPath) return container;
			if ([...node.files, ...node.looseFiles].some((file) => file.path === lessonPath)) {
				return node.role === 'module' || node.role === 'category' ? node : container;
			}
			const found = visit(node.children, next);
			if (found) return found;
		}
		return null;
	}
	return visit(nodes, null);
}
