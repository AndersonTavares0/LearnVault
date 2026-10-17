import { courseIdOf, type Catalog, type CatalogNode, type Lesson } from './transport';

export type Route = 'login' | 'home' | 'course' | 'player';
export type HashLocation = { route: Route; courseId: string | null; lessonPath: string | null };

function decodeSegment(value: string | undefined): string | null {
	if (value === undefined) return null;
	try {
		return decodeURIComponent(value);
	} catch {
		return null;
	}
}

/**
 * Interpreta o hash da localização.
 *
 * `#/course/<id>` abre um curso e `#/player/<id>/<caminho>` abre uma aula dentro do curso:
 * o caminho relativo da aula só é único por curso, então a identidade do curso faz parte
 * da rota. Qualquer outro valor cai no login.
 */
export function locationFromHash(hash: string): HashLocation {
	const value = hash.replace(/^#\/?/, '');
	const [name, ...parts] = value.split('/');
	const route = name === 'home' || name === 'course' || name === 'player' ? name : 'login';
	if (route === 'course') return { route, courseId: decodeSegment(parts[0]), lessonPath: null };
	if (route === 'player') return { route, courseId: decodeSegment(parts[0]), lessonPath: parts.length > 1 ? decodeSegment(parts.slice(1).join('/')) : null };
	return { route, courseId: null, lessonPath: null };
}

export function routeFromHash(hash: string): Route {
	return locationFromHash(hash).route;
}

/** Monta o hash canônico de uma rota. */
export function href(route: Route, courseId?: string, lessonPath?: string): string {
	if (route === 'course' && courseId !== undefined) return `#/course/${encodeURIComponent(courseId)}`;
	if (route === 'player' && courseId !== undefined && lessonPath !== undefined) {
		return `#/player/${encodeURIComponent(courseId)}/${encodeURIComponent(lessonPath)}`;
	}
	return `#/${route}`;
}

/** Destino seguro depois do login, preservando uma rota já navegável. */
export function destinationAfterLogin(hash: string): string {
	const location = locationFromHash(hash);
	if (location.route === 'home' || location.route === 'course') return href(location.route, location.courseId ?? undefined);
	if (location.route === 'player' && location.courseId && location.lessonPath) return href('player', location.courseId, location.lessonPath);
	return href('home');
}

/** Curso de topo do catálogo pela identidade opaca (ou pelo caminho no formato legado). */
export function findCourse(catalog: Catalog | null, courseId: string | null): CatalogNode | null {
	if (!catalog || !courseId) return null;
	return catalog.nodes.find((node) => courseIdOf(node) === courseId) ?? null;
}

function lessonFromNode(node: CatalogNode): Lesson {
	const files = [...node.files, ...node.looseFiles];
	return { title: node.title, path: node.path, media: files.filter((file) => file.kind === 'media'), subtitles: files.filter((file) => file.kind === 'subtitle'), materials: files.filter((file) => file.kind === 'material') };
}

/**
 * Aula dentro de um curso pelo caminho relativo.
 *
 * A busca é restrita à subárvore do curso: dois cursos podem ter o mesmo caminho
 * relativo, e a identidade do curso desempata.
 */
export function findLesson(catalog: Catalog | null, courseId: string | null, lessonPath: string | null): Lesson | null {
	const course = findCourse(catalog, courseId);
	if (!course || !lessonPath) return null;
	function visit(node: CatalogNode): Lesson | null {
		if (node.role === 'lesson' && node.path === lessonPath) return lessonFromNode(node);
		for (const child of node.children) {
			const found = visit(child);
			if (found) return found;
		}
		return null;
	}
	if (course.role === 'lesson' && course.path === lessonPath) return lessonFromNode(course);
	return visit(course);
}

/** Localização de uma aula pelo `media_id`: o curso e a aula que a contêm. */
export function findLessonLocation(catalog: Catalog | null, mediaId: string | null): { courseId: string; lesson: Lesson } | null {
	if (!catalog || !mediaId) return null;
	for (const course of catalog.nodes) {
		function visit(node: CatalogNode): Lesson | null {
			if (node.role === 'lesson') {
				const files = [...node.files, ...node.looseFiles];
				if (files.some((file) => file.mediaId === mediaId)) return lessonFromNode(node);
			}
			for (const child of node.children) {
				const found = visit(child);
				if (found) return found;
			}
			return null;
		}
		const lesson = visit(course);
		if (lesson) return { courseId: courseIdOf(course), lesson };
	}
	return null;
}
