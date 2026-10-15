import type { Catalog, Lesson } from './transport';

export type Route = 'login' | 'home' | 'catalog' | 'player';
export type HashLocation = { route: Route; lessonPath: string | null };

export function locationFromHash(hash: string): HashLocation {
	const value = hash.replace(/^#\/?/, '');
	const [name, ...parts] = value.split('/');
	const route = name === 'home' || name === 'catalog' || name === 'player' ? name : 'login';
	return { route, lessonPath: route === 'player' && parts.length > 0 ? decodeURIComponent(parts.join('/')) : null };
}

export function routeFromHash(hash: string): Route {
	return locationFromHash(hash).route;
}

export function href(route: Route, lessonPath?: string): string {
	return lessonPath === undefined ? `#/${route}` : `#/${route}/${encodeURIComponent(lessonPath)}`;
}

export function findLesson(catalog: Catalog | null, lessonPath: string | null): Lesson | null {
	if (!catalog || !lessonPath) return null;
	for (const category of catalog.categories) for (const course of category.courses) for (const module of course.modules) {
		const lesson = module.lessons.find((candidate) => candidate.path === lessonPath);
		if (lesson) return lesson;
	}
	return null;
}
