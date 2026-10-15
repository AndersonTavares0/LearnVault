import { describe, expect, it, vi } from 'vitest';
import { DesktopStub, HttpTransport, normalizeCatalog, normalizeUser, type Transport } from './transport';

describe('transport contract', () => {
	it('exposes the HTTP transport seam without routing assumptions', () => {
		const methods: (keyof Transport)[] = ['me', 'login', 'logout', 'catalog', 'rescan', 'mediaUrl'];
		expect(methods).toHaveLength(6);
	});
	it('builds media and caption URLs using the existing endpoint contract', () => {
		const transport = new HttpTransport();
		expect(transport.mediaUrl('módulo/aula.mp4')).toBe('/api/media?path=m%C3%B3dulo%2Faula.mp4');
		expect(transport.mediaUrl('legendas/aula.vtt')).toBe('/api/media?path=legendas%2Faula.vtt');
	});
	it('does not expose arbitrary desktop paths as media URLs', () => {
		expect(new DesktopStub().mediaUrl('private/video.mp4')).toBeNull();
	});
	it('posts the admin rescan endpoint and maps a conflict', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetchMock);
		await new HttpTransport().rescan();
		expect(fetchMock).toHaveBeenCalledWith('/api/admin/catalog/rescan', { method: 'POST' });
		fetchMock.mockResolvedValue(new Response(null, { status: 409 }));
		await expect(new HttpTransport().rescan()).rejects.toThrow('Já existe um rescan em andamento.');
		vi.unstubAllGlobals();
	});
	it('normalizes the Rust user and recursive catalog contract', () => {
		expect(normalizeUser({ user_id: 'u1', username: 'admin', is_admin: true, must_change_credentials: true })).toEqual({ userId: 'u1', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: true });
		const catalog = normalizeCatalog({ complete: true, loose_files: [{ media_id: 'loose', name: 'nota.pdf', path: 'nota.pdf', size: 3, kind: 'material', availability: 'missing', unverified: true }], categories: [{ title: 'Categoria', path: 'cat', role: 'category', files: [], loose_files: [], children: [{ title: 'Curso', path: 'cat/curso', role: 'course', files: [], loose_files: [], children: [{ title: 'Módulo', path: 'cat/curso/mod', role: 'module', files: [], loose_files: [], children: [{ title: 'Aula', path: 'cat/curso/mod/aula', role: 'lesson', files: [{ media_id: 'm1', name: 'aula.mp4', path: 'cat/curso/mod/aula.mp4', size: 10, kind: 'media', availability: 'available', unverified: false }, { media_id: 's1', name: 'aula.vtt', path: 'cat/curso/mod/aula.vtt', size: 2, kind: 'subtitle', availability: 'available', unverified: false }], loose_files: [], children: [] }] }] }] }] });
		expect(catalog.complete).toBe(true);
		expect(catalog.looseFiles[0]).toMatchObject({ mediaId: 'loose', path: 'nota.pdf', availability: 'missing', unverified: true });
		expect(catalog.nodes[0].children[0].path).toBe('cat/curso');
		expect(catalog.categories[0].courses[0].modules[0].lessons[0].media[0].path).toBe('cat/curso/mod/aula.mp4');
		expect(catalog.categories[0].courses[0].modules[0].lessons[0].media[0].mediaId).toBe('m1');
		expect(catalog.categories[0].courses[0].modules[0].lessons[0].subtitles).toHaveLength(1);
	});
	it('preserves root loose files when a Rust catalog has no categories', () => {
		const catalog = normalizeCatalog({ complete: true, categories: [], loose_files: [{ name: 'nota.pdf', path: 'nota.pdf', size: 3, kind: 'material' }] });
		expect(catalog.categories).toEqual([]);
		expect(catalog.looseFiles[0].path).toBe('nota.pdf');
	});
	it('keeps shallow recursive lessons and files addressable by path', () => {
		const catalog = normalizeCatalog({ complete: true, loose_files: [], nodes: [{ title: 'Categoria', path: 'cat', role: 'category', files: [{ name: 'cat.pdf', path: 'cat/cat.pdf', size: 1, kind: 'material' }], loose_files: [], children: [{ title: 'Aula', path: 'cat/aula', role: 'lesson', files: [{ name: 'aula.mp4', path: 'cat/aula.mp4', size: 4, kind: 'media' }], loose_files: [], children: [] }] }] });
		expect(catalog.nodes[0].children[0].path).toBe('cat/aula');
		expect(catalog.nodes[0].children[0].files[0].path).toBe('cat/aula.mp4');
	});
	it('keeps the Go tree and material files compatible', () => {
		const catalog = normalizeCatalog({ categories: [{ title: 'Categoria', courses: [{ title: 'Curso', modules: [{ title: 'Módulo', lessons: [{ title: 'Aula', path: 'aula', media: [], subtitles: [{ name: 'legenda.vtt', path: 'legenda.vtt', size: 1 }], materials: [{ name: 'slide.pdf', path: 'slide.pdf', size: 2 }] }] }] }] }] });
		expect(catalog.categories[0].courses[0].modules[0].lessons[0].materials[0].path).toBe('slide.pdf');
		expect(catalog.categories[0].courses[0].modules[0].lessons[0].subtitles[0].path).toBe('legenda.vtt');
	});
	it('rejects an unknown catalog shape with a controlled error', () => expect(() => normalizeCatalog({ nope: true })).toThrow('Resposta inválida do servidor'));
});
