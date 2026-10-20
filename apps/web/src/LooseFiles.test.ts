import { render } from 'svelte/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LooseFiles from './LooseFiles.svelte';
import type { FileEntry } from './transport';
import { isCoverFile, materialPreviewKind } from './transport';

function file(name: string, extra: Partial<FileEntry> = {}): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind: 'material', availability: 'available', ...extra };
}

beforeEach(() => vi.stubGlobal('navigator', { userAgent: 'Windows' }));
afterEach(() => vi.unstubAllGlobals());

describe('loose files rendering', () => {
	it('classifies safe material previews and cover names without treating unknown files as covers', () => {
		expect(materialPreviewKind('capa.png')).toBe('image');
		expect(materialPreviewKind('apostila.pdf')).toBe('pdf');
		expect(materialPreviewKind('notas.md')).toBe('text');
		expect(materialPreviewKind('arquivo.zip')).toBeNull();
		expect(isCoverFile(file('Capa.png'))).toBe(true);
		expect(isCoverFile(file('manual.pdf'))).toBe(false);
	});
	it('renders one list row per file with player link, explicit download and status', () => {
		const { body } = render(LooseFiles, { props: { files: [file('apostila.pdf'), file('aula.mp4', { kind: 'media', path: 'aula.mp4' }), file('extra.pdf', { unverified: true })], heading: 'Arquivos soltos', courseId: 'course-1' } });
		expect(body).toContain('<strong>Arquivos soltos</strong>');
		expect(body.match(/<li>/g)).toHaveLength(3);
		expect(body).toContain('href="#/player/course-1/aula.mp4"');
		expect(body).toContain('/api/media?course=course-1&amp;path=apostila.pdf');
		expect(body).toContain('download="apostila.pdf"');
		expect(body).toContain('target="_blank"');
		expect(body).toContain('rel="noopener noreferrer"');
		expect(body).toContain('(não verificado)');
	});
	it('keeps missing files noninteractive even when the transport could build a URL', () => {
		const { body } = render(LooseFiles, { props: { files: [file('ausente.mp4', { kind: 'media', availability: 'missing' })], heading: 'Arquivos soltos', courseId: 'course-1' } });
		expect(body).not.toContain('<a ');
		expect(body).toContain('ausente.mp4');
	});
});
