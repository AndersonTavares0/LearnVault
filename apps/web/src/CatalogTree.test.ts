import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import CatalogTree from './CatalogTree.svelte';
import type { CatalogNode, FileEntry } from './transport';

function file(name: string, kind: string): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind, availability: 'available' };
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

	it('keeps nested lessons of a lesson visible and unannotated from the parent meta', () => {
		const parent = node('lesson', 'Aula 01', 'curso/aula-01', [node('lesson', 'Bônus', 'curso/aula-01/bonus', [], [file('b.pdf', 'material')])], [file('a.mp4', 'media')]);
		const { body } = render(CatalogTree, { props: { nodes: [parent], courseId: 'course-1' } });
		expect(body).toContain('href="#/player/course-1/curso%2Faula-01%2Fbonus"');
		expect(body).toContain('<span class="lesson-meta">1 vídeo</span>');
	});

	it('renders course meta and loose files as list rows instead of inline links', () => {
		const category = node('category', 'Curso', 'curso', [node('lesson', 'Aula', 'curso/aula', [], [file('a.mp4', 'media'), file('a.pdf', 'material')])], [file('aviso.pdf', 'material')]);
		const { body } = render(CatalogTree, { props: { nodes: [category], courseId: 'course-1' } });
		expect(body).toContain('1 aula · 2 materiais');
		expect(body).toContain('<li>');
	});
});
