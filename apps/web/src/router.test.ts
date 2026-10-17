import { describe, expect, it } from 'vitest';
import { destinationAfterLogin, findLesson, findLessonByMediaId, href, locationFromHash, routeFromHash } from './router';
import type { Catalog, FileEntry } from './transport';

describe('hash router', () => {
	it('defaults unknown hashes to login and supports the core routes', () => {
		expect(routeFromHash('')).toBe('login');
		expect(routeFromHash('#/catalog')).toBe('catalog');
		expect(routeFromHash('#/player/lesson')).toBe('player');
		expect(locationFromHash('#/player/modules/lesson%20one')).toEqual({ route: 'player', lessonPath: 'modules/lesson one' });
		expect(locationFromHash('#/player/%E0%A4%A')).toEqual({ route: 'player', lessonPath: null });
	});
	it('creates server-independent links and preserves the lesson path', () => {
		expect(href('home')).toBe('#/home');
		expect(href('player', 'modules/lesson one')).toBe('#/player/modules%2Flesson%20one');
	});
	it('preserves a deep-link after login', () => expect(destinationAfterLogin('#/player/aula')).toBe('#/player/aula'));
	it('drops an unknown hash to home after login', () => expect(destinationAfterLogin('#/desconhecido')).toBe('#/home'));
	it('drops a malformed percent-encoding to home after login', () => expect(destinationAfterLogin('#/player/%E0%A4%A')).toBe('#/home'));
	it('resolves a deep-linked lesson after catalog loading', () => {
		const lesson = { title: 'Aula', path: 'modules/lesson one', media: [], subtitles: [], materials: [] };
		const catalog = { complete: true, looseFiles: [], nodes: [], categories: [{ title: 'Categoria', looseFiles: [], courses: [{ title: 'Curso', looseFiles: [], modules: [{ title: 'Módulo', looseFiles: [], lessons: [lesson] }] }] }] };
		expect(findLesson(catalog, 'modules/lesson one')).toBe(lesson);
		expect(findLesson(catalog, 'missing')).toBeNull();
	});
	it('clears a player lookup when a rescan removes the lesson', () => {
		const oldLesson = { title: 'Antiga', path: 'aula', media: [], subtitles: [], materials: [] };
		const current = { complete: true, looseFiles: [], nodes: [], categories: [{ title: 'Categoria', looseFiles: [], courses: [{ title: 'Curso', looseFiles: [], modules: [{ title: 'Módulo', looseFiles: [], lessons: [] }] }] }] };
		expect(findLesson(current, oldLesson.path)).toBeNull();
	});
	it('resolves a lesson from a media id in both canonical and projected trees', () => {
		const lesson = { title: 'Aula', path: 'cat/curso/mod/aula', media: [{ name: 'video.mp4', path: 'cat/curso/mod/aula.mp4', size: 10, mediaId: 'm1', kind: 'media' }], subtitles: [], materials: [] };
		const catalog = { complete: true, looseFiles: [], nodes: [{ title: 'Categoria', path: 'cat', role: 'category', files: [], looseFiles: [], children: [{ title: 'Curso', path: 'cat/curso', role: 'course', files: [], looseFiles: [], children: [{ title: 'Módulo', path: 'cat/curso/mod', role: 'module', files: [], looseFiles: [], children: [{ title: 'Aula', path: 'cat/curso/mod/aula', role: 'lesson', files: lesson.media, looseFiles: [], children: [] }] }] }] }], categories: [{ title: 'Categoria', looseFiles: [], courses: [{ title: 'Curso', looseFiles: [], modules: [{ title: 'Módulo', looseFiles: [], lessons: [lesson] }] }] }] };
		expect(findLessonByMediaId(catalog, 'm1')?.path).toBe('cat/curso/mod/aula');
		expect(findLessonByMediaId(catalog, 'missing')).toBeNull();
		expect(findLessonByMediaId(null, 'm1')).toBeNull();
	});
	it('opens standalone root and nested media in the player and restores recent media by id', () => {
		const root: FileEntry = { name: 'Áudio inicial.mp3', path: 'Áudio inicial.mp3', size: 10, kind: 'media', mediaId: 'root' };
		const nested: FileEntry = { name: 'extra.mp4', path: 'curso/extra.mp4', size: 10, kind: 'media', mediaId: 'nested' };
		const catalog: Catalog = { complete: true, categories: [], looseFiles: [root], nodes: [{ title: 'Curso', role: 'course', path: 'curso', files: [], looseFiles: [nested], children: [] }] };
		for (const file of [root, nested]) {
			const link = href('player', file.path);
			const lesson = findLesson(catalog, locationFromHash(link).lessonPath);
			expect(lesson).toMatchObject({ title: file.name, path: file.path, media: [file] });
			expect(findLessonByMediaId(catalog, file.mediaId!)?.path).toBe(file.path);
		}
	});
	it('does not resolve materials, missing files or removed paths as playable standalone media', () => {
		const catalog: Catalog = { complete: true, categories: [], nodes: [], looseFiles: [
			{ name: 'aviso.pdf', path: 'aviso.pdf', size: 10, kind: 'material', mediaId: 'pdf' },
			{ name: 'ausente.mp4', path: 'ausente.mp4', size: 10, kind: 'media', mediaId: 'missing', availability: 'missing' }
		] };
		expect(findLesson(catalog, 'aviso.pdf')).toBeNull();
		expect(findLesson(catalog, 'ausente.mp4')).toBeNull();
		expect(findLesson(catalog, 'removido.mp4')).toBeNull();
		expect(findLessonByMediaId(catalog, 'missing')).toBeNull();
	});
});
