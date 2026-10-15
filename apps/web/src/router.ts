import type { Catalog, Lesson } from './transport';

export type Route = 'login' | 'home' | 'catalog' | 'player';
export type HashLocation = { route: Route; lessonPath: string | null };

export function locationFromHash(hash: string): HashLocation {
	const value = hash.replace(/^#\/?/, '');
	const [name, ...parts] = value.split('/');
	const route = name === 'home' || name === 'catalog' || name === 'player' ? name : 'login';
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
	return hash || href('home');
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
	return null;
}
