import { courseIdOf, lessonFromNode, type Catalog, type CatalogNode, type FileEntry, type Lesson } from './transport';

export type Route = 'login' | 'home' | 'course' | 'player' | 'settings' | 'statistics' | 'notes';
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
 * Interpreta o hash da localização.
 *
 * `#/course/<id>` abre um curso e `#/course/<id>/<caminho>` abre a aula indicada dentro
 * dele: o caminho relativo da aula só é único por curso, então a identidade do curso faz
 * parte da rota. `#/player/<id>/<caminho>` é a forma legada da mesma tela e continua
 * aceita. `#/settings` abre as preferências locais, `#/statistics` o resumo do
 * estudo e `#/notes` o painel global de anotações; nenhum deles identifica curso.
 * Parâmetro opcional `?t=<segundos>` posiciona o player diretamente no timestamp da anotação.
 * `#/player/<caminho>` é a forma legada de um arquivo solto sem curso dono;
 * o caminho deve começar a busca global. Qualquer outro valor cai na rota padrão do build:
 * o login no servidor e a tela inicial no desktop, que não tem sessão.
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
	const route = name === 'home' || name === 'course' || name === 'player' || name === 'settings' || name === 'statistics' || name === 'notes' ? name : fallback;
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

/** Monta o hash canônico de uma rota. */
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

/** Destino seguro depois do login, preservando uma rota já navegável. */
export function destinationAfterLogin(hash: string): string {
	const location = locationFromHash(hash);
	if (location.route === 'home' || location.route === 'course' || location.route === 'settings' || location.route === 'statistics' || location.route === 'notes') return href(location.route, location.courseId ?? undefined, location.lessonPath ?? undefined);
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

/** Curso de topo do catálogo pela identidade opaca (ou pelo caminho no formato legado). */
export function findCourse(catalog: Catalog | null, courseId: string | null): CatalogNode | null {
	if (!catalog || !courseId) return null;
	return catalog.nodes.find((node) => courseIdOf(node) === courseId) ?? null;
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

/** Localização de uma aula pelo `media_id`: o curso, o módulo (quando houver) e a aula. */
export type LessonLocation = {
	courseId: string;
	courseTitle: string;
	moduleTitle: string | null;
	lesson: Lesson;
};

// Mídia tocável: um arquivo de mídia que a última varredura não marcou como ausente.
function playableMedia(file: FileEntry): boolean {
	return file.kind === 'media' && file.availability?.trim().toLowerCase() !== 'missing';
}

// Agrupador visível de aulas na árvore canônica: módulo ou categoria. O primeiro nível sob o
// curso é servido com papel `category`, então ele também conta como contexto de módulo.
function groupsLessons(node: CatalogNode): boolean {
	return node.role === 'module' || node.role === 'category';
}

/**
 * Título apresentado de uma mídia solta: sem prefixo numérico nem extensão.
 *
 * Repete a limpeza que o catálogo aplica ao título de uma aula derivada de arquivo, para que
 * a rede de segurança do cliente nunca mostre o nome cru do arquivo na tela inicial.
 */
export function cleanMediaTitle(name: string): string {
	const lastDot = name.lastIndexOf('.');
	const stem = lastDot > 0 ? name.slice(0, lastDot) : name;
	const cleaned = stem.replace(/^[0-9._\- •\t]+/, '').trim();
	return cleaned || name;
}

/**
 * Localização de uma aula pelo `media_id`: o curso, o módulo (quando houver) e a aula.
 *
 * O curso é o nó de topo que contém a mídia. O módulo é o agrupador mais próximo da aula —
 * módulo ou categoria — e é `null` quando a aula está diretamente sob o curso. Mídia solta na
 * raiz do curso continua sendo um item avulso (o catálogo a mantém em `loose_files`), resolvida
 * com o título limpo e sem módulo.
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
		// A própria raiz do curso nunca conta como módulo: só o agrupamento abaixo dela.
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
