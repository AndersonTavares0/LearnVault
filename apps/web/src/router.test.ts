import { describe, expect, it } from 'vitest';
import { destinationAfterLogin, cleanMediaTitle, findCourse, findLesson, findLessonLocation, href, locationFromHash, routeFromHash } from './router';
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

function moduleNode(title: string, path: string, children: CatalogNode[]): CatalogNode {
	return { title, path, role: 'module', files: [], looseFiles: [], children };
}

function looseMedia(mediaId: string, name: string): FileEntry {
	return { name, path: name, size: 10, mediaId, kind: 'media' };
}

describe('hash router', () => {
	it('defaults unknown hashes to login and supports the core routes', () => {
		expect(routeFromHash('')).toBe('login');
		expect(routeFromHash('#/home')).toBe('home');
		expect(routeFromHash('#/course/course-1')).toBe('course');
		expect(routeFromHash('#/player/course-1/aula')).toBe('player');
		expect(locationFromHash('#/course/course%20one')).toEqual({ route: 'course', courseId: 'course one', lessonPath: null });
		expect(locationFromHash('#/course/course-1/modules/aula%20one')).toEqual({ route: 'course', courseId: 'course-1', lessonPath: 'modules/aula one' });
		// Barra final não inventa uma aula de caminho vazio.
		expect(locationFromHash('#/course/course-1/')).toEqual({ route: 'course', courseId: 'course-1', lessonPath: null });
		expect(locationFromHash('#/player/course-1/modules/aula%20one')).toEqual({ route: 'player', courseId: 'course-1', lessonPath: 'modules/aula one' });
		expect(locationFromHash('#/player/%E0%A4%A/aula')).toEqual({ route: 'player', courseId: null, lessonPath: 'aula' });
	});

	it('defaults unknown and login hashes to the fallback route of the build', () => {
		// O desktop não tem sessão: login e endereços desconhecidos caem na tela inicial.
		expect(routeFromHash('', 'home')).toBe('home');
		expect(routeFromHash('#/login', 'home')).toBe('home');
		expect(routeFromHash('#/desconhecido', 'home')).toBe('home');
		expect(locationFromHash('#/login', 'home')).toEqual({ route: 'home', courseId: null, lessonPath: null });
		// Rotas nomeadas continuam valendo independentemente do padrão do build.
		expect(routeFromHash('#/settings', 'home')).toBe('settings');
		expect(routeFromHash('#/notes', 'home')).toBe('notes');
		expect(routeFromHash('#/analytics', 'home')).toBe('analytics');
	});

	it('supports settings as an authenticated destination', () => {
		expect(locationFromHash('#/settings')).toEqual({ route: 'settings', courseId: null, lessonPath: null });
		expect(destinationAfterLogin('#/settings')).toBe('#/settings');
	});
	it('creates server-independent links with the course identity', () => {
		expect(href('home')).toBe('#/home');
		expect(href('course', 'course one')).toBe('#/course/course%20one');
		expect(href('course', 'course one', 'modules/aula one')).toBe('#/course/course%20one/modules%2Faula%20one');
		expect(href('course', 'course one', '')).toBe('#/course/course%20one');
		expect(href('player', 'course-1', 'modules/aula one')).toBe('#/player/course-1/modules%2Faula%20one');
	});

	it('preserves a deep-link after login', () => {
		expect(destinationAfterLogin('#/course/course-1')).toBe('#/course/course-1');
		expect(destinationAfterLogin('#/course/course-1/aula')).toBe('#/course/course-1/aula');
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

	// Portado do PR 121 (nós canônicos recursivos): o roteador é a única porta de entrada de
	// uma aula, então um `kind` desconhecido não pode ficar inalcançável. A estrutura tem
	// níveis aninhados (módulo dentro do curso) como na árvore canônica.
	it('keeps an unknown-kind file reachable through open/deep-link and media id', () => {
		const files: FileEntry[] = [
			{ name: 'video.mp4', path: 'curso/mod/aula.mp4', size: 10, mediaId: 'm1', kind: 'media' },
			{ name: 'extra.bin', path: 'curso/mod/extra.bin', size: 2, mediaId: 'bonus-1', kind: 'bonus' }
		];
		const module: CatalogNode = { title: 'Módulo', path: 'curso/mod', role: 'module', files: [], looseFiles: [], children: [{ title: 'Aula', path: 'curso/mod/aula', role: 'lesson', files, looseFiles: [], children: [] }] };
		const data = catalog([course('course-1', [module])]);

		const location = locationFromHash(href('player', 'course-1', 'curso/mod/aula'));
		const opened = findLesson(data, location.courseId, location.lessonPath);
		expect(location).toEqual({ route: 'player', courseId: 'course-1', lessonPath: 'curso/mod/aula' });
		expect(opened?.media.map((file) => file.path)).toEqual(['curso/mod/aula.mp4']);
		expect(opened?.materials.map((file) => file.path)).toEqual(['curso/mod/extra.bin']);
		expect(findLessonLocation(data, 'bonus-1')).toMatchObject({ courseId: 'course-1', lesson: { path: 'curso/mod/aula' } });
	});

	it('opens a nested loose media in the player and restores it by id without a course', () => {
		const nested: FileEntry = { name: 'extra.mp4', path: 'curso/extra.mp4', size: 10, kind: 'media', mediaId: 'nested' };
		const data = catalog([{ title: 'Curso course-1', path: '', role: 'course', courseId: 'course-1', files: [], looseFiles: [nested], children: [] }]);

		expect(locationFromHash(href('player', nested.path)).lessonPath).toBe(nested.path);
		expect(findLesson(data, null, nested.path)).toMatchObject({ title: nested.name, path: nested.path, media: [nested] });
		expect(findLessonLocation(data, 'nested')).toMatchObject({ courseId: 'course-1', lesson: { path: nested.path } });
	});

	it('strips ordering prefixes and extensions from a loose media label', () => {
		expect(cleanMediaTitle('01 - Hierarquia visual.mp4')).toBe('Hierarquia visual');
		expect(cleanMediaTitle('02_Introdução.mkv')).toBe('Introdução');
		expect(cleanMediaTitle('aula.mp4')).toBe('aula');
		// Um nome que ficaria vazio após a limpeza cai no próprio nome, nunca num rótulo vazio.
		expect(cleanMediaTitle('01 - .mp4')).toBe('01 - .mp4');
	});

	it('cleans the raw file name of loose course-root media in the recent label', () => {
		const loose = looseMedia('root-media', '01 - Hierarquia visual.mp4');
		const data = catalog([{ title: 'Curso Curto de Interface', path: '', role: 'course', courseId: 'cc', files: [], looseFiles: [loose], children: [] }]);

		const location = findLessonLocation(data, 'root-media');

		expect(location).toMatchObject({
			courseId: 'cc',
			courseTitle: 'Curso Curto de Interface',
			moduleTitle: null,
			lesson: { title: 'Hierarquia visual' }
		});
		expect(location?.lesson.title).not.toContain('.mp4');
		expect(location?.lesson.title).not.toMatch(/^\d/);
	});

	it('resolves the module context for a lesson inside a module', () => {
		const data = catalog([course('course-1', [moduleNode('Módulo 1', 'mod', [lessonNode('mod/aula', 'm1')])])]);

		expect(findLessonLocation(data, 'm1')).toMatchObject({
			courseId: 'course-1',
			courseTitle: 'Curso course-1',
			moduleTitle: 'Módulo 1',
			lesson: { path: 'mod/aula' }
		});
	});

	it('uses the nearest grouping as the module context and none directly under the course', () => {
		// O primeiro nível sob o curso é servido com papel `category`; ainda é o módulo visível.
		const category: CatalogNode = { title: 'Modulo 1', path: 'Modulo 1', role: 'category', files: [], looseFiles: [], children: [lessonNode('Modulo 1/aula', 'm1')] };
		const data = catalog([course('course-1', [category]), course('course-2', [lessonNode('aula', 'm2')])]);

		expect(findLessonLocation(data, 'm1')).toMatchObject({ courseId: 'course-1', moduleTitle: 'Modulo 1' });
		expect(findLessonLocation(data, 'm2')).toMatchObject({ courseId: 'course-2', moduleTitle: null });
	});

	it('keeps an unknown media id in the not-found state', () => {
		const data = catalog([course('course-1', [moduleNode('Módulo 1', 'mod', [lessonNode('mod/aula', 'm1')])])]);

		expect(findLessonLocation(data, 'ausente')).toBeNull();
	});
});
