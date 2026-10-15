import { describe, expect, it } from 'vitest';
import { findLesson, href, locationFromHash, routeFromHash } from './router';

describe('hash router', () => {
	it('defaults unknown hashes to login and supports the core routes', () => {
		expect(routeFromHash('')).toBe('login');
		expect(routeFromHash('#/catalog')).toBe('catalog');
		expect(routeFromHash('#/player/lesson')).toBe('player');
		expect(locationFromHash('#/player/modules/lesson%20one')).toEqual({ route: 'player', lessonPath: 'modules/lesson one' });
	});
	it('creates server-independent links and preserves the lesson path', () => {
		expect(href('home')).toBe('#/home');
		expect(href('player', 'modules/lesson one')).toBe('#/player/modules%2Flesson%20one');
	});
	it('resolves a deep-linked lesson after catalog loading', () => {
		const lesson = { title: 'Aula', path: 'modules/lesson one', media: [], subtitles: [], materials: [] };
		const catalog = { categories: [{ title: 'Categoria', courses: [{ title: 'Curso', modules: [{ title: 'Módulo', lessons: [lesson] }] }] }] };
		expect(findLesson(catalog, 'modules/lesson one')).toBe(lesson);
		expect(findLesson(catalog, 'missing')).toBeNull();
	});
});
