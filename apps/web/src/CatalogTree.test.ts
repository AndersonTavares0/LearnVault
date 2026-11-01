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
		expect(body).toContain('href="#/course/course-1/curso%2Faula-1"');
		expect(body).toContain('1 vídeo · 2 materiais');
	});

	it('opens a module only when expandAll is set, keeping the lessons inside it', () => {
		const module = node('module', 'Módulo', 'curso/modulo', [node('lesson', 'Aula', 'curso/modulo/aula', [], [file('a.mp4', 'media')])]);
		const closed = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1' } });
		expect(closed.body).toContain('course-section"');
		expect(closed.body).not.toMatch(/course-section"[^>]* open/);
		expect(closed.body).toContain('href="#/course/course-1/curso%2Fmodulo%2Faula"');
		const open = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1', expandAll: true } });
		expect(open.body).toMatch(/course-section"[^>]* open/);
	});

	it('keeps module materials downloadable inside its disclosure without creating player lessons', () => {
		const module = node('module', 'Conceitos', 'modulo', [node('lesson', 'Introdução', 'modulo/intro.mp4', [], [file('intro.mp4', 'media')])]);
		module.looseFiles = [file('exercicios.pdf', 'material')];
		const { body } = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1' } });
		expect(body).toContain('aria-label="Materiais complementares"');
		expect(body).toContain('download="exercicios.pdf"');
		expect(body).not.toContain('#/course/course-1/exercicios.pdf');
		expect(body.indexOf('download="exercicios.pdf"')).toBeLessThan(body.indexOf('</details>'));
	});

	it('hides module materials when the host renders them elsewhere', () => {
		const module = node('module', 'Conceitos', 'modulo', [node('lesson', 'Introdução', 'modulo/intro.mp4', [], [file('intro.mp4', 'media')])]);
		module.looseFiles = [file('exercicios.pdf', 'material')];
		const { body } = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1', showFiles: false } });
		expect(body).not.toContain('Materiais complementares');
		expect(body).not.toContain('exercicios.pdf');
	});

	it('keeps nested lessons of a lesson visible and unannotated from the parent meta', () => {
		const parent = node('lesson', 'Aula 01', 'curso/aula-01', [node('lesson', 'Bônus', 'curso/aula-01/bonus', [], [file('b.pdf', 'material')])], [file('a.mp4', 'media')]);
		const { body } = render(CatalogTree, { props: { nodes: [parent], courseId: 'course-1' } });
		expect(body).toContain('href="#/course/course-1/curso%2Faula-01%2Fbonus"');
		expect(body).toContain('<span class="lesson-meta">1 vídeo</span>');
	});

	it('renders course meta and loose files as list rows instead of inline links', () => {
		const category = node('category', 'Curso', 'curso', [node('lesson', 'Aula', 'curso/aula', [], [file('a.mp4', 'media'), file('a.pdf', 'material')])], [file('aviso.pdf', 'material')]);
		const { body } = render(CatalogTree, { props: { nodes: [category], courseId: 'course-1' } });
		expect(body).toContain('1 aula · 2 materiais');
		expect(body).toContain('<li>');
	});

	it('keeps missing lessons static and without a player link', () => {
		const lesson = node('lesson', 'Aula 1', 'curso/aula-1', [], [file('a.mp4', 'media', 'missing')]);
		const { body } = render(CatalogTree, { props: { nodes: [lesson], courseId: 'course-1' } });
		expect(body).toContain('<span class="lesson-title">Aula 1</span>');
		expect(body).toContain('<div class="lesson-row">');
		expect(body).not.toContain('<a ');
		expect(body).not.toContain('#/course/course-1/curso%2Faula-1');
	});

	it('keeps an available lesson as a real player link', () => {
		const lesson = node('lesson', 'Aula 1', 'curso/aula-1', [], [file('a.mp4', 'media')]);
		const { body } = render(CatalogTree, { props: { nodes: [lesson], courseId: 'course-1' } });
		expect(body).toContain('href="#/course/course-1/curso%2Faula-1"');
	});

	it('reports available and missing lessons side by side in the module count', () => {
		const module = node('module', 'Módulo', 'curso/modulo', [
			node('lesson', 'Aula 1', 'curso/modulo/aula-1', [], [file('a.mp4', 'media')]),
			node('lesson', 'Aula 2', 'curso/modulo/aula-2', [], [file('b.mp4', 'media', 'missing')]),
			node('lesson', 'Aula 3', 'curso/modulo/aula-3', [], [file('c.mp4', 'media', 'missing'), file('c.srt', 'subtitle')])
		]);
		const { body } = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1' } });
		expect(body).toContain('<span class="section-meta">1 aula · 2 ausentes</span>');
		// Só a aula disponível continua roteável.
		expect(body.match(/href="#\/course/g)).toHaveLength(1);
	});

	it('reports missing lessons in the course meta of a category header', () => {
		const category = node('category', 'Curso', 'curso', [node('lesson', 'Aula', 'curso/aula', [], [file('a.mp4', 'media', 'missing')])]);
		const { body } = render(CatalogTree, { props: { nodes: [category], courseId: 'course-1' } });
		expect(body).toContain('<p class="course-meta">0 aulas · 1 ausente</p>');
	});

	it('keeps the count readable when every lesson of the module is missing', () => {
		const module = node('module', 'Módulo', 'curso/modulo', [node('lesson', 'Aula 1', 'curso/modulo/aula-1', [], [file('a.mp4', 'media', 'missing')])]);
		const { body } = render(CatalogTree, { props: { nodes: [module], courseId: 'course-1' } });
		expect(body).toContain('<span class="section-meta">0 aulas · 1 ausente</span>');
		// A linha continua listada, agora sem destino de player.
		expect(body).not.toContain('href="#/course');
	});

	it('highlights the active lesson and opens only the module that contains it', () => {
		const first = node('module', 'Módulo 1', 'mod-1', [
			node('lesson', 'Aula 1', 'mod-1/aula-1', [], [file('a.mp4', 'media')]),
			node('lesson', 'Aula 2', 'mod-1/aula-2', [], [file('b.mp4', 'media')])
		]);
		const second = node('module', 'Módulo 2', 'mod-2', [node('lesson', 'Aula 3', 'mod-2/aula-3', [], [file('c.mp4', 'media')])]);
		const { body } = render(CatalogTree, { props: { nodes: [first, second], courseId: 'course-1', activePath: 'mod-1/aula-2' } });

		expect(body).toMatch(/class="[^"]*\bactive\b[^"]*"[^>]*href="#\/course\/course-1\/mod-1%2Faula-2"[^>]*aria-current="page"/);
		expect(body.match(/course-section"[^>]* open/g)).toHaveLength(1);
		expect(body).toContain('href="#/course/course-1/mod-1%2Faula-1"');
	});

	it('renders quick completion toggle button when handler is supplied and reflects completed state', () => {
		const lesson = node('lesson', 'Aula Concluível', 'curso/aula-1', [], [file('a.mp4', 'media')]);
		const uncompleted = render(CatalogTree, {
			props: {
				nodes: [lesson],
				courseId: 'course-1',
				completedMediaIds: new Set<string>(),
				onToggleLessonCompleted: () => undefined,
			}
		});
		expect(uncompleted.body).toContain('lesson-check-btn');
		expect(uncompleted.body).toContain('aria-label="Marcar &quot;Aula Concluível&quot; como concluída"');
		expect(uncompleted.body).not.toContain('is-completed');

		const completed = render(CatalogTree, {
			props: {
				nodes: [lesson],
				courseId: 'course-1',
				completedMediaIds: new Set(['a.mp4']),
				onToggleLessonCompleted: () => undefined,
			}
		});
		expect(completed.body).toContain('lesson-check-btn is-completed');
		expect(completed.body).toContain('aria-label="Marcar &quot;Aula Concluível&quot; como não concluída"');
		expect(completed.body).toContain('check-filled');
	});

	it('does not render completion toggle button for non-media lessons', () => {
		const readingLesson = node('lesson', 'Leitura', 'curso/slides', [], [file('slides.pdf', 'material')]);
		const { body } = render(CatalogTree, {
			props: {
				nodes: [readingLesson],
				courseId: 'course-1',
				completedMediaIds: new Set<string>(),
				onToggleLessonCompleted: () => undefined,
			}
		});
		expect(body).not.toContain('lesson-check-btn');
	});
});
