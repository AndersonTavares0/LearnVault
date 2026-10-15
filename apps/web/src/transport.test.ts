import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { findLesson } from './router';
import { DesktopStub, HttpTransport, normalizeCatalog, normalizeUser, type Transport } from './transport';

// O Tauri só é carregado no desktop: fora dele o módulo é substituído pelo mock, de modo
// que a suíte nunca toca `window.__TAURI_INTERNALS__`.
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('transport contract', () => {
	it('exposes the HTTP transport seam without routing assumptions', () => {
		const methods: (keyof Transport)[] = ['me', 'login', 'logout', 'config', 'catalog', 'chooseLibrary', 'rescan', 'updateCredentials', 'mediaUrl'];
		expect(methods).toHaveLength(9);
	});
	it('does not offer native library selection over HTTP', async () => {
		await expect(new HttpTransport().chooseLibrary()).rejects.toThrow('definida no servidor');
	});
	it('troca credenciais temporárias pelo contrato autenticado', async () => {
		const user = { user_id: 'u1', username: 'novo', is_admin: true, must_change_credentials: false };
		const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify(user), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().updateCredentials('novo', 'senha suficientemente longa')).resolves.toMatchObject({ username: 'novo', mustChangeCredentials: false });
		expect(fetchMock).toHaveBeenCalledWith('/api/account/credentials', expect.objectContaining({ method: 'POST' }));
		vi.unstubAllGlobals();
	});
	it('builds media and caption URLs using the existing endpoint contract', () => {
		const transport = new HttpTransport();
		expect(transport.mediaUrl('módulo/aula.mp4')).toBe('/api/media?path=m%C3%B3dulo%2Faula.mp4');
		expect(transport.mediaUrl('legendas/aula.vtt')).toBe('/api/media?path=legendas%2Faula.vtt');
	});
	it('does not expose arbitrary desktop paths as media URLs', () => {
		expect(new DesktopStub().mediaUrl('private/video.mp4')).toBeNull();
	});
	it('reports the HTTP platform as configured without any request', async () => {
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().config()).resolves.toEqual({ configured: true });
		expect(fetchMock).not.toHaveBeenCalled();
		vi.unstubAllGlobals();
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
	it('projects a realistic Go catalog into non-empty canonical nodes for the SPA', () => {
		// Mirrors the Go /api/catalog payload: recursive category/course/module/lesson tree,
		// files carry no `kind` (the server classifies by media/subtitles/materials arrays),
		// root loose_files are present and `nodes` is omitted (json omitempty).
		const catalog = normalizeCatalog({
			categories: [{
				title: 'Programação', path: 'programming',
				courses: [{
					title: 'Go', path: 'programming/go',
					modules: [{
						title: 'Básico', path: 'programming/go/basics',
						lessons: [{
							title: 'Primeiros passos', path: 'programming/go/basics/first',
							media: [{ name: 'video.mp4', path: 'programming/go/basics/first/video.mp4', size: 42 }],
							subtitles: [{ name: 'video.vtt', path: 'programming/go/basics/first/video.vtt', size: 7 }],
							materials: [{ name: 'notes.pdf', path: 'programming/go/basics/first/notes.pdf', size: 99 }]
						}]
					}]
				}]
			}],
			loose_files: [{ name: 'intro.pdf', path: 'intro.pdf', size: 5 }]
		});
		// App.svelte renders only `catalog.nodes`, so an empty projection was the P1 hole.
		expect(catalog.nodes.length).toBeGreaterThan(0);
		expect(catalog.nodes[0].role).toBe('category');
		expect(catalog.nodes[0].children[0].children[0].children[0].path).toBe('programming/go/basics/first');
		expect(catalog.looseFiles[0].path).toBe('intro.pdf');
		// The player resolves the lesson from the canonical nodes collection via findLesson.
		const lesson = findLesson(catalog, 'programming/go/basics/first');
		expect(lesson?.media[0]).toMatchObject({ path: 'programming/go/basics/first/video.mp4', kind: 'media' });
		expect(lesson?.subtitles[0]).toMatchObject({ path: 'programming/go/basics/first/video.vtt', kind: 'subtitle' });
		expect(lesson?.materials[0]).toMatchObject({ path: 'programming/go/basics/first/notes.pdf', kind: 'material' });
		// categories is a projection of nodes, not a second parallel tree.
		expect(catalog.categories[0].courses[0].modules[0].lessons[0].path).toBe('programming/go/basics/first');
	});
	it('rejects an unknown catalog shape with a controlled error', () => expect(() => normalizeCatalog({ nope: true })).toThrow('Resposta inválida do servidor'));
});

