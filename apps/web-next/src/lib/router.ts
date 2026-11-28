import { isPdf } from './pdf';
import { courseIdOf, lessonFromNode, type Catalog, type CatalogNode, type FileEntry, type Lesson } from './transport';

export type Route = 'login' | 'home' | 'course' | 'player' | 'settings' | 'statistics' | 'notes' | 'history';
export type HashLocation = { route: Route; courseId: string | null; lessonPath: string | null; positionSeconds?: number | null };

function decodeSegment(value: string | undefined): string | null {
	if (value === undefined) return null;
	try {
		return decodeURIComponent(value);
	} catch {
		return null;
	}
}

/**
 * Parses the location hash.
 *
 * `#/course/<id>` opens a course and `#/course/<id>/<path>` opens the lesson indicated inside
 * it: a lesson's relative path is only unique per course, so the course's identity is
 * part of the route. `#/player/<id>/<path>` is the legacy form of the same screen and stays
 * accepted. `#/settings` opens the local preferences, `#/statistics` the study
 * summary and `#/notes` the global notes panel; none of them identifies a course.
 * The optional parameter `?t=<seconds>` positions the player directly at the note's timestamp.
 * `#/player/<path>` is the legacy form of a loose file with no owning course;
 * the path must start the global search. Any other value falls to the build's default route:
 * the server sign-in and the home screen on the desktop, which has no session.
 */
