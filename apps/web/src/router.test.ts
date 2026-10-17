import { describe, expect, it } from 'vitest';
import { destinationAfterLogin, findCourse, findLesson, findLessonLocation, href, locationFromHash, routeFromHash } from './router';
import type { Catalog, CatalogNode, FileEntry } from './transport';

function course(id: string, children: CatalogNode[] = []): CatalogNode {
	return { title: `Curso ${id}`, path: '', role: 'course', courseId: id, files: [], looseFiles: [], children };
}

function lessonNode(path: string, mediaId?: string): CatalogNode {
	return {
		title: 'Aula',
		path,
		role: 'lesson',
		files: mediaId ? [{ name: 'video.mp4', path: `${path}.mp4`, size: 10, mediaId, kind: 'media' }] : [],
		looseFiles: [],
		children: []
	};
}

function catalog(nodes: CatalogNode[], looseFiles: FileEntry[] = []): Catalog {
	return { complete: true, looseFiles, nodes };
}

describe('hash router', () => {
	it('defaults unknown hashes to login and supports the core routes', () => {
		expect(routeFromHash('')).toBe('login');
		expect(routeFromHash('#/home')).toBe('home');
		expect(routeFromHash('#/course/course-1')).toBe('course');
		expect(routeFromHash('#/player/course-1/aula')).toBe('player');
		expect(locationFromHash('#/course/course%20one')).toEqual({ route: 'course', courseId: 'course one', lessonPath: null });
		expect(locationFromHash('#/player/course-1/modules/aula%20one')).toEqual({ route: 'player', courseId: 'course-1', lessonPath: 'modules/aula one' });
		expect(locationFromHash('#/player/%E0%A4%A/aula')).toEqual({ route: 'player', courseId: null, lessonPath: 'aula' });
	});

	it('creates server-independent links with the course identity', () => {
		expect(href('home')).toBe('#/home');
		expect(href('course', 'course one')).toBe('#/course/course%20one');
		expect(href('player', 'course-1', 'modules/aula one')).toBe('#/player/course-1/modules%2Faula%20one');
	});

	it('preserves a deep-link after login', () => {
		expect(destinationAfterLogin('#/course/course-1')).toBe('#/course/course-1');
		expect(destinationAfterLogin('#/player/course-1/aula')).toBe('#/player/course-1/aula');
	});

	it('drops an unknown hash to home after login', () => expect(destinationAfterLogin('#/desconhecido')).toBe('#/home'));
	it('drops a malformed percent-encoding to home after login', () => expect(destinationAfterLogin('#/player/%E0%A4%A/aula')).toBe('#/home'));

	it('resolves a deep-linked lesson inside the right course', () => {
		const data = catalog([course('course-1', [lessonNode('aula')]), course('course-2', [lessonNode('aula')])]);

		expect(findLesson(data, 'course-2', 'aula')?.path).toBe('aula');
		// O caminho relativo é o mesmo nos dois cursos: a identidade desempata.
		expect(findLesson(data, 'course-2', 'missing')).toBeNull();
		expect(findLesson(data, 'unknown', 'aula')).toBeNull();
	});

	it('clears a player lookup when a rescan removes the lesson', () => {
		const current = catalog([course('course-1')]);

		expect(findLesson(current, 'course-1', 'aula')).toBeNull();
	});

	it('finds the course node by identity, falling back to the legacy path', () => {
		const legacy: CatalogNode = { title: 'Curso legado', path: 'programming/go', role: 'category', files: [], looseFiles: [], children: [] };
		const data = catalog([course('course-1'), legacy]);

		expect(findCourse(data, 'course-1')).toBe(data.nodes[0]);
		expect(findCourse(data, 'programming/go')).toBe(legacy);
		expect(findCourse(data, 'missing')).toBeNull();
		expect(findCourse(null, 'course-1')).toBeNull();
	});

	it('resolves a lesson location from a media id across courses', () => {
		const data = catalog([
			course('course-1', [lessonNode('aula', 'm1')]),
			course('course-2', [lessonNode('aula', 'm2')])
		]);

		expect(findLessonLocation(data, 'm2')).toMatchObject({ courseId: 'course-2', lesson: { path: 'aula' } });
		expect(findLessonLocation(data, 'missing')).toBeNull();
		expect(findLessonLocation(null, 'm1')).toBeNull();
		expect(findLessonLocation(data, null)).toBeNull();
	});
	it('opens standalone root media in the player', () => {
		const root: FileEntry = { name: 'Áudio inicial.mp3', path: 'Áudio inicial.mp3', size: 10, kind: 'media', mediaId: 'root' };
		const data = catalog([], [root]);
		expect(locationFromHash(href('player', root.path)).lessonPath).toBe(root.path);
		expect(findLesson(data, null, root.path)).toMatchObject({ title: root.name, path: root.path, media: [root] });
	});
	it('does not resolve materials, missing files or removed paths as playable standalone media', () => {
		const data = catalog([], [
			{ name: 'aviso.pdf', path: 'aviso.pdf', size: 10, kind: 'material', mediaId: 'pdf' },
			{ name: 'ausente.mp4', path: 'ausente.mp4', size: 10, kind: 'media', mediaId: 'missing', availability: 'missing' }
		]);
		expect(findLesson(data, null, 'aviso.pdf')).toBeNull();
		expect(findLesson(data, null, 'ausente.mp4')).toBeNull();
		expect(findLesson(data, null, 'removido.mp4')).toBeNull();
	});
});