describe('desktop transport over IPC', () => {
	beforeEach(() => { vi.mocked(invoke).mockReset(); });

	it('loads the catalog through the desktop_catalog command without touching fetch', async () => {
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		vi.mocked(invoke).mockResolvedValue({
			complete: true,
			loose_files: [],
			categories: [{ title: 'Curso', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }]
		});

		const catalog = await new DesktopStub().catalog();

		expect(invoke).toHaveBeenCalledWith('desktop_catalog');
		expect(fetchMock).not.toHaveBeenCalled();
		expect(catalog.nodes[0].role).toBe('category');
		expect(catalog.categories[0].title).toBe('Curso');
		vi.unstubAllGlobals();
	});

	it('maps a known IPC code to its stable sanitized message, ignoring arbitrary text', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'catalog_unavailable', message: 'mensagem arbitrária em /mnt/cursos' });
		await expect(new DesktopStub().catalog()).rejects.toThrow('catálogo indisponível');
	});

	it('falls back to a stable message for an unknown or missing IPC code', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'algo_desconhecido', message: 'CAMINHO privado /mnt/cursos' });
		await expect(new DesktopStub().catalog()).rejects.toThrow('Não foi possível carregar o catálogo.');
		vi.mocked(invoke).mockRejectedValue({ message: 'sem código /etc/shadow' });
		await expect(new DesktopStub().catalog()).rejects.toThrow('Não foi possível carregar o catálogo.');
	});

	it('keeps credential changes unimplemented on the desktop', async () => {
		const desktop: Transport = new DesktopStub();
		await expect(desktop.updateCredentials('u', 'senha')).rejects.toThrow('ainda não implementado');
	});

	it('triggers an explicit desktop rescan through the command without touching fetch', async () => {
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		vi.mocked(invoke).mockResolvedValue({ complete: true, inserted: 1 });

		await expect(new DesktopStub().rescan()).resolves.toBeUndefined();

		expect(invoke).toHaveBeenCalledWith('desktop_rescan');
		expect(fetchMock).not.toHaveBeenCalled();
		vi.unstubAllGlobals();
	});

	it('maps each stable desktop rescan error code to a sanitized message', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'rescan_in_progress', message: '/mnt/cursos em uso' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Já existe uma indexação em andamento.');
		vi.mocked(invoke).mockRejectedValue({ code: 'library_not_configured', message: '/mnt/cursos não configurada' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Nenhuma biblioteca está configurada.');
		vi.mocked(invoke).mockRejectedValue({ code: 'rescan_unavailable', message: 'falha em C:\\segredo\\cursos' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Não foi possível indexar a biblioteca.');
	});

	it('falls back to a stable message for an unknown desktop rescan error code', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'algo_desconhecido', message: 'CAMINHO privado /mnt/cursos' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Não foi possível indexar a biblioteca.');
		vi.mocked(invoke).mockRejectedValue({ message: 'sem código /etc/shadow' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Não foi possível indexar a biblioteca.');
	});

	it('reports the desktop as unconfigured through the desktop_config command', async () => {
		vi.mocked(invoke).mockResolvedValue({ configured: false });
		await expect(new DesktopStub().config()).resolves.toEqual({ configured: false });
		expect(invoke).toHaveBeenCalledWith('desktop_config');
	});

	it('selects the library through the desktop command without sending a path', async () => {
		vi.mocked(invoke).mockResolvedValue({ configured: true });
		await expect(new DesktopStub().chooseLibrary()).resolves.toEqual({ configured: true });
		// O webview apenas dispara o seletor nativo; nenhum caminho cruza o IPC.
		expect(invoke).toHaveBeenCalledWith('desktop_select_library');
	});

	it('keeps the previous state when the user cancels the native picker', async () => {
		vi.mocked(invoke).mockResolvedValue({ configured: false });
		await expect(new DesktopStub().chooseLibrary()).resolves.toEqual({ configured: false });
	});

	it('maps a library selection error to a sanitized message without the path', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'library_root_invalid', message: '/mnt/cursos não é válido' });
		await expect(new DesktopStub().chooseLibrary()).rejects.toThrow('A pasta escolhida não é uma biblioteca válida.');
		vi.mocked(invoke).mockRejectedValue({ code: 'library_config_unavailable', message: 'falha em C:\\segredo' });
		await expect(new DesktopStub().chooseLibrary()).rejects.toThrow('Não foi possível salvar a configuração do aplicativo.');
	});

	it('maps a sanitized desktop configuration error to its message', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'database_unavailable', message: 'banco de dados local indisponível' });
		await expect(new DesktopStub().config()).rejects.toThrow('banco de dados local indisponível');
	});
});
