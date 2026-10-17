import type { Catalog, FileEntry, Lesson } from './transport';

export type Route = 'login' | 'home' | 'catalog' | 'player' | 'settings';
export type HashLocation = { route: Route; lessonPath: string | null };

export function locationFromHash(hash: string): HashLocation {
	const value = hash.replace(/^#\/?/, '');
	const [name, ...parts] = value.split('/');
	const route = name === 'home' || name === 'catalog' || name === 'player' || name === 'settings' ? name : 'login';
	if (route !== 'player' || parts.length === 0) return { route, lessonPath: null };
	try {
		return { route, lessonPath: decodeURIComponent(parts.join('/')) };
	} catch {
		return { route, lessonPath: null };
	}
}

export function routeFromHash(hash: string): Route {
	return locationFromHash(hash).route;
}

export function href(route: Route, lessonPath?: string): string {
	return lessonPath === undefined ? `#/${route}` : `#/${route}/${encodeURIComponent(lessonPath)}`;
}

export function destinationAfterLogin(hash: string): string {
	const location = locationFromHash(hash);
	if (location.route === 'home' || location.route === 'catalog' || location.route === 'settings') return href(location.route);
	if (location.route === 'player' && location.lessonPath) return href('player', location.lessonPath);
	return href('home');
}

export function findLesson(catalog: Catalog | null, lessonPath: string | null): Lesson | null {
	if (!catalog || !lessonPath) return null;
	function visit(nodes: Catalog['nodes']): Lesson | null {
		for (const node of nodes) {
			if (node.role === 'lesson' && node.path === lessonPath) {
				const files = [...node.files, ...node.looseFiles];
				return { title: node.title, path: node.path, media: files.filter((file) => file.kind === 'media'), subtitles: files.filter((file) => file.kind === 'subtitle'), materials: files.filter((file) => file.kind === 'material') };
			}
			const found = visit(node.children);
			if (found) return found;
		}
		return null;
	}
	const recursive = visit(catalog.nodes);
	if (recursive) return recursive;
	for (const category of catalog.categories) for (const course of category.courses) for (const module of course.modules) {
		const lesson = module.lessons.find((candidate) => candidate.path === lessonPath);
		if (lesson) return lesson;
	}
	return standaloneMedia(catalog, (file) => file.path === lessonPath);
}

export function findLessonByMediaId(catalog: Catalog | null, mediaId: string | null): Lesson | null {
	if (!catalog || !mediaId) return null;
	function visit(nodes: Catalog['nodes']): Lesson | null {
		for (const node of nodes) {
			if (node.role === 'lesson') {
				const files = [...node.files, ...node.looseFiles];
				if (files.some((file) => file.mediaId === mediaId)) {
					return { title: node.title, path: node.path, media: files.filter((file) => file.kind === 'media'), subtitles: files.filter((file) => file.kind === 'subtitle'), materials: files.filter((file) => file.kind === 'material') };
				}
			}
			const found = visit(node.children);
			if (found) return found;
		}
		return null;
	}
	const recursive = visit(catalog.nodes);
	if (recursive) return recursive;
	for (const category of catalog.categories) for (const course of category.courses) for (const module of course.modules) {
		const lesson = module.lessons.find((item) => item.media.some((file) => file.mediaId === mediaId));
		if (lesson) return lesson;
	}
	return standaloneMedia(catalog, (file) => file.mediaId === mediaId);
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
