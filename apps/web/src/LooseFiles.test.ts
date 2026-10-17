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
	it('keeps missing files noninteractive even when the transport could build a URL', () => {
		const { body } = render(LooseFiles, { props: { files: [file('ausente.mp4', { kind: 'media', availability: 'missing' })], heading: 'Arquivos soltos' } });
		expect(body).not.toContain('<a ');
		expect(body).toContain('ausente.mp4');
	});
	it('routes playable media through the application rather than the raw media endpoint', () => {
		const { body } = render(LooseFiles, { props: { files: [file('aula.mp4', { kind: 'media', path: 'curso/aula.mp4' })], heading: 'Arquivos soltos' } });
		expect(body).toContain('href="#/player/curso%2Faula.mp4"');
		expect(body).not.toContain('/api/media');
		expect(body).not.toContain('learnvault-media');
	});
	it('makes materials explicit downloads and prevents same-window raw navigation', () => {
		const { body } = render(LooseFiles, { props: { files: [file('apostila.pdf')], heading: 'Materiais' } });
		expect(body).toContain('download="apostila.pdf"');
		expect(body).toContain('target="_blank"');
		expect(body).toContain('rel="noopener noreferrer"');
		expect(body).toContain('Baixar apostila.pdf');
	});
});
