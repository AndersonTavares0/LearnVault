import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import CatalogTree from './CatalogTree.svelte';
import type { CatalogNode, FileEntry } from './transport';

function file(name: string, kind: string, availability = 'available'): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind, availability };
}

function node(role: string, title: string, path: string, children: CatalogNode[] = [], files: FileEntry[] = []): CatalogNode {
	return { title, path, role, files, looseFiles: [], children };
}

describe('catalog tree rendering', () => {
	it('renders a lesson as a real link to the player inside its course with counts', () => {
		const lesson = node('lesson', 'Aula 1', 'curso/aula-1', [], [file('a.mp4', 'media'), file('b.pdf', 'material'), file('b.srt', 'subtitle')]);
		const { body } = render(CatalogTree, { props: { nodes: [lesson], courseId: 'course-1' } });
		expect(body).toContain('href="#/player/course-1/curso%2Faula-1"');
		expect(body).toContain('1 vídeo · 2 materiais');
	});

	it('opens a module only when expandAll is set, keeping the lessons inside it', () => {
		const module = node('module', 'Módulo', 'curso/modulo', [node('lesson', 'Aula', 'curso/modulo/aula', [], [file('a.mp4', 'media')])]);
		const closed = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1' } });
		expect(closed.body).toContain('course-section"');
		expect(closed.body).not.toMatch(/course-section"[^>]* open/);
		expect(closed.body).toContain('href="#/player/course-1/curso%2Fmodulo%2Faula"');
		const open = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1', expandAll: true } });
		expect(open.body).toMatch(/course-section"[^>]* open/);
	});

	it('keeps module materials downloadable inside its disclosure without creating player lessons', () => {
		const module = node('module', 'Conceitos', 'modulo', [node('lesson', 'Introdução', 'modulo/intro.mp4', [], [file('intro.mp4', 'media')])]);
		module.looseFiles = [file('exercicios.pdf', 'material')];
		const { body } = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1' } });
		expect(body).toContain('aria-label="Materiais complementares"');
		expect(body).toContain('download="exercicios.pdf"');
		expect(body).not.toContain('#/player/course-1/exercicios.pdf');
		expect(body.indexOf('download="exercicios.pdf"')).toBeLessThan(body.indexOf('</details>'));
	});

	it('keeps nested lessons of a lesson visible and unannotated from the parent meta', () => {
		const parent = node('lesson', 'Aula 01', 'curso/aula-01', [node('lesson', 'Bônus', 'curso/aula-01/bonus', [], [file('b.pdf', 'material')])], [file('a.mp4', 'media')]);
		const { body } = render(CatalogTree, { props: { nodes: [parent], courseId: 'course-1' } });
		expect(body).toContain('href="#/player/course-1/curso%2Faula-01%2Fbonus"');
		expect(body).toContain('<span class="small">1 vídeo</span>');
	});

	it('renders course meta and loose files as list rows instead of inline links', () => {
		const category = node('category', 'Curso', 'curso', [node('lesson', 'Aula', 'curso/aula', [], [file('a.mp4', 'media'), file('a.pdf', 'material')])], [file('aviso.pdf', 'material')]);
		const { body } = render(CatalogTree, { props: { nodes: [category], courseId: 'course-1' } });
		expect(body).toContain('1 aula · 2 materiais');
		expect(body).toContain('<li>');
	});

	it('renders a missing lesson as static content with the badge and without a player link', () => {
		const lesson = node('lesson', 'Aula 1', 'curso/aula-1', [], [file('a.mp4', 'media', 'missing')]);
		const { body } = render(CatalogTree, { props: { nodes: [lesson], courseId: 'course-1' } });
		// A aula continua listada, com título e selo como texto real do conteúdo (legível por
		// leitor de tela), mas sem `href`: um elemento sem link não recebe foco nem abre o player.
		expect(body).toContain('Aula 1');
		expect(body).toContain('<small class="small">(ausente)');
		expect(body).toContain('<div class="lesson missing">');
		expect(body).not.toContain('<a ');
		expect(body).not.toContain('#/player/course-1/curso%2Faula-1');
	});

	it('keeps an available lesson as a real link without any badge text', () => {
		const lesson = node('lesson', 'Aula 1', 'curso/aula-1', [], [file('a.mp4', 'media')]);
		const { body } = render(CatalogTree, { props: { nodes: [lesson], courseId: 'course-1' } });
		expect(body).toContain('href="#/player/course-1/curso%2Faula-1"');
		expect(body).not.toContain('ausente');
	});

	it('reports available and missing lessons side by side in the module count', () => {
		const module = node('module', 'Módulo', 'curso/modulo', [
			node('lesson', 'Aula 1', 'curso/modulo/aula-1', [], [file('a.mp4', 'media')]),
			node('lesson', 'Aula 2', 'curso/modulo/aula-2', [], [file('b.mp4', 'media', 'missing')]),
			node('lesson', 'Aula 3', 'curso/modulo/aula-3', [], [file('c.mp4', 'media', 'missing'), file('c.srt', 'subtitle')])
		]);
		const { body } = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1' } });
		expect(body).toContain('<span class="small">1 aula · 2 ausentes</span>');
		expect(body.match(/\(ausente\)/g)).toHaveLength(2);
		// Só a aula disponível continua roteável.
		expect(body.match(/href="#\/player/g)).toHaveLength(1);
	});

	it('reports missing lessons in the course meta of a category header', () => {
		const category = node('category', 'Curso', 'curso', [node('lesson', 'Aula', 'curso/aula', [], [file('a.mp4', 'media', 'missing')])]);
		const { body } = render(CatalogTree, { props: { nodes: [category], courseId: 'course-1' } });
		expect(body).toContain('<p class="course-meta">0 aulas · 1 ausente</p>');
	});

	it('keeps the count readable when every lesson of the module is missing', () => {
		const module = node('module', 'Módulo', 'curso/modulo', [node('lesson', 'Aula 1', 'curso/modulo/aula-1', [], [file('a.mp4', 'media', 'missing')])]);
		const { body } = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1' } });
		expect(body).toContain('<span class="small">0 aulas · 1 ausente</span>');
		expect(body).toContain('(ausente)');
		// A linha continua listada, agora sem destino de player.
		expect(body).not.toContain('href="#/player');
	});

	it('shows accessible sibling controls only in organize mode and offers reset once per list', () => {
		const lessons = [node('lesson', 'Aula 1', 'aula-1', [], [file('a.mp4', 'media')]), node('lesson', 'Aula 2', 'aula-2', [], [file('b.mp4', 'media')])];
		const normal = render(CatalogTree, { props: { nodes: lessons, courseId: 'course-1' } });
		expect(normal.body).not.toContain('Mover Aula 1 para cima');

		const organized = render(CatalogTree, { props: { nodes: lessons, courseId: 'course-1', organizing: true } });
		expect(organized.body).toContain('aria-label="Mover Aula 1 para cima" disabled');
		expect(organized.body).toContain('aria-label="Mover Aula 1 para baixo"');
		expect(organized.body).toContain('aria-label="Mover Aula 2 para cima"');
		expect(organized.body).toContain('aria-label="Mover Aula 2 para baixo" disabled');
		expect(organized.body.match(/Restaurar ordem automática/g)).toHaveLength(1);
	});

	it('provides controls for actual nested lesson siblings, not isolated recursive nodes', () => {
		const module = node('module', 'Módulo', 'modulo', [
			node('lesson', 'Aula 1', 'modulo/aula-1', [], [file('a.mp4', 'media')]),
			node('lesson', 'Aula 2', 'modulo/aula-2', [], [file('b.mp4', 'media')])
		]);
		const { body } = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1', organizing: true } });
		expect(body.match(/Mover Aula [12] para (cima|baixo)/g)).toHaveLength(4);
		expect(body.match(/Restaurar ordem automática/g)).toHaveLength(1);
	});
});
