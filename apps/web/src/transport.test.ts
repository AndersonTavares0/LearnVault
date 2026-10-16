import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { findLesson } from './router';
import { desktopMediaUrl, DesktopStub, HttpTransport, normalizeCatalog, normalizeRecentStudy, normalizeStudyProgress, normalizeUser, STUDY_NOT_AVAILABLE, type Transport } from './transport';

// O Tauri só é carregado no desktop: fora dele o módulo é substituído pelo mock, de modo
// que a suíte nunca toca `window.__TAURI_INTERNALS__`.
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('transport contract', () => {
	it('exposes the HTTP transport seam without routing assumptions', () => {
		const methods: (keyof Transport)[] = ['me', 'login', 'logout', 'config', 'catalog', 'chooseLibrary', 'rescan', 'updateCredentials', 'studyProgress', 'recordPosition', 'setCompleted', 'recentStudy', 'mediaUrl'];
		expect(methods).toHaveLength(13);
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
	it('builds media and caption URLs from the path on HTTP', () => {
		const transport = new HttpTransport();
		expect(transport.mediaUrl({ name: 'aula.mp4', path: 'módulo/aula.mp4', size: 1, kind: 'media', mediaId: 'm1' })).toBe('/api/media?path=m%C3%B3dulo%2Faula.mp4');
		expect(transport.mediaUrl({ name: 'aula.vtt', path: 'legendas/aula.vtt', size: 1, kind: 'subtitle', mediaId: 's1' })).toBe('/api/media?path=legendas%2Faula.vtt');
	});
	it('builds desktop media URLs from the authorized media id, never the raw path', () => {
		const transport = new DesktopStub();
		// Os IDs são UUID v7 gerados pelo domínio: ASCII seguros, sem percent-encoding, o
		// que preserva o contrato do parser estrito do Rust.
		const uuid = '0192f0e0-1111-7abc-8def-0123456789ab';
		// Fixar o `userAgent` de Linux isola a forma do esquema da plataforma que executa a
		// suíte: o bundle desktop é o mesmo e só a base da URL muda por plataforma.
		vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)' });
		try {
			expect(transport.mediaUrl({ name: 'foto.png', path: 'Curso privado/foto.png', size: 1, kind: 'material', mediaId: uuid })).toBe(`learnvault-media://localhost/${uuid}`);
			// Vídeo/áudio e legendas ficam fora do protocolo nesta fatia.
			expect(transport.mediaUrl({ name: 'aula.mp4', path: 'Curso privado/aula.mp4', size: 1, kind: 'media', mediaId: 'v1' })).toBeNull();
			expect(transport.mediaUrl({ name: 'aula.vtt', path: 'Curso privado/aula.vtt', size: 1, kind: 'subtitle', mediaId: 's1' })).toBeNull();
			// Um arquivo sem identidade autorizada não é endereçável.
			expect(transport.mediaUrl({ name: 'x.pdf', path: 'Curso privado/x.pdf', size: 1, kind: 'material' })).toBeNull();
			const url = transport.mediaUrl({ name: 'x.pdf', path: 'Curso privado/x.pdf', size: 1, kind: 'material', mediaId: 'm1' })!;
			expect(url).toBe('learnvault-media://localhost/m1');
			expect(url).not.toContain('Curso privado');
		} finally {
			vi.unstubAllGlobals();
		}
	});
	it('addresses the desktop protocol through the WebView2 http.localhost prefix on Windows', () => {
		// Regressão da causa-raiz: no Windows o WebView2 não resolve `learnvault-media://`.
		// O runtime só entrega o esquema quando o webview pede
		// `http://learnvault-media.localhost/<id>` e depois reverte para o handler.
		const windows = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0';
		const uuid = '0192f0e0-2222-7abc-8def-0123456789ab';
		const transport = new DesktopStub();
		vi.stubGlobal('navigator', { userAgent: windows });
		try {
			const url = transport.mediaUrl({ name: 'foto.png', path: 'Curso privado/foto.png', size: 1, kind: 'material', mediaId: uuid })!;
			expect(url).toBe(`http://learnvault-media.localhost/${uuid}`);
			expect(url).not.toContain('Curso privado');
			expect(transport.mediaUrl({ name: 'aula.mp4', path: 'Curso privado/aula.mp4', size: 1, kind: 'media', mediaId: 'v1' })).toBeNull();
		} finally {
			vi.unstubAllGlobals();
		}
	});
	it('keeps the direct scheme on Linux and macOS and only changes the base per platform', () => {
		const uuid = '0192f0e0-3333-7abc-8def-0123456789ab';
		expect(desktopMediaUrl(uuid, 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)')).toBe(`learnvault-media://localhost/${uuid}`);
		expect(desktopMediaUrl(uuid, 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)')).toBe(`learnvault-media://localhost/${uuid}`);
		expect(desktopMediaUrl(uuid, 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0')).toBe(`http://learnvault-media.localhost/${uuid}`);
	});
	it('addresses the desktop protocol through the http.localhost prefix on Android, like Windows', () => {
		// O workaround do wry é o mesmo no WebView2 (Windows) e no webview do Android (ambos
		// Chromium): `http://<esquema>.localhost` é revertido para o esquema antes do handler.
		// Sem isso o Android cairia no esquema direto, que não é resolvido no Chromium.
		const uuid = '0192f0e0-4444-7abc-8def-0123456789ab';
		const android = 'Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36';
		expect(desktopMediaUrl(uuid, android)).toBe(`http://learnvault-media.localhost/${uuid}`);
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
	it('prefers canonical nodes over the legacy categories field when both are present', () => {
		// O contrato Rust passa a expor `nodes` e `categories` com a mesma árvore recursiva
		// canônica. Quando os dois vêm no payload, a SPA prefere `nodes` e nunca cai na
		// projeção de categorias para montar a árvore.
		const catalog = normalizeCatalog({
			complete: true,
			loose_files: [],
			nodes: [{ title: 'DaNodes', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }],
			categories: [{ title: 'DaCategories', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }]
		});
		expect(catalog.nodes[0].title).toBe('DaNodes');
		expect(catalog.categories[0].title).toBe('DaNodes');
	});
	it('normalizes a Go payload that carries canonical nodes and omits root loose files', () => {
		// O serviço Go serializa a árvore canônica em `nodes` e omite `loose_files` na raiz
		// quando não há arquivos soltos (omitempty). A preferência por `nodes` precisa
		// aceitar essa ausência em vez de recusar o catálogo Go.
		const catalog = normalizeCatalog({
			nodes: [{
				title: 'Programação', path: 'programming', role: 'category', files: [], loose_files: [],
				children: [{
					title: 'Go', path: 'programming/go', role: 'group', files: [], loose_files: [],
					children: [{
						title: 'Primeiros passos', path: 'programming/go/first', role: 'lesson',
						files: [{ name: 'video.mp4', path: 'programming/go/first/video.mp4', size: 42, kind: 'media' }],
						loose_files: [], children: []
					}]
				}]
			}],
			categories: [{
				title: 'Programação', path: 'programming',
				courses: [{
					title: 'Go', path: 'programming/go',
					modules: [{ title: 'Primeiros passos', path: 'programming/go/first', lessons: [{ title: 'Primeiros passos', path: 'programming/go/first', media: [{ name: 'video.mp4', path: 'programming/go/first/video.mp4', size: 42 }], subtitles: [], materials: [] }] }]
				}]
			}]
		});
		expect(catalog.looseFiles).toEqual([]);
		expect(catalog.nodes[0].path).toBe('programming');
		expect(findLesson(catalog, 'programming/go/first')?.media[0].path).toBe('programming/go/first/video.mp4');
	});
	it('falls back to populated legacy categories when nodes is empty', () => {
		// Um `nodes` vazio não pode esconder a árvore legada entregue em `categories`.
		const catalog = normalizeCatalog({
			complete: true,
			nodes: [],
			loose_files: [],
			categories: [{ title: 'Legado', path: 'cat', role: 'category', files: [], loose_files: [], children: [{ title: 'Aula', path: 'cat/aula', role: 'lesson', files: [{ name: 'a.mp4', path: 'cat/aula.mp4', size: 4, kind: 'media' }], loose_files: [], children: [] }] }]
		});
		expect(catalog.nodes[0].title).toBe('Legado');
		expect(catalog.nodes[0].children[0].path).toBe('cat/aula');
	});
	it('falls back to a Go catalog when nodes is empty but categories is populated', () => {
		const catalog = normalizeCatalog({
			nodes: [],
			categories: [{ title: 'Programação', path: 'programming', courses: [{ title: 'Go', path: 'programming/go', modules: [{ title: 'Básico', path: 'programming/go/basics', lessons: [{ title: 'Aula', path: 'programming/go/basics/first', media: [{ name: 'v.mp4', path: 'programming/go/basics/first/v.mp4', size: 9 }], subtitles: [], materials: [] }] }] }] }]
		});
		expect(catalog.nodes[0].role).toBe('category');
		expect(findLesson(catalog, 'programming/go/basics/first')?.media[0].path).toBe('programming/go/basics/first/v.mp4');
	});
	it('rejects an explicitly malformed nodes field instead of falling back to categories', () => {
		// Um `nodes` presente com tipo inválido é payload malformado: cair para `categories`
		// esconderia o erro em vez de recusá-lo, e a árvore canônica ficaria ambígua.
		const categories = [{ title: 'Legado', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }];
		for (const nodes of [null, 'nodes', 42, true, {}]) {
			expect(() => normalizeCatalog({ nodes, categories })).toThrow('Resposta inválida do servidor');
		}
	});
	it('falls back to categories only when nodes is absent', () => {
		const catalog = normalizeCatalog({ categories: [{ title: 'Legado', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }] });
		expect(catalog.nodes[0].title).toBe('Legado');
	});
	it('preserves lessons through unknown group descendants in the compatibility projection', () => {
		// O serviço Go usa o papel `group` nos níveis intermediários. A projeção de
		// compatibilidade precisa descer por eles em vez de descartar as aulas.
		const catalog = normalizeCatalog({
			nodes: [{
				title: 'Programação', path: 'programming', role: 'category', files: [], loose_files: [],
				children: [{
					title: 'Go', path: 'programming/go', role: 'group', files: [], loose_files: [],
					children: [{
						title: 'Básico', path: 'programming/go/basics', role: 'group', files: [], loose_files: [],
						children: [{ title: 'Aula', path: 'programming/go/basics/first', role: 'lesson', files: [{ name: 'v.mp4', path: 'programming/go/basics/first/v.mp4', size: 9, kind: 'media' }], loose_files: [], children: [] }]
					}]
				}]
			}]
		});
		expect(catalog.categories[0].courses[0].modules[0].lessons.map((item) => item.path)).toEqual(['programming/go/basics/first']);
	});
	it('preserves lessons when a module sits directly under a category without a course level', () => {
		const catalog = normalizeCatalog({
			complete: true,
			loose_files: [],
			nodes: [{ title: 'Cat', path: 'Cat', role: 'category', files: [], loose_files: [], children: [{ title: 'Curso', path: 'Cat/Curso', role: 'module', files: [], loose_files: [], children: [{ title: 'Aula', path: 'Cat/Curso/Aula', role: 'lesson', files: [{ name: 'a.mp4', path: 'Cat/Curso/Aula/a.mp4', size: 1, kind: 'media' }], loose_files: [], children: [] }] }] }]
		});
		expect(catalog.categories[0].courses[0].modules[0].lessons[0].path).toBe('Cat/Curso/Aula');
	});
	it('prefers canonical nodes when the legacy categories carry Go-shaped courses', () => {
		// O servidor Rust passou a enviar `nodes` recursivo e `categories` na forma legada do
		// serviço Go (`courses`/`modules`/`lessons`). A SPA usa `nodes` para montar a árvore e
		// reprojeta as categorias, sem confundir os dois shapes.
		const catalog = normalizeCatalog({
			complete: true,
			loose_files: [],
			nodes: [{ title: 'DaNodes', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }],
			categories: [{ title: 'DaCategories', path: 'cat', courses: [{ title: 'Curso', path: 'cat/curso', modules: [{ title: 'Mod', path: 'cat/curso/mod', lessons: [{ title: 'Aula', path: 'cat/curso/mod/aula', media: [{ name: 'v.mp4', path: 'cat/curso/mod/aula/v.mp4', size: 1 }], subtitles: [], materials: [] }] }] }] }]
		});
		expect(catalog.nodes[0].title).toBe('DaNodes');
		// categories é reprojetada a partir de nodes, não copiada do payload legado.
		expect(catalog.categories[0].title).toBe('DaNodes');
	});
	it('keeps a node of an unknown role in the canonical tree instead of dropping it', () => {
		// Um papel desconhecido (por exemplo um `group` futuro) não pode sumir: a árvore
		// canônica o preserva com seus arquivos, e a projeção legada desce por ele.
		const catalog = normalizeCatalog({
			nodes: [{
				title: 'Cat', path: 'cat', role: 'category', files: [], loose_files: [],
				children: [{
					title: 'Grupo', path: 'cat/grupo', role: 'group',
					files: [{ name: 'guia.pdf', path: 'cat/grupo/guia.pdf', size: 2, kind: 'material' }],
					loose_files: [], children: []
				}]
			}]
		});
		expect(catalog.nodes[0].children[0].role).toBe('group');
		expect(catalog.nodes[0].children[0].files[0].path).toBe('cat/grupo/guia.pdf');
		// A projeção mantém o arquivo do nó desconhecido em vez de descartá-lo.
		expect(catalog.categories[0].courses[0].looseFiles.map((file) => file.path)).toEqual(['cat/grupo/guia.pdf']);
	});
	it('keeps an unknown file kind instead of dropping it from the lesson projection', () => {
		// Um `kind` que a SPA ainda não conhece não pode fazer o arquivo desaparecer: a árvore
		// canônica preserva o valor original e a projeção o trata como material (catch-all).
		const catalog = normalizeCatalog({
			nodes: [{
				title: 'Cat', path: 'cat', role: 'category', files: [], loose_files: [],
				children: [{
					title: 'Aula', path: 'cat/aula', role: 'lesson',
					files: [{ name: 'extra.bin', path: 'cat/aula/extra.bin', size: 1, kind: 'bonus' }],
					loose_files: [], children: []
				}]
			}]
		});
		expect(catalog.nodes[0].children[0].files[0].kind).toBe('bonus');
		const projected = catalog.categories[0].courses[0].modules[0].lessons[0];
		expect(projected.media).toEqual([]);
		expect(projected.subtitles).toEqual([]);
		expect(projected.materials.map((file) => file.path)).toEqual(['cat/aula/extra.bin']);
	});
	it('rejects an explicit null root loose_files instead of treating it as absent', () => {
		expect(() => normalizeCatalog({ nodes: [], categories: [], loose_files: null })).toThrow('Resposta inválida do servidor');
		expect(() => normalizeCatalog({ nodes: [], looseFiles: null })).toThrow('Resposta inválida do servidor');
	});
	it('rejects a wrong-type or nested-array root loose_files', () => {
		expect(() => normalizeCatalog({ nodes: [], loose_files: 'nao-lista' })).toThrow('Resposta inválida do servidor');
		expect(() => normalizeCatalog({ nodes: [], loose_files: [[{ name: 'a.pdf', path: 'a.pdf', size: 1 }]] })).toThrow('Resposta inválida do servidor');
	});
	it('rejects conflicting snake_case and camelCase loose files', () => {
		expect(() => normalizeCatalog({ nodes: [], loose_files: [], looseFiles: [{ name: 'a.pdf', path: 'a.pdf', size: 1 }] })).toThrow('conflitantes');
	});
	it('accepts equal snake_case and camelCase loose files deterministically', () => {
		const file = { name: 'a.pdf', path: 'a.pdf', size: 1, kind: 'material' };
		const catalog = normalizeCatalog({ nodes: [], loose_files: [file], looseFiles: [file] });
		expect(catalog.looseFiles[0].path).toBe('a.pdf');
	});
	it('rejects a present optional file field with the wrong type', () => {
		// O serviço Go não emite esses campos, então a ausência é aceita; um valor presente
		// com tipo incorreto não pode vazar para a interface.
		const base = { name: 'a.pdf', path: 'a.pdf', size: 1 };
		const malformed = [
			{ ...base, media_id: 42 },
			{ ...base, media_id: null },
			{ ...base, kind: 7 },
			{ ...base, availability: false },
			{ ...base, unverified: 'sim' }
		];
		for (const file of malformed) {
			expect(() => normalizeCatalog({ nodes: [], loose_files: [file] })).toThrow('Resposta inválida do servidor');
		}
		// O mesmo vale para um arquivo aninhado em um nó canônico.
		expect(() => normalizeCatalog({ nodes: [{ title: 'C', path: 'c', role: 'category', files: [{ ...base, media_id: 1 }], loose_files: [], children: [] }] })).toThrow('Resposta inválida do servidor');
	});
	it('keeps a file whose optional fields are present and valid', () => {
		const catalog = normalizeCatalog({ nodes: [], loose_files: [{ name: 'a.pdf', path: 'a.pdf', size: 5, media_id: 'm1', kind: 'material', availability: 'missing', unverified: true }] });
		expect(catalog.looseFiles[0]).toEqual({ name: 'a.pdf', path: 'a.pdf', size: 5, mediaId: 'm1', kind: 'material', availability: 'missing', unverified: true });
	});
	it('rejects a fractional, negative or unsafe file size', () => {
		const base = { name: 'a.pdf', path: 'a.pdf' };
		for (const size of [1.5, -1, Number.MAX_SAFE_INTEGER + 1, Number.NaN]) {
			expect(() => normalizeCatalog({ nodes: [], loose_files: [{ ...base, size }] })).toThrow('Resposta inválida do servidor');
		}
	});
	it('rejects an unknown catalog shape with a controlled error', () => expect(() => normalizeCatalog({ nope: true })).toThrow('Resposta inválida do servidor'));
	it('normalizes a study progress record with optional timestamp', () => {
		expect(normalizeStudyProgress({ media_id: 'm1', position_seconds: 42, completed: false, updated_at: 1234567890, recorded: true })).toEqual({ mediaId: 'm1', positionSeconds: 42, completed: false, updatedAt: 1234567890, recorded: true });
		expect(normalizeStudyProgress({ media_id: 'm2', position_seconds: 0, completed: true, recorded: true })).toEqual({ mediaId: 'm2', positionSeconds: 0, completed: true, updatedAt: null, recorded: true });
		expect(() => normalizeStudyProgress({ media_id: 'm3', position_seconds: 'zero', completed: true, recorded: true })).toThrow('Resposta inválida do servidor');
	});
	it('normalizes a recent study list', () => {
		const recent = normalizeRecentStudy({ items: [{ media_id: 'm1', position_seconds: 10, completed: false, recorded: true }] });
		expect(recent.items).toHaveLength(1);
		expect(recent.items[0].mediaId).toBe('m1');
	});
	it('reads study progress from the HTTP endpoint', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ media_id: 'm1', position_seconds: 42, completed: false, recorded: true }), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);
		const progress = await new HttpTransport().studyProgress('m1');
		expect(fetchMock).toHaveBeenCalledWith('/api/study/progress?media_id=m1');
		expect(progress).toMatchObject({ mediaId: 'm1', positionSeconds: 42, completed: false, recorded: true });
		vi.unstubAllGlobals();
	});
	it('records position and completion through the HTTP endpoints', async () => {
		const fetchMock = vi.fn().mockImplementation(() => new Response(JSON.stringify({ media_id: 'm1', position_seconds: 90, completed: true, recorded: true }), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().recordPosition('m1', 90)).resolves.toMatchObject({ positionSeconds: 90, completed: true });
		expect(fetchMock).toHaveBeenCalledWith('/api/study/position', expect.objectContaining({ method: 'POST', body: JSON.stringify({ media_id: 'm1', position_seconds: 90 }) }));
		fetchMock.mockClear();
		await expect(new HttpTransport().setCompleted('m1', false)).resolves.toMatchObject({ mediaId: 'm1', recorded: true });
		expect(fetchMock).toHaveBeenCalledWith('/api/study/completed', expect.objectContaining({ method: 'POST', body: JSON.stringify({ media_id: 'm1', completed: false }) }));
		vi.unstubAllGlobals();
	});
	it('loads recent study history from the HTTP endpoint', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ items: [{ media_id: 'm1', position_seconds: 7, completed: false, recorded: true }] }), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);
		const recent = await new HttpTransport().recentStudy(5);
		expect(fetchMock).toHaveBeenCalledWith('/api/study/recent?limit=5');
		expect(recent.items[0].mediaId).toBe('m1');
		vi.unstubAllGlobals();
	});
	it('reports study endpoints as unavailable when the server returns 404', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response('404 page not found', { status: 404 }));
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().recentStudy()).rejects.toMatchObject({ message: 'Estudo não disponível neste servidor.', code: STUDY_NOT_AVAILABLE });
		vi.unstubAllGlobals();
	});
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

	it('prefers canonical nodes over the legacy categories field in the desktop payload', async () => {
		// O `desktop_catalog` também passa a devolver `nodes` e `categories`; a SPA usa
		// `nodes` e ignora a árvore legada quando ela está presente.
		vi.mocked(invoke).mockResolvedValue({
			complete: true,
			loose_files: [],
			nodes: [{ title: 'Nós', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }],
			categories: [{ title: 'Categorias', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }]
		});

		const catalog = await new DesktopStub().catalog();

		expect(catalog.nodes[0].title).toBe('Nós');
		expect(catalog.categories[0].title).toBe('Nós');
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

	it('reads study progress through the desktop IPC command without touching fetch', async () => {
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		vi.mocked(invoke).mockResolvedValue({ media_id: 'm1', position_seconds: 42, completed: true, updated_at: 1234567890, recorded: true });

		const progress = await new DesktopStub().studyProgress('m1');

		expect(invoke).toHaveBeenCalledWith('desktop_study_progress', { mediaId: 'm1' });
		expect(fetchMock).not.toHaveBeenCalled();
		expect(progress).toEqual({ mediaId: 'm1', positionSeconds: 42, completed: true, updatedAt: 1234567890, recorded: true });
		vi.unstubAllGlobals();
	});

	it('normalizes an absent desktop progress record as not recorded', async () => {
		// Uma mídia conhecida sem progresso responde com o estado neutro, sem erro.
		vi.mocked(invoke).mockResolvedValue({ media_id: 'm1', position_seconds: 0, completed: false, updated_at: null, recorded: false });

		await expect(new DesktopStub().studyProgress('m1')).resolves.toEqual({ mediaId: 'm1', positionSeconds: 0, completed: false, updatedAt: null, recorded: false });
	});

	it('records position and completion through the desktop IPC commands', async () => {
		vi.mocked(invoke).mockResolvedValue({ media_id: 'm1', position_seconds: 90, completed: false, updated_at: 10, recorded: true });

		await expect(new DesktopStub().recordPosition('m1', 90)).resolves.toMatchObject({ mediaId: 'm1', positionSeconds: 90 });
		expect(invoke).toHaveBeenCalledWith('desktop_record_position', { mediaId: 'm1', positionSeconds: 90 });

		vi.mocked(invoke).mockResolvedValue({ media_id: 'm1', position_seconds: 90, completed: true, updated_at: 11, recorded: true });
		await expect(new DesktopStub().setCompleted('m1', true)).resolves.toMatchObject({ mediaId: 'm1', completed: true });
		expect(invoke).toHaveBeenCalledWith('desktop_set_completed', { mediaId: 'm1', completed: true });
	});

	it('never sends an owner or a path to the study commands', async () => {
		// O dono é fixo no Rust: o webview só envia dados da mídia. Nenhum argumento pode
		// carregar `owner`/`ownerId`, um caminho ou uma biblioteca.
		vi.mocked(invoke).mockResolvedValue({ media_id: 'm1', position_seconds: 1, completed: false, updated_at: 2, recorded: true });
		const desktop = new DesktopStub();

		await desktop.studyProgress('m1');
		await desktop.recordPosition('m1', 1);
		await desktop.setCompleted('m1', false);
		vi.mocked(invoke).mockResolvedValue({ items: [] });
		await desktop.recentStudy(3);

		const calls = vi.mocked(invoke).mock.calls;
		expect(calls.map((call) => call[0])).toEqual(['desktop_study_progress', 'desktop_record_position', 'desktop_set_completed', 'desktop_recent_study']);
		const allowedKeys = [['mediaId'], ['mediaId', 'positionSeconds'], ['completed', 'mediaId'], ['limit']];
		calls.forEach(([, args], index) => {
			expect(Object.keys(args as object).sort()).toEqual([...allowedKeys[index]].sort());
			expect(JSON.stringify(args)).not.toContain('/');
		});
	});

	it('loads the recent history through the desktop IPC command with and without a limit', async () => {
		vi.mocked(invoke).mockResolvedValue({ items: [{ media_id: 'm1', position_seconds: 7, completed: false, updated_at: 9, recorded: true }] });

		const recent = await new DesktopStub().recentStudy(6);

		expect(invoke).toHaveBeenCalledWith('desktop_recent_study', { limit: 6 });
		expect(recent.items[0]).toMatchObject({ mediaId: 'm1', positionSeconds: 7, recorded: true });

		vi.mocked(invoke).mockResolvedValue({ items: [] });
		await expect(new DesktopStub().recentStudy()).resolves.toEqual({ items: [] });
		expect(invoke).toHaveBeenLastCalledWith('desktop_recent_study', {});
	});

	it('maps each stable desktop study error code to a sanitized message', async () => {
		const desktop: Transport = new DesktopStub();

		vi.mocked(invoke).mockRejectedValue({ code: 'media_not_found', message: '/mnt/cursos secretos/aula.mp4' });
		await expect(desktop.studyProgress('m1')).rejects.toThrow('Mídia não encontrada.');
		await expect(desktop.recordPosition('m1', 1)).rejects.toThrow('Mídia não encontrada.');
		await expect(desktop.setCompleted('m1', true)).rejects.toThrow('Mídia não encontrada.');

		vi.mocked(invoke).mockRejectedValue({ code: 'invalid_position', message: 'posição -1 em /mnt/cursos' });
		await expect(desktop.recordPosition('m1', -1)).rejects.toThrow('Posição inválida.');

		vi.mocked(invoke).mockRejectedValue({ code: 'study_unavailable', message: 'falha em C:\\segredo\\cursos' });
		await expect(desktop.studyProgress('m1')).rejects.toThrow('Não foi possível sincronizar o progresso de estudo.');
		await expect(desktop.recentStudy(2)).rejects.toThrow('Não foi possível sincronizar o progresso de estudo.');

		vi.mocked(invoke).mockRejectedValue({ code: 'database_unavailable', message: 'banco em C:\\segredo' });
		await expect(desktop.recordPosition('m1', 1)).rejects.toThrow('banco de dados local indisponível');
	});

	it('falls back to a stable message for an unknown desktop study error', async () => {
		const desktop: Transport = new DesktopStub();

		vi.mocked(invoke).mockRejectedValue({ code: 'algo_desconhecido', message: 'CAMINHO privado /mnt/cursos' });
		await expect(desktop.studyProgress('m1')).rejects.toThrow('Não foi possível ler o progresso.');
		await expect(desktop.recordPosition('m1', 1)).rejects.toThrow('Não foi possível salvar o progresso.');
		await expect(desktop.setCompleted('m1', true)).rejects.toThrow('Não foi possível atualizar a conclusão.');
		await expect(desktop.recentStudy()).rejects.toThrow('Não foi possível carregar o histórico de estudo.');

		vi.mocked(invoke).mockRejectedValue({ message: 'sem código /etc/shadow' });
		await expect(desktop.studyProgress('m1')).rejects.toThrow('Não foi possível ler o progresso.');
	});

	it('does not trust a malformed desktop study payload', async () => {
		// Como no catálogo, um payload fora do contrato cai na mensagem sanitizada da
		// chamada: nada do conteúdo recebido chega à interface.
		vi.mocked(invoke).mockResolvedValue({ media_id: 'm1', position_seconds: 'zero', completed: false, recorded: true });
		await expect(new DesktopStub().studyProgress('m1')).rejects.toThrow('Não foi possível ler o progresso.');
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
