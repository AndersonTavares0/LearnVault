import { courseIdOf, type Catalog, type CatalogNode, type FileEntry, type Lesson } from './transport';

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
 * da rota. `#/player/<caminho>` é a forma legada de um arquivo solto sem curso dono;
 * o caminho deve começar a busca global. Qualquer outro valor cai no login.
 */
export function locationFromHash(hash: string): HashLocation {
	const value = hash.replace(/^#\/?/, '');
	const [name, ...parts] = value.split('/');
	const route = name === 'home' || name === 'course' || name === 'player' ? name : 'login';
	if (route === 'course') return { route, courseId: decodeSegment(parts[0]), lessonPath: null };
	if (route === 'player') {
		if (parts.length === 0) return { route, courseId: null, lessonPath: null };
		if (parts.length === 1) return { route, courseId: null, lessonPath: decodeSegment(parts[0]) };
		return { route, courseId: decodeSegment(parts[0]), lessonPath: decodeSegment(parts.slice(1).join('/')) };
	}
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
	if (route === 'player' && courseId !== undefined && lessonPath === undefined) {
		return `#/player/${encodeURIComponent(courseId)}`;
	}
	return `#/${route}`;
}

/** Destino seguro depois do login, preservando uma rota já navegável. */
export function destinationAfterLogin(hash: string): string {
	const location = locationFromHash(hash);
	if (location.route === 'home' || location.route === 'course') return href(location.route, location.courseId ?? undefined);
	if (location.route === 'player' && location.lessonPath) return href('player', location.courseId ?? undefined, location.lessonPath);
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
 * relativo, e a identidade do curso desempata. Sem curso (arquivo solto da raiz), a
 * busca é global com `media_id` e caminho exato como critério, e nunca devolve
 * materiais ausentes.
 */
export function findLesson(catalog: Catalog | null, courseId: string | null, lessonPath: string | null): Lesson | null {
	if (!catalog || !lessonPath) return null;
	const playable = (file: FileEntry) =>
		file.kind === 'media' && file.availability?.trim().toLowerCase() !== 'missing' && file.path === lessonPath;
	if (courseId) {
		const course = findCourse(catalog, courseId);
		if (!course) return null;
		function visit(node: CatalogNode): Lesson | null {
			if (node.role === 'lesson' && node.path === lessonPath) return lessonFromNode(node);
			const loose = node.looseFiles.find(playable);
			if (loose) return { title: loose.name, path: loose.path, media: [loose], subtitles: [], materials: [] };
			for (const child of node.children) {
				const found = visit(child);
				if (found) return found;
			}
			return null;
		}
		if (course.role === 'lesson' && course.path === lessonPath) return lessonFromNode(course);
		const ownLoose = course.looseFiles.find(playable);
		if (ownLoose) return { title: ownLoose.name, path: ownLoose.path, media: [ownLoose], subtitles: [], materials: [] };
		return visit(course);
	}
	return standaloneMedia(catalog, (file) => file.path === lessonPath);
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
	for (const course of catalog.nodes) {
		const loose = course.looseFiles.find(
			(file) => file.kind === 'media' && file.availability?.trim().toLowerCase() !== 'missing' && file.mediaId === mediaId
		);
		if (loose) return { courseId: courseIdOf(course), lesson: { title: loose.name, path: loose.path, media: [loose], subtitles: [], materials: [] } };
	}
	return null;
}

// Arquivos de mídia fora de uma aula usam o mesmo player, sem navegar para a URL crua.
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
