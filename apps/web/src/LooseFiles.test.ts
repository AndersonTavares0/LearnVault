import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import LooseFiles from './LooseFiles.svelte';
import type { FileEntry } from './transport';

function file(name: string, extra: Partial<FileEntry> = {}): FileEntry {
	return { name, path: name, size: 10, mediaId: name, kind: 'material', availability: 'available', ...extra };
}

describe('loose files rendering', () => {
	it('renders one list row per file with name, course-scoped link and status', () => {
		const { body } = render(LooseFiles, { props: { files: [file('apostila.pdf'), file('aviso.pdf', { availability: 'missing' }), file('extra.pdf', { unverified: true })], heading: 'Arquivos soltos', courseId: 'course-1' } });
		expect(body).toContain('<strong>Arquivos soltos</strong>');
		expect(body.match(/<li>/g)).toHaveLength(3);
		expect(body).toContain('href="/api/media?course=course-1&amp;path=apostila.pdf"');
		expect(body).toContain('(ausente)');
		expect(body).toContain('(não verificado)');
	});
});
