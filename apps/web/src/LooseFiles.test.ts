import { render } from 'svelte/server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LooseFiles from './LooseFiles.svelte';
import type { FileEntry } from './transport';

function file(name: string, extra: Partial<FileEntry> = {}): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind: 'material', availability: 'available', ...extra };
}

beforeEach(() => vi.stubGlobal('navigator', { userAgent: 'Windows' }));
afterEach(() => vi.unstubAllGlobals());

describe('loose files rendering', () => {
	it('renders one card per file with type, name, action and status', () => {
		const { body } = render(LooseFiles, { props: { files: [file('apostila.pdf'), file('aula.mp4', { kind: 'media', path: 'aula.mp4' }), file('extra.pdf', { unverified: true })], heading: 'Arquivos soltos', courseId: 'course-1' } });
		expect(body).toContain('<strong>Arquivos soltos</strong>');
		expect(body.match(/<li>/g)).toHaveLength(3);
		// O tipo vem da extensão: o nome sozinho não diz o que a peça é.
		expect(body).toContain('<span class="file-type">PDF</span>');
		expect(body).toContain('<span class="file-type">Vídeo</span>');
		expect(body).toContain('<span class="file-name">apostila.pdf</span>');
		expect(body).toContain('<span class="file-action">Baixar</span>');
		expect(body).toContain('<span class="file-action">Reproduzir</span>');
		expect(body).toContain('href="#/course/course-1/aula.mp4"');
		expect(body).toContain('/api/media?course=course-1&amp;path=apostila.pdf');
		expect(body).toContain('download="apostila.pdf"');
		expect(body).toContain('target="_blank"');
		expect(body).toContain('rel="noopener noreferrer"');
		expect(body).toContain('não verificado');
	});
	it('keeps missing files noninteractive even when the transport could build a URL', () => {
		const { body } = render(LooseFiles, { props: { files: [file('ausente.mp4', { kind: 'media', availability: 'missing' })], heading: 'Arquivos soltos', courseId: 'course-1' } });
		expect(body).not.toContain('<a ');
		expect(body).toContain('ausente.mp4');
	});
});
