import { describe, expect, it } from 'vitest';
import { findLesson, href, locationFromHash, routeFromHash } from './router';

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
});