export function locationFromHash(hash: string, fallback: Route = 'login'): HashLocation {
	const [cleanHash, queryString] = hash.replace(/^#\/?/, '').split('?');
	let positionSeconds: number | null = null;
	if (queryString) {
		const params = new URLSearchParams(queryString);
		const t = params.get('t');
		if (t !== null) {
			const parsed = Number(t);
			if (Number.isFinite(parsed) && parsed >= 0) {
				positionSeconds = parsed;
			}
		}
	}
	const [name, ...parts] = cleanHash.split('/');
	if (name === 'focus') {
		const res: HashLocation = { route: 'statistics', courseId: null, lessonPath: null };
		if (positionSeconds !== null) res.positionSeconds = positionSeconds;
		return res;
	}
	if (name === 'notes') {
		const res: HashLocation = { route: 'notes', courseId: null, lessonPath: null };
		if (positionSeconds !== null) res.positionSeconds = positionSeconds;
		return res;
	}
	if (name === 'history') {
		const res: HashLocation = { route: 'history', courseId: null, lessonPath: null };
		if (positionSeconds !== null) res.positionSeconds = positionSeconds;
		return res;
	}
	const route = name === 'home' || name === 'course' || name === 'player' || name === 'settings' || name === 'statistics' || name === 'notes' || name === 'history' ? name : fallback;
	if (route === 'course') {
		if (parts.length === 0) {
			const res: HashLocation = { route, courseId: null, lessonPath: null };
			if (positionSeconds !== null) res.positionSeconds = positionSeconds;
			return res;
		}
		const lessonString = parts.slice(1).join('/');
		const res: HashLocation = { route, courseId: decodeSegment(parts[0]), lessonPath: lessonString === '' ? null : decodeSegment(lessonString) };
		if (positionSeconds !== null) res.positionSeconds = positionSeconds;
		return res;
	}
	if (route === 'player') {
		if (parts.length === 0) {
			const res: HashLocation = { route, courseId: null, lessonPath: null };
			if (positionSeconds !== null) res.positionSeconds = positionSeconds;
			return res;
		}
		if (parts.length === 1) {
			const res: HashLocation = { route, courseId: null, lessonPath: decodeSegment(parts[0]) };
			if (positionSeconds !== null) res.positionSeconds = positionSeconds;
			return res;
		}
		const res: HashLocation = { route, courseId: decodeSegment(parts[0]), lessonPath: decodeSegment(parts.slice(1).join('/')) };
		if (positionSeconds !== null) res.positionSeconds = positionSeconds;
		return res;
	}
	const res: HashLocation = { route, courseId: null, lessonPath: null };
	if (positionSeconds !== null) res.positionSeconds = positionSeconds;
	return res;
}

export function routeFromHash(hash: string, fallback: Route = 'login'): Route {
	return locationFromHash(hash, fallback).route;
}

/** Builds the canonical hash of a route. */
export function href(route: Route, courseId?: string, lessonPath?: string, positionSeconds?: number | null): string {
	const query = positionSeconds !== undefined && positionSeconds !== null && positionSeconds >= 0 ? `?t=${Math.floor(positionSeconds)}` : '';
	if (route === 'course' && courseId !== undefined) {
		const base = `#/course/${encodeURIComponent(courseId)}`;
		return (lessonPath ? `${base}/${encodeURIComponent(lessonPath)}` : base) + query;
	}
	if (route === 'player' && courseId !== undefined && lessonPath !== undefined) {
		return `#/player/${encodeURIComponent(courseId)}/${encodeURIComponent(lessonPath)}` + query;
	}
	if (route === 'player' && courseId !== undefined && lessonPath === undefined) {
		return `#/player/${encodeURIComponent(courseId)}` + query;
	}
	return `#/${route}` + query;
}

/** Safe destination after sign-in, preserving an already navigable route. */
export function destinationAfterLogin(hash: string): string {
	const location = locationFromHash(hash);
	if (location.route === 'home' || location.route === 'course' || location.route === 'settings' || location.route === 'statistics' || location.route === 'notes' || location.route === 'history') return href(location.route, location.courseId ?? undefined, location.lessonPath ?? undefined);
	if (location.route === 'player') {
		const parts = hash.replace(/^#\/?/, '').split('/');
		if (parts.length === 1) {
			const path = decodeSegment(parts[0]);
			return path ? href('player', path) : href('home');
		}
		if (location.courseId && location.lessonPath) return href('player', location.courseId, location.lessonPath);
	}
	return href('home');
}

/** Top-level course of the catalog by opaque identity (or by path in the legacy format). */
export function findCourse(catalog: Catalog | null, courseId: string | null): CatalogNode | null {
	if (!catalog || !courseId) return null;
	return catalog.nodes.find((node) => courseIdOf(node) === courseId) ?? null;
}

/**
 * Lesson inside a course by relative path.
 *
 * The search is restricted to the course's subtree: two courses may have the same relative
 * path, and the course's identity breaks the tie. Without a course (a loose file at the root), the
 * search is global with `media_id` and the exact path as the criterion, and never returns
 * missing materials.
 */
export function findLesson(catalog: Catalog | null, courseId: string | null, lessonPath: string | null): Lesson | null {
	if (!catalog || !lessonPath) return null;
	const playable = (file: FileEntry) =>
		file.kind === 'media' && file.availability?.trim().toLowerCase() !== 'missing' && file.path === lessonPath;
	// A PDF kept loose in the course or a module has no lesson node; it still opens in the reader.
	const readablePdf = (file: FileEntry) =>
		isPdf(file) && Boolean(file.mediaId) && file.availability?.trim().toLowerCase() !== 'missing' && file.path === lessonPath;
	if (courseId) {
		const course = findCourse(catalog, courseId);
		if (!course) return null;
		function visit(node: CatalogNode): Lesson | null {
			if (node.role === 'lesson' && node.path === lessonPath) return lessonFromNode(node);
			const loose = node.looseFiles.find(playable);
			// Search and study records can link to a file inside a folder-based lesson.
			if (node.role === 'lesson' && (loose || node.files.some(playable))) return lessonFromNode(node);
			if (loose) return { title: loose.name, path: loose.path, media: [loose], subtitles: [], materials: [] };
			const document = node.looseFiles.find(readablePdf);
			if (document) return { title: cleanMediaTitle(document.name), path: document.path, media: [], subtitles: [], materials: [document] };
			for (const child of node.children) {
				const found = visit(child);
				if (found) return found;
			}
			return null;
		}
		return visit(course);
	}
	return standaloneMedia(catalog, (file) => file.path === lessonPath);
}

/** Location of a lesson by `media_id`: the course, the module (when there is one) and the lesson. */
export type LessonLocation = {
	courseId: string;
	courseTitle: string;
	moduleTitle: string | null;
	lesson: Lesson;
};

// Playable media: a media file that the last scan did not mark as missing.
function playableMedia(file: FileEntry): boolean {
	return file.kind === 'media' && file.availability?.trim().toLowerCase() !== 'missing';
}

// Visible grouping of lessons in the canonical tree: module or category. The first level under the
// course is served with the `category` role, so it also counts as module context.
function groupsLessons(node: CatalogNode): boolean {
	return node.role === 'module' || node.role === 'category';
}

/**
 * Presented title of a loose media: without a numeric prefix or extension.
 *
 * Repeats the cleanup the catalog applies to the title of a lesson derived from a file, so that
 * the client's safety net never shows the raw file name on the home screen.
 */
export function cleanMediaTitle(name: string): string {
	const lastDot = name.lastIndexOf('.');
	const stem = lastDot > 0 ? name.slice(0, lastDot) : name;
	const cleaned = stem.replace(/^[0-9._\- •\t]+/, '').trim();
	return cleaned || name;
}

/**
 * Location of a lesson by `media_id`: the course, the module (when there is one) and the lesson.
 *
 * The course is the top-level node that contains the media. The module is the grouping nearest the lesson —
 * module or category — and is `null` when the lesson sits directly under the course. Loose media at the
 * course root remains a standalone item (the catalog keeps it in `loose_files`), resolved
 * with the cleaned title and no module.
 */
export function findLessonLocation(catalog: Catalog | null, mediaId: string | null): LessonLocation | null {
	if (!catalog || !mediaId) return null;
	for (const course of catalog.nodes) {
		const found = locateInCourse(course, mediaId);
		if (found) return found;
	}
	return null;
}

function locateInCourse(course: CatalogNode, mediaId: string): LessonLocation | null {
	function visit(node: CatalogNode, moduleTitle: string | null, isRoot: boolean): { moduleTitle: string | null; lesson: Lesson } | null {
		if (node.role === 'lesson' && [...node.files, ...node.looseFiles].some((file) => file.mediaId === mediaId)) {
			return { moduleTitle, lesson: lessonFromNode(node) };
		}
		// The course root itself never counts as a module: only the grouping below it.
		const nested = !isRoot && groupsLessons(node) ? node.title : moduleTitle;
		for (const child of node.children) {
			const found = visit(child, nested, false);
			if (found) return found;
		}
		return null;
	}
	const found = visit(course, null, true);
	if (found) return { courseId: courseIdOf(course), courseTitle: course.title, moduleTitle: found.moduleTitle, lesson: found.lesson };
	const loose = course.looseFiles.find((file) => playableMedia(file) && file.mediaId === mediaId);
	if (loose) {
		return {
			courseId: courseIdOf(course),
			courseTitle: course.title,
			moduleTitle: null,
			lesson: { title: cleanMediaTitle(loose.name), path: loose.path, media: [loose], subtitles: [], materials: [] }
		};
	}
	return null;
}

// Media files outside a lesson use the same player, without navigating to the raw URL.
function standaloneMedia(catalog: Catalog, matches: (file: FileEntry) => boolean): Lesson | null {
	function fromFiles(files: FileEntry[]): Lesson | null {
		const file = files.find((entry) => entry.kind === 'media' && entry.availability?.trim().toLowerCase() !== 'missing' && matches(entry));
		return file ? { title: file.name, path: file.path, media: [file], subtitles: [], materials: [] } : null;
	}
	function visit(nodes: Catalog['nodes']): Lesson | null {
		for (const node of nodes) {
			const found = fromFiles(node.files) ?? fromFiles(node.looseFiles) ?? visit(node.children);
			if (found) return found;
		}
		return null;
	}
	return fromFiles(catalog.looseFiles) ?? visit(catalog.nodes);
}
