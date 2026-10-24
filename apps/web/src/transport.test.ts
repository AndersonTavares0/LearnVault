import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { findLesson } from './router';
import { desktopCoverUrl, desktopMediaUrl, DesktopStub, HttpTransport, lessonFromNode, MAX_COVER_BYTES, normalizeCatalog, normalizeRecentStudy, normalizeStudyProgress, normalizeUpdateStatus, normalizeUser, STUDY_NOT_AVAILABLE, CATALOG_LIMITS, type Transport } from './transport';

// O Tauri só é carregado no desktop: fora dele o módulo é substituído pelo mock, de modo
// que a suíte nunca toca `window.__TAURI_INTERNALS__`.
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('transport contract', () => {
	  it('reads study analytics from HTTP', async () => {
    const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ tracked_count: 4, completed_count: 2, total_position_seconds: 99, last_activity_at: 123 }), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);
    await expect(new HttpTransport().studyAnalytics()).resolves.toEqual({ trackedCount: 4, completedCount: 2, totalPositionSeconds: 99, lastActivityAt: 123 });
    expect(fetchMock).toHaveBeenCalledWith('/api/study/analytics');
    vi.unstubAllGlobals();
	  });
	it('loads and persists Pomodoro state over HTTP', async () => {
		const idle = { status: 'idle', duration_seconds: 1500, elapsed_seconds: 0, updated_at: 0 };
		const fetchMock = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify(idle), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify({ ...idle, status: 'running' }), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify({ ...idle, status: 'paused', elapsed_seconds: 12 }), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify(idle), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);
		const api = new HttpTransport();
		await expect(api.pomodoro()).resolves.toMatchObject({ status: 'idle' });
		await expect(api.startPomodoro(1500)).resolves.toMatchObject({ status: 'running' });
		await expect(api.pausePomodoro(12)).resolves.toMatchObject({ status: 'paused', elapsedSeconds: 12 });
		await expect(api.resetPomodoro()).resolves.toMatchObject({ status: 'idle' });
		expect(fetchMock.mock.calls.map(([url, options]) => [url, options?.method])).toEqual([
			['/api/study/pomodoro', undefined], ['/api/study/pomodoro', 'POST'], ['/api/study/pomodoro', 'PUT'], ['/api/study/pomodoro', 'DELETE']
		]);
		vi.unstubAllGlobals();
	});
  it('uses the desktop IPC commands for analytics, notes and Pomodoro', async () => {
		const note = { note_id: 4, media_id: 'm1', text: 'nota', position_seconds: null, created_at: 10, updated_at: 10 };
		vi.mocked(invoke).mockResolvedValueOnce({ tracked_count: 1, completed_count: 0, total_position_seconds: 5, last_activity_at: 10 })
			.mockResolvedValueOnce({ items: [note], truncated: false })
			.mockResolvedValueOnce(note)
			.mockResolvedValueOnce(undefined)
			.mockResolvedValueOnce({ status: 'idle', duration_seconds: 1500, elapsed_seconds: 0, updated_at: 0 })
			.mockResolvedValueOnce({ status: 'running', duration_seconds: 1500, elapsed_seconds: 0, updated_at: 10 });
		const transport = new DesktopStub();
		await expect(transport.studyAnalytics()).resolves.toMatchObject({ trackedCount: 1 });
		await expect(transport.listNotes('m1')).resolves.toMatchObject({ items: [{ noteId: 4 }] });
		await expect(transport.createNote('m1', 'nota', null)).resolves.toMatchObject({ noteId: 4 });
		await expect(transport.deleteNote(4)).resolves.toBeUndefined();
		await expect(transport.pomodoro()).resolves.toMatchObject({ status: 'idle', durationSeconds: 1500 });
		await expect(transport.startPomodoro(1500)).resolves.toMatchObject({ status: 'running' });
		expect(invoke).toHaveBeenNthCalledWith(1, 'desktop_study_analytics');
		expect(invoke).toHaveBeenNthCalledWith(2, 'desktop_list_notes', { mediaId: 'm1' });
		expect(invoke).toHaveBeenNthCalledWith(3, 'desktop_create_note', { mediaId: 'm1', text: 'nota', positionSeconds: null });
		expect(invoke).toHaveBeenNthCalledWith(4, 'desktop_delete_note', { noteId: 4 });
		expect(invoke).toHaveBeenNthCalledWith(5, 'desktop_pomodoro');
		expect(invoke).toHaveBeenNthCalledWith(6, 'desktop_start_pomodoro', { durationSeconds: 1500 });
	  });
	it('reads and mutates owner-scoped study notes through the HTTP API', async () => {
		const note = { note_id: 9, media_id: 'm/1', text: 'nota', position_seconds: 12, created_at: 100, updated_at: 101 };
		const fetchMock = vi.fn()
			.mockResolvedValueOnce(new Response(JSON.stringify({ items: [note], truncated: false }), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify({ items: [note], truncated: true }), { status: 200 }))
			.mockResolvedValueOnce(new Response(JSON.stringify(note), { status: 201 }))
			.mockResolvedValueOnce(new Response(JSON.stringify({ ...note, text: 'edit' }), { status: 200 }))
			.mockResolvedValueOnce(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetchMock);
		const transport = new HttpTransport();
		expect(await transport.listNotes('m/1')).toMatchObject({ items: [{ noteId: 9, mediaId: 'm/1' }], truncated: false });
		expect(await transport.listAllNotes()).toMatchObject({ truncated: true });
		expect(await transport.createNote('m/1', 'nota', 12)).toMatchObject({ noteId: 9 });
		expect(await transport.updateNote(9, 'edit', null)).toMatchObject({ text: 'edit' });
		await expect(transport.deleteNote(9)).resolves.toBeUndefined();
		expect(fetchMock.mock.calls.map(([url]) => url)).toEqual([
			'/api/study/notes?media_id=m%2F1', '/api/study/notes/all', '/api/study/notes',
			'/api/study/notes/9', '/api/study/notes/9'
		]);
		expect(fetchMock.mock.calls[2][1]).toMatchObject({ method: 'POST', body: JSON.stringify({ media_id: 'm/1', text: 'nota', position_seconds: 12 }) });
		expect(fetchMock.mock.calls[3][1]).toMatchObject({ method: 'PUT', body: JSON.stringify({ text: 'edit', position_seconds: null }) });
		expect(fetchMock.mock.calls[4][1]).toMatchObject({ method: 'DELETE' });
		vi.unstubAllGlobals();
	});
	it('exposes the HTTP transport seam without routing assumptions', () => {
		const methods: (keyof Transport)[] = ['me', 'login', 'logout', 'catalog', 'importCourse', 'importCoursePath', 'removeCourse', 'rescan', 'updateCredentials', 'studyProgress', 'recordPosition', 'setCompleted', 'recentStudy', 'listNotes', 'listAllNotes', 'createNote', 'updateNote', 'deleteNote', 'studyAnalytics', 'pomodoro', 'startPomodoro', 'pausePomodoro', 'resetPomodoro', 'studySummary', 'checkUpdate', 'mediaUrl', 'coverUrl', 'importCourseCover', 'removeCourseCover'];
		expect(methods).toHaveLength(29);
	});
	it('does not offer native folder selection over HTTP', async () => {
		await expect(new HttpTransport().importCourse()).rejects.toThrow('desktop é quem escolhe pastas');
	});
	it('registers and removes a server course through the admin endpoints', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);

		await new HttpTransport().importCoursePath('C:\\cursos\\Física');
		expect(fetchMock).toHaveBeenCalledWith('/api/admin/courses', expect.objectContaining({ method: 'POST', body: JSON.stringify({ path: 'C:\\cursos\\Física' }) }));

		await new HttpTransport().removeCourse('course one');
		expect(fetchMock).toHaveBeenLastCalledWith('/api/admin/courses/course%20one', { method: 'DELETE' });
		vi.unstubAllGlobals();
	});
	it('maps the stable server import errors to actionable messages', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ error: 'course_already_registered' }), { status: 400 }));
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().importCoursePath('C:\\cursos')).rejects.toThrow('Este curso já está importado.');

		fetchMock.mockResolvedValue(new Response(JSON.stringify({ error: 'invalid_course_path' }), { status: 400 }));
		await expect(new HttpTransport().importCoursePath('C:\\nao-existe')).rejects.toThrow('A pasta informada não existe ou não pode ser lida no servidor.');

		fetchMock.mockResolvedValue(new Response(null, { status: 503 }));
		await expect(new HttpTransport().importCoursePath('C:\\cursos')).rejects.toThrow('O servidor está ocupado; tente de novo em instantes.');
		vi.unstubAllGlobals();
	});
	it('troca credenciais temporárias pelo contrato autenticado', async () => {
		const user = { user_id: 'u1', username: 'novo', is_admin: true, must_change_credentials: false };
		const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify(user), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().updateCredentials('novo', 'senha suficientemente longa')).resolves.toMatchObject({ username: 'novo', mustChangeCredentials: false });
		expect(fetchMock).toHaveBeenCalledWith('/api/account/credentials', expect.objectContaining({ method: 'POST' }));
		vi.unstubAllGlobals();
	});
	it('builds media and caption URLs from the course identity and the path on HTTP', () => {
		const transport = new HttpTransport();
		expect(transport.mediaUrl({ name: 'aula.mp4', path: 'módulo/aula.mp4', size: 1, kind: 'media', mediaId: 'm1' }, 'course-1')).toBe('/api/media?course=course-1&path=m%C3%B3dulo%2Faula.mp4');
		expect(transport.mediaUrl({ name: 'aula.vtt', path: 'legendas/aula.vtt', size: 1, kind: 'subtitle', mediaId: 's1' }, 'course one')).toBe('/api/media?course=course%20one&path=legendas%2Faula.vtt');
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
			expect(transport.mediaUrl({ name: 'aula.mp4', path: 'Curso privado/aula.mp4', size: 1, kind: 'media', mediaId: 'v1' })).toBe('learnvault-media://localhost/v1');
			expect(transport.mediaUrl({ name: 'aula.vtt', path: 'Curso privado/aula.vtt', size: 1, kind: 'subtitle', mediaId: 's1' })).toBe('learnvault-media://localhost/s1');
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
			expect(transport.mediaUrl({ name: 'aula.mp4', path: 'Curso privado/aula.mp4', size: 1, kind: 'media', mediaId: 'v1' })).toBe('http://learnvault-media.localhost/v1');
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
	it('posts the admin rescan endpoint and maps a conflict and an empty registry', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 204 }));
		vi.stubGlobal('fetch', fetchMock);
		await new HttpTransport().rescan();
		expect(fetchMock).toHaveBeenCalledWith('/api/admin/catalog/rescan', { method: 'POST' });
		fetchMock.mockResolvedValue(new Response(null, { status: 409 }));
		await expect(new HttpTransport().rescan()).rejects.toThrow('Já existe um rescan em andamento.');
		fetchMock.mockResolvedValue(new Response(null, { status: 400 }));
		await expect(new HttpTransport().rescan()).rejects.toThrow('Nenhum curso foi importado ainda.');
		vi.unstubAllGlobals();
	});
	it('normalizes the Rust user and recursive catalog contract', () => {
		expect(normalizeUser({ user_id: 'u1', username: 'admin', is_admin: true, must_change_credentials: true })).toEqual({ userId: 'u1', username: 'admin', role: 'admin', isAdmin: true, mustChangeCredentials: true });
		const catalog = normalizeCatalog({ complete: true, loose_files: [], categories: [{ title: 'Física', path: '', role: 'course', course_id: 'course-1', files: [], loose_files: [{ media_id: 'loose', name: 'nota.pdf', path: 'nota.pdf', size: 3, kind: 'material', availability: 'missing', unverified: true }], children: [{ title: 'Módulo', path: 'mod', role: 'module', files: [], loose_files: [], children: [{ title: 'Aula', path: 'mod/aula', role: 'lesson', files: [{ media_id: 'm1', name: 'aula.mp4', path: 'mod/aula.mp4', size: 10, mtime_seconds: 1_760_000_000, kind: 'media', availability: 'available', unverified: false }, { media_id: 's1', name: 'aula.vtt', path: 'mod/aula.vtt', size: 2, kind: 'subtitle', availability: 'available', unverified: false }], loose_files: [], children: [] }] }] }] });
		expect(catalog.complete).toBe(true);
		expect(catalog.nodes[0]).toMatchObject({ title: 'Física', role: 'course', courseId: 'course-1' });
		expect(catalog.nodes[0].looseFiles[0]).toMatchObject({ mediaId: 'loose', path: 'nota.pdf', availability: 'missing', unverified: true });
		expect(catalog.nodes[0].children[0].children[0].files[0].path).toBe('mod/aula.mp4');
		expect(catalog.nodes[0].children[0].children[0].files[0].mediaId).toBe('m1');
		expect(catalog.nodes[0].children[0].children[0].files[0].mtimeSeconds).toBe(1_760_000_000);
		expect(catalog.nodes[0].children[0].children[0].files[1].kind).toBe('subtitle');
		// Nós internos não carregam identidade de curso: ela vive apenas no topo.
		expect(catalog.nodes[0].children[0].courseId).toBeUndefined();
	});
	it('rejects invalid media revision metadata when present', () => {
		const file = { media_id: 'm1', name: 'aula.mp4', path: 'aula.mp4', size: 10, kind: 'media' };
		for (const mtime_seconds of [-1, 1.5, Number.MAX_SAFE_INTEGER + 1, '1760000000']) {
			expect(() => normalizeCatalog({ complete: true, categories: [], loose_files: [{ ...file, mtime_seconds }] })).toThrow('Resposta inválida do servidor');
		}
	});
	it('requires media_id on canonical root loose files and preserves them when present', () => {
		// Um arquivo solto de raiz em payload canônico segue a mesma regra dos nós: identidade
		// obrigatória. Sem `media_id`, o payload é recusado em vez de aceitar um arquivo órfão.
		expect(() => normalizeCatalog({ complete: true, categories: [], loose_files: [{ name: 'nota.pdf', path: 'nota.pdf', size: 3, kind: 'material' }] })).toThrow('Resposta inválida do servidor');
		const catalog = normalizeCatalog({ complete: true, categories: [], loose_files: [{ media_id: 'loose-1', name: 'nota.pdf', path: 'nota.pdf', size: 3, kind: 'material' }] });
		expect(catalog.looseFiles[0]).toMatchObject({ mediaId: 'loose-1', path: 'nota.pdf' });
	});
	it('accepts an empty Rust catalog without course wrappers', () => {
		// O contrato canônico não usa envelope de categoria: `categories: []` vale como árvore
		// vazia, sem criar nós de curso artificiais. O arquivo solto canônico carrega identidade.
		const catalog = normalizeCatalog({ complete: true, categories: [], loose_files: [{ media_id: 'loose-1', name: 'nota.pdf', path: 'nota.pdf', size: 3, kind: 'material' }] });
		expect(catalog.nodes).toEqual([]);
		expect(catalog.looseFiles[0].path).toBe('nota.pdf');
	});
	it('rejects a node with a non-string course identity', () => {
		expect(() => normalizeCatalog({ complete: true, loose_files: [], categories: [{ title: 'Curso', path: '', role: 'course', course_id: 42, files: [], loose_files: [], children: [] }] })).toThrow('Resposta inválida do servidor');
	});
	it('keeps shallow recursive lessons and files addressable by path', () => {
		const catalog = normalizeCatalog({ complete: true, loose_files: [], nodes: [{ title: 'Categoria', path: 'cat', role: 'category', files: [{ media_id: 'cat-pdf', name: 'cat.pdf', path: 'cat/cat.pdf', size: 1, kind: 'material' }], loose_files: [], children: [{ title: 'Aula', path: 'cat/aula', role: 'lesson', files: [{ media_id: 'aula-mp4', name: 'aula.mp4', path: 'cat/aula.mp4', size: 4, kind: 'media' }], loose_files: [], children: [] }] }] });
		expect(catalog.nodes[0].children[0].path).toBe('cat/aula');
		expect(catalog.nodes[0].children[0].files[0].path).toBe('cat/aula.mp4');
	});
	it('rejeita catálogo legado no formato Go sem o marcador complete', () => {
		expect(() => normalizeCatalog({ categories: [{ title: 'Categoria', courses: [{ title: 'Curso', path: 'cat/curso', modules: [{ title: 'Módulo', lessons: [{ title: 'Aula', path: 'aula', media: [], subtitles: [], materials: [] }] }] }] }] })).toThrow('Resposta inválida do servidor: campo complete ausente ou inválido.');
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
		expect(catalog.nodes.map((node) => node.title)).toEqual(['DaNodes']);
	});
	it('accepts a canonical payload that omits root loose files and prefers nodes over the legacy categories', () => {
		// O contrato canônico serializa a árvore em `nodes` e omite `loose_files` na raiz quando
		// não há arquivos soltos (omitempty): a ausência vale como lista vazia. Com o marcador
		// `complete` a árvore canônica vence a `categories` legada, que é ignorada.
		const catalog = normalizeCatalog({
			complete: true,
			nodes: [{
				title: 'Programação', path: 'programming', role: 'category', files: [], loose_files: [],
				children: [{
					title: 'Go', path: 'programming/go', role: 'group', files: [], loose_files: [],
					children: [{
						title: 'Primeiros passos', path: 'programming/go/first', role: 'lesson',
						files: [{ media_id: 'first-mp4', name: 'video.mp4', path: 'programming/go/first/video.mp4', size: 42, kind: 'media' }],
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
		expect(findLesson(catalog, 'programming', 'programming/go/first')?.media[0].path).toBe('programming/go/first/video.mp4');
	});
	it('falls back to populated legacy categories when nodes is empty', () => {
		// Um `nodes` vazio não pode esconder a árvore legada entregue em `categories`.
		const catalog = normalizeCatalog({
			complete: true,
			nodes: [],
			loose_files: [],
			categories: [{ title: 'Legado', path: 'cat', role: 'category', files: [], loose_files: [], children: [{ title: 'Aula', path: 'cat/aula', role: 'lesson', files: [{ media_id: 'a-mp4', name: 'a.mp4', path: 'cat/aula.mp4', size: 4, kind: 'media' }], loose_files: [], children: [] }] }]
		});
		expect(catalog.nodes[0].title).toBe('Legado');
		expect(catalog.nodes[0].children[0].path).toBe('cat/aula');
	});
	it('rejects an explicit non-boolean complete marker instead of treating it as Go', () => {
		// `complete` presente precisa ser booleano: um `null`/string/objeto não pode ser lido
		// como "sem marcador" e cair no fallback do Go.
		const categories = [{ title: 'Legado', path: 'cat', courses: [{ title: 'C', path: 'c', modules: [] }] }];
		const nodes = [{ title: 'Cat', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }];
		for (const complete of [null, 'true', 'false', 1, {}, []]) {
			expect(() => normalizeCatalog({ complete, nodes, categories })).toThrow('Resposta inválida do servidor');
		}
	});
	it('rejeita payload de catálogo quando o marcador complete está ausente', () => {
		const nodes = [{ title: 'Cat', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }];
		expect(() => normalizeCatalog({ nodes })).toThrow('Resposta inválida do servidor: campo complete ausente ou inválido.');
		expect(() => normalizeCatalog({ categories: [] })).toThrow('Resposta inválida do servidor: campo complete ausente ou inválido.');
	});
	it('routes a canonical payload strictly through Rust even when categories look like Go courses', () => {
		// `complete: true` + `nodes` vazio + `categories` em forma Go: o payload é canônico, então
		// a forma legada do Go não pode servir de fallback. As `courses` não são nós canônicos e o
		// payload é recusado em vez de virar um catálogo Go.
		expect(() => normalizeCatalog({ complete: true, nodes: [], categories: [{ title: 'Legado', path: 'cat', courses: [{ title: 'C', path: 'c', modules: [] }] }] })).toThrow('Resposta inválida do servidor');
		// Um `nodes` canônico vazio, sem categorias, continua sendo um catálogo vazio válido.
		const empty = normalizeCatalog({ complete: true, nodes: [] });
		expect(empty.nodes).toEqual([]);
		expect(empty.looseFiles).toEqual([]);
	});
	it('rejects Go-only courses fields in a canonical payload instead of yielding empty content', () => {
		// Mesmo com `role` presente (o que faria o nó "parsear"), um `courses` só-do-Go em um
		// payload canônico sem nós seria descartado silenciosamente, virando conteúdo vazio.
		expect(() => normalizeCatalog({
			complete: true,
			nodes: [],
			categories: [{ title: 'Legado', path: 'cat', role: 'category', courses: [{ title: 'C', path: 'c', modules: [] }] }]
		})).toThrow('Resposta inválida do servidor');
		// Um `categories` recursivo (Rust legado) sem `courses` continua aceito.
		const legacy = normalizeCatalog({ complete: true, categories: [{ title: 'Legado', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }] });
		expect(legacy.nodes[0].title).toBe('Legado');
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
		const catalog = normalizeCatalog({ complete: true, categories: [{ title: 'Legado', path: 'cat', role: 'category', files: [], loose_files: [], children: [] }] });
		expect(catalog.nodes[0].title).toBe('Legado');
	});
	it('preserves lessons through unknown group descendants in the canonical tree', () => {
		const catalog = normalizeCatalog({
			complete: true,
			nodes: [{
				title: 'Programação', path: 'programming', role: 'category', files: [], loose_files: [],
				children: [{
					title: 'Grupo', path: 'programming/grupo', role: 'group', files: [], loose_files: [],
					children: [{
						title: 'Básico', path: 'programming/grupo/basics', role: 'group', files: [], loose_files: [],
						children: [{ title: 'Aula', path: 'programming/grupo/basics/first', role: 'lesson', files: [{ media_id: 'v-mp4', name: 'v.mp4', path: 'programming/grupo/basics/first/v.mp4', size: 9, kind: 'media' }], loose_files: [], children: [] }]
					}]
				}]
			}]
		});
		expect(catalog.nodes[0].children[0].children[0].children[0].path).toBe('programming/grupo/basics/first');
		expect(lessonFromNode(catalog.nodes[0].children[0].children[0].children[0]).media.map((file) => file.path)).toEqual(['programming/grupo/basics/first/v.mp4']);
	});
	it('preserves lessons when a module sits directly under a category without a course level', () => {
		const catalog = normalizeCatalog({
			complete: true,
			loose_files: [],
			nodes: [{ title: 'Cat', path: 'Cat', role: 'category', files: [], loose_files: [], children: [{ title: 'Curso', path: 'Cat/Curso', role: 'module', files: [], loose_files: [], children: [{ title: 'Aula', path: 'Cat/Curso/Aula', role: 'lesson', files: [{ media_id: 'a-mp4', name: 'a.mp4', path: 'Cat/Curso/Aula/a.mp4', size: 1, kind: 'media' }], loose_files: [], children: [] }] }] }]
		});
		expect(catalog.nodes[0].children[0].children[0].path).toBe('Cat/Curso/Aula');
		expect(findLesson(catalog, 'Cat', 'Cat/Curso/Aula')?.media[0].path).toBe('Cat/Curso/Aula/a.mp4');
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
		// A árvore canônica vem de `nodes`; a `categories` legada do payload não é copiada.
		expect(catalog.nodes.map((node) => node.title)).toEqual(['DaNodes']);
	});
	it('keeps a node of an unknown role in the canonical tree instead of dropping it', () => {
		// Um papel desconhecido (por exemplo um `group` futuro) não pode sumir: a árvore
		// canônica o preserva com seus arquivos, e a projeção legada desce por ele.
		const catalog = normalizeCatalog({
			complete: true,
			nodes: [{
				title: 'Cat', path: 'cat', role: 'category', files: [], loose_files: [],
				children: [{
					title: 'Grupo', path: 'cat/grupo', role: 'group',
					files: [{ media_id: 'guia-pdf', name: 'guia.pdf', path: 'cat/grupo/guia.pdf', size: 2, kind: 'material' }],
					loose_files: [], children: []
				}]
			}]
		});
		expect(catalog.nodes[0].children[0].role).toBe('group');
		expect(catalog.nodes[0].children[0].files[0].path).toBe('cat/grupo/guia.pdf');
		// O arquivo do nó de papel desconhecido continua acessível: `lessonFromNode` o trata
		// como material (catch-all) em vez de descartá-lo.
		expect(lessonFromNode(catalog.nodes[0].children[0]).materials.map((file) => file.path)).toEqual(['cat/grupo/guia.pdf']);
	});
	it('keeps an unknown file kind instead of dropping it from the lesson projection', () => {
		// Um `kind` que a SPA ainda não conhece não pode fazer o arquivo desaparecer: a árvore
		// canônica preserva o valor original e a aula o trata como material (catch-all).
		const catalog = normalizeCatalog({
			complete: true,
			nodes: [{
				title: 'Cat', path: 'cat', role: 'category', files: [], loose_files: [],
				children: [{
					title: 'Aula', path: 'cat/aula', role: 'lesson',
					files: [{ media_id: 'extra-bin', name: 'extra.bin', path: 'cat/aula/extra.bin', size: 1, kind: 'bonus' }],
					loose_files: [], children: []
				}]
			}]
		});
		expect(catalog.nodes[0].children[0].files[0].kind).toBe('bonus');
		const lesson = lessonFromNode(catalog.nodes[0].children[0]);
		expect(lesson.media).toEqual([]);
		expect(lesson.subtitles).toEqual([]);
		expect(lesson.materials.map((file) => file.path)).toEqual(['cat/aula/extra.bin']);
	});
	it('keeps a top-level node of an unknown role searchable instead of dropping it', () => {
		// Um papel de topo desconhecido não some do payload: a árvore canônica o mantém e a
		// busca por aula continua alcançando seus descendentes.
		const catalog = normalizeCatalog({
			complete: true,
			nodes: [{
				title: 'Coleção', path: 'colecao', role: 'collection', files: [], loose_files: [],
				children: [{
					title: 'Aula', path: 'colecao/aula', role: 'lesson',
					files: [{ media_id: 'm1', name: 'v.mp4', path: 'colecao/aula/v.mp4', size: 1, kind: 'media' }],
					loose_files: [], children: []
				}]
			}]
		});
		expect(catalog.nodes[0].role).toBe('collection');
		expect(findLesson(catalog, 'colecao', 'colecao/aula')?.media[0].path).toBe('colecao/aula/v.mp4');
		// Nada se perde com o papel desconhecido: a aula continua na árvore canônica.
		expect(catalog.nodes[0].children.map((child) => child.path)).toEqual(['colecao/aula']);
	});
	it('requires a non-empty media_id for canonical Rust node files', () => {
		// A árvore canônica Rust declara `media_id` obrigatório: um arquivo sem identidade não
		// é endereçável. `complete: bool` marca o contrato canônico (Rust/desktop).
		const bare = { name: 'aula.mp4', path: 'cat/aula.mp4', size: 1, kind: 'media' };
		expect(() => normalizeCatalog({ complete: true, nodes: [{ title: 'Cat', path: 'cat', role: 'category', files: [bare], loose_files: [], children: [] }] })).toThrow('Resposta inválida do servidor');
		const empty = { ...bare, media_id: '' };
		expect(() => normalizeCatalog({ complete: true, nodes: [{ title: 'Cat', path: 'cat', role: 'category', files: [], loose_files: [empty], children: [] }] })).toThrow('Resposta inválida do servidor');
		// A `categories` recursiva (fallback Rust quando `nodes` está vazio) também é canônica.
		expect(() => normalizeCatalog({ complete: true, nodes: [], categories: [{ title: 'Cat', path: 'cat', role: 'category', files: [bare], loose_files: [], children: [] }] })).toThrow('Resposta inválida do servidor');
	});
	it('rejects a catalog deeper than the depth limit instead of overflowing the stack', () => {
		let deep: Record<string, unknown> = { title: 'Aula', path: 'aula', role: 'lesson', files: [], loose_files: [], children: [] };
		for (let level = 0; level <= CATALOG_LIMITS.depth; level++) {
			deep = { title: 'Nó', path: `n${level}`, role: 'module', files: [], loose_files: [], children: [deep] };
		}
		expect(() => normalizeCatalog({ complete: true, nodes: [deep] })).toThrow('Resposta inválida do servidor');
	});
	it('rejects a catalog with more nodes than the limit without truncating', () => {
		const leaf = { title: 'Aula', path: 'aula', role: 'lesson', files: [], loose_files: [], children: [] };
		const children = Array.from({ length: CATALOG_LIMITS.nodes }, () => leaf);
		// Uma categoria mais o limite de filhos ultrapassa o orçamento total de nós.
		expect(() => normalizeCatalog({ complete: true, nodes: [{ title: 'Cat', path: 'cat', role: 'category', files: [], loose_files: [], children }] })).toThrow('Resposta inválida do servidor');
	});
	it('rejects a catalog with more files than the limit without truncating', () => {
		const file = { media_id: 'm', name: 'f', path: 'f', size: 1 };
		const files = Array.from({ length: CATALOG_LIMITS.files + 1 }, () => file);
		expect(() => normalizeCatalog({ complete: true, nodes: [{ title: 'Cat', path: 'cat', role: 'category', files, loose_files: [], children: [] }] })).toThrow('Resposta inválida do servidor');
	});
	it('rejects an explicit null root loose_files instead of treating it as absent', () => {
		expect(() => normalizeCatalog({ complete: true, nodes: [], categories: [], loose_files: null })).toThrow('Resposta inválida do servidor');
		expect(() => normalizeCatalog({ complete: true, nodes: [], looseFiles: null })).toThrow('Resposta inválida do servidor');
	});
	it('rejects a wrong-type or nested-array root loose_files', () => {
		expect(() => normalizeCatalog({ complete: true, nodes: [], loose_files: 'nao-lista' })).toThrow('Resposta inválida do servidor');
		expect(() => normalizeCatalog({ complete: true, nodes: [], loose_files: [[{ name: 'a.pdf', path: 'a.pdf', size: 1 }]] })).toThrow('Resposta inválida do servidor');
	});
	it('rejects conflicting snake_case and camelCase loose files', () => {
		expect(() => normalizeCatalog({ complete: true, nodes: [], loose_files: [], looseFiles: [{ name: 'a.pdf', path: 'a.pdf', size: 1 }] })).toThrow('conflitantes');
	});
	it('accepts equal snake_case and camelCase loose files deterministically', () => {
		const file = { media_id: 'm1', name: 'a.pdf', path: 'a.pdf', size: 1, kind: 'material' };
		const catalog = normalizeCatalog({ complete: true, nodes: [], loose_files: [file], looseFiles: [file] });
		expect(catalog.looseFiles[0].path).toBe('a.pdf');
	});
	it('rejects a present optional file field with the wrong type', () => {
		const base = { media_id: 'm1', name: 'a.pdf', path: 'a.pdf', size: 1 };
		const malformed = [
			{ ...base, media_id: 42 },
			{ ...base, media_id: null },
			{ ...base, kind: 7 },
			{ ...base, availability: false },
			{ ...base, unverified: 'sim' }
		];
		for (const file of malformed) {
			expect(() => normalizeCatalog({ complete: true, nodes: [], loose_files: [file] })).toThrow('Resposta inválida do servidor');
		}
		// O mesmo vale para um arquivo aninhado em um nó canônico.
		expect(() => normalizeCatalog({ complete: true, nodes: [{ title: 'C', path: 'c', role: 'category', files: [{ ...base, media_id: 1 }], loose_files: [], children: [] }] })).toThrow('Resposta inválida do servidor');
	});
	it('keeps a file whose optional fields are present and valid', () => {
		const catalog = normalizeCatalog({ complete: true, nodes: [], loose_files: [{ name: 'a.pdf', path: 'a.pdf', size: 5, media_id: 'm1', kind: 'material', availability: 'missing', unverified: true }] });
		expect(catalog.looseFiles[0]).toEqual({ name: 'a.pdf', path: 'a.pdf', size: 5, mediaId: 'm1', kind: 'material', availability: 'missing', unverified: true });
	});
	it('rejects a fractional, negative or unsafe file size', () => {
		const base = { media_id: 'm1', name: 'a.pdf', path: 'a.pdf' };
		for (const size of [1.5, -1, Number.MAX_SAFE_INTEGER + 1, Number.NaN]) {
			expect(() => normalizeCatalog({ complete: true, nodes: [], loose_files: [{ ...base, size }] })).toThrow('Resposta inválida do servidor');
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
	it('builds the manual cover URL from the course identity and its version on HTTP', () => {
		const transport = new HttpTransport();
		expect(transport.coverUrl('course-1', 1_760_000_000)).toBe('/api/courses/course-1/cover?v=1760000000');
		expect(transport.coverUrl('curso um', 42)).toBe('/api/courses/curso%20um/cover?v=42');
		// Sem capa publicada não existe URL manual: a interface cai na miniatura ou na capa
		// gerada pelo título em vez de apontar para uma imagem inexistente.
		expect(transport.coverUrl('course-1')).toBeNull();
		expect(transport.coverUrl('course-1', null)).toBeNull();
	});
	it('uploads the chosen cover to the admin endpoint as raw bytes', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ cover_version: 1_760_000_000 }), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);
		const file = new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], 'capa.png', { type: 'image/png' });

		await new HttpTransport().importCourseCover('course one', file);

		const [url, init] = fetchMock.mock.calls[0] as [string, RequestInit];
		expect(url).toBe('/api/admin/courses/course%20one/cover');
		expect(init.method).toBe('PUT');
		// O corpo é o arquivo cru: o tipo é decidido pelos bytes iniciais no servidor, então o
		// cliente não negocia nem declara formato.
		expect(init.body).toBe(file);
		vi.unstubAllGlobals();
	});
	it('maps the stable cover upload failures to actionable messages', async () => {
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		const file = new File([new Uint8Array([0x89])], 'capa.png', { type: 'image/png' });

		for (const [status, message] of [[413, 'A imagem é maior que 4 MiB.'], [415, 'A capa precisa ser PNG, JPEG ou WebP.'], [404, 'Curso não encontrado.'], [503, 'O servidor está ocupado; tente de novo em instantes.'], [500, 'Não foi possível salvar a capa do curso.'], [403, 'credentials_change_required']] as const) {
			fetchMock.mockResolvedValue(new Response(null, { status }));
			await expect(new HttpTransport().importCourseCover('course-1', file)).rejects.toThrow(message);
		}
		vi.unstubAllGlobals();
	});
	it('refuses a missing or oversized cover before touching the network', async () => {
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().importCourseCover('course-1')).rejects.toThrow('Escolha uma imagem para a capa.');
		const oversized = new File([new Uint8Array(1)], 'grande.png', { type: 'image/png' });
		Object.defineProperty(oversized, 'size', { value: MAX_COVER_BYTES + 1 });
		await expect(new HttpTransport().importCourseCover('course-1', oversized)).rejects.toThrow('A imagem é maior que 4 MiB.');
		expect(fetchMock).not.toHaveBeenCalled();
		vi.unstubAllGlobals();
	});
	it('removes the cover and reports the server flag', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(JSON.stringify({ removed: true }), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);

		await expect(new HttpTransport().removeCourseCover('course one')).resolves.toBe(true);
		expect(fetchMock).toHaveBeenLastCalledWith('/api/admin/courses/course%20one/cover', { method: 'DELETE' });

		fetchMock.mockResolvedValue(new Response(JSON.stringify({ removed: false }), { status: 200 }));
		await expect(new HttpTransport().removeCourseCover('course one')).resolves.toBe(false);

		// Uma resposta sem corpo JSON não pode ser confundida com uma remoção concluída.
		fetchMock.mockResolvedValue(new Response(null, { status: 204 }));
		await expect(new HttpTransport().removeCourseCover('course one')).resolves.toBe(false);

		fetchMock.mockResolvedValue(new Response(null, { status: 403 }));
		await expect(new HttpTransport().removeCourseCover('course one')).rejects.toThrow('credentials_change_required');
		fetchMock.mockResolvedValue(new Response(null, { status: 404 }));
		await expect(new HttpTransport().removeCourseCover('course one')).rejects.toThrow('Curso não encontrado.');
		vi.unstubAllGlobals();
	});
	it('normalizes the cover version exposed for a course node', () => {
		const catalog = normalizeCatalog({ complete: true, loose_files: [], nodes: [{ title: 'Curso', path: '', role: 'course', course_id: 'course-1', cover_version: 1_760_000_000, files: [], loose_files: [], children: [] }] });
		expect(catalog.nodes[0].coverVersion).toBe(1_760_000_000);
		// Um curso sem capa não declara o campo.
		const without = normalizeCatalog({ complete: true, loose_files: [], nodes: [{ title: 'Curso', path: '', role: 'course', course_id: 'course-1', files: [], loose_files: [], children: [] }] });
		expect(without.nodes[0].coverVersion).toBeUndefined();
		// Um valor presente precisa ser um segundo Unix inteiro não negativo.
		for (const cover_version of [-1, 1.5, '1760000000', Number.MAX_SAFE_INTEGER + 1, null]) {
			expect(() => normalizeCatalog({ complete: true, loose_files: [], nodes: [{ title: 'Curso', path: '', role: 'course', course_id: 'course-1', cover_version, files: [], loose_files: [], children: [] }] })).toThrow('Resposta inválida do servidor');
		}
	});
	it('normalizes the update status and keeps a nullable error', () => {
		expect(normalizeUpdateStatus({ current: '0.3.0', enabled: true, available: true, latest: '0.4.0', releasedAt: '2026-10-17T12:00:00Z', url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0', checkedAt: '1760702400', error: null })).toEqual({
			current: '0.3.0', enabled: true, available: true, latest: '0.4.0', releasedAt: '2026-10-17T12:00:00Z', url: 'https://github.com/AndersonTavares0/LearnVault/releases/tag/v0.4.0', checkedAt: '1760702400', error: null
		});
		// Uma falha chega como 200 com `error` preenchido e sem versão nova.
		expect(normalizeUpdateStatus({ current: '0.3.0', enabled: true, available: false, latest: null, releasedAt: null, url: null, checkedAt: null, error: 'não foi possível contatar o GitHub' })).toMatchObject({ available: false, latest: null, error: 'não foi possível contatar o GitHub' });
		// Um campo presente com tipo incorreto é recusado em vez de vazar para a interface.
		expect(() => normalizeUpdateStatus({ current: '0.3.0', enabled: true, available: false, latest: 42 })).toThrow('Resposta inválida do servidor');
		expect(() => normalizeUpdateStatus({ current: 0.3, enabled: true, available: false })).toThrow('Resposta inválida do servidor');
		expect(() => normalizeUpdateStatus(null)).toThrow('Resposta inválida do servidor');
	});
	it('checks the update endpoint and only forces a fresh check when asked', async () => {
		const fetchMock = vi.fn().mockImplementation(() => new Response(JSON.stringify({ current: '0.3.0', enabled: true, available: false, latest: null, releasedAt: null, url: null, checkedAt: '1', error: null }), { status: 200 }));
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().checkUpdate()).resolves.toMatchObject({ current: '0.3.0', enabled: true, available: false });
		expect(fetchMock).toHaveBeenLastCalledWith('/api/update');
		await new HttpTransport().checkUpdate(true);
		expect(fetchMock).toHaveBeenLastCalledWith('/api/update?force=1');
		vi.unstubAllGlobals();
	});
	it('maps a rejected update check to an actionable message', async () => {
		const fetchMock = vi.fn().mockResolvedValue(new Response(null, { status: 403 }));
		vi.stubGlobal('fetch', fetchMock);
		await expect(new HttpTransport().checkUpdate()).rejects.toThrow('credentials_change_required');
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
		expect(catalog.nodes[0].title).toBe('Curso');
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

		expect(catalog.nodes.map((node) => node.title)).toEqual(['Nós']);
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
		vi.mocked(invoke).mockRejectedValue({ code: 'no_courses_registered', message: '/mnt/cursos sem cursos' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Nenhum curso foi importado ainda.');
		vi.mocked(invoke).mockRejectedValue({ code: 'rescan_unavailable', message: 'falha em C:\\segredo\\cursos' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Não foi possível indexar os cursos.');
	});

	it('falls back to a stable message for an unknown desktop rescan error code', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'algo_desconhecido', message: 'CAMINHO privado /mnt/cursos' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Não foi possível indexar os cursos.');
		vi.mocked(invoke).mockRejectedValue({ message: 'sem código /etc/shadow' });
		await expect(new DesktopStub().rescan()).rejects.toThrow('Não foi possível indexar os cursos.');
	});

	it('imports a course through the native picker command without sending a path', async () => {
		vi.mocked(invoke).mockResolvedValue({ configured: true });
		await expect(new DesktopStub().importCourse()).resolves.toBeUndefined();
		// O webview apenas dispara o seletor nativo; nenhum caminho cruza o IPC.
		expect(invoke).toHaveBeenCalledWith('desktop_import_course');
	});

	it('never offers server path import on the desktop', async () => {
		await expect(new DesktopStub().importCoursePath('C:\\cursos')).rejects.toThrow('seletor nativo');
	});

	it('maps course import errors to sanitized messages without the path', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'course_path_invalid', message: '/mnt/cursos não é válido' });
		await expect(new DesktopStub().importCourse()).rejects.toThrow('A pasta escolhida não é um curso válido.');
		vi.mocked(invoke).mockRejectedValue({ code: 'courses_config_unavailable', message: 'falha em C:\\segredo' });
		await expect(new DesktopStub().importCourse()).rejects.toThrow('Não foi possível salvar a configuração do aplicativo.');
	});

	it('removes a course through the IPC command sending only the opaque identity', async () => {
		vi.mocked(invoke).mockResolvedValue({ configured: true });
		await expect(new DesktopStub().removeCourse('course-1')).resolves.toBeUndefined();
		expect(invoke).toHaveBeenCalledWith('desktop_remove_course', { courseId: 'course-1' });

		vi.mocked(invoke).mockRejectedValue({ code: 'courses_config_unavailable', message: 'falha em C:\\segredo' });
		await expect(new DesktopStub().removeCourse('course-1')).rejects.toThrow('Não foi possível salvar a configuração do aplicativo.');
	});

	it('builds the cover URL from the opaque course identity per platform', () => {
		const linux = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)';
		const windows = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0';
		expect(desktopCoverUrl('course-1', linux)).toBe('learnvault-cover://localhost/course-1');
		expect(desktopCoverUrl('course-1', windows)).toBe('http://learnvault-cover.localhost/course-1');
		expect(desktopCoverUrl('course um', linux)).toBe('learnvault-cover://localhost/course%20um');
	});

	it('addresses the desktop cover by identity and version, never by path', () => {
		const transport = new DesktopStub();
		vi.stubGlobal('navigator', { userAgent: 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko)' });
		try {
			const url = transport.coverUrl('course-1', 42)!;
			expect(url).toBe('learnvault-cover://localhost/course-1?v=42');
			expect(url).not.toContain('Acervo privado');
			expect(transport.coverUrl('course-1')).toBeNull();
			expect(transport.coverUrl('course-1', null)).toBeNull();
		} finally {
			vi.unstubAllGlobals();
		}
	});

	it('imports and removes the cover through the native picker commands without sending a file', async () => {
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);

		vi.mocked(invoke).mockResolvedValue({ cover_version: 1_760_000_000 });
		// O arquivo escolhido fica no Rust: nada é enviado pelo IPC.
		await expect(new DesktopStub().importCourseCover('course-1', new File([new Uint8Array([0x89])], 'capa.png'))).resolves.toBeUndefined();
		expect(invoke).toHaveBeenCalledWith('desktop_import_course_cover', { courseId: 'course-1' });
		expect(fetchMock).not.toHaveBeenCalled();

		vi.mocked(invoke).mockResolvedValue({ removed: true });
		await expect(new DesktopStub().removeCourseCover('course-1')).resolves.toBe(true);
		expect(invoke).toHaveBeenLastCalledWith('desktop_remove_course_cover', { courseId: 'course-1' });

		vi.mocked(invoke).mockResolvedValue({ removed: false });
		await expect(new DesktopStub().removeCourseCover('course-1')).resolves.toBe(false);
		vi.mocked(invoke).mockResolvedValue(null);
		await expect(new DesktopStub().removeCourseCover('course-1')).resolves.toBe(false);
		vi.unstubAllGlobals();
	});

	it('maps the cover IPC errors to sanitized messages without the path', async () => {
		vi.mocked(invoke).mockRejectedValue({ code: 'course_not_found', message: 'curso em /mnt/cursos não está registrado' });
		await expect(new DesktopStub().importCourseCover('course-1')).rejects.toThrow('Curso não encontrado.');
		vi.mocked(invoke).mockRejectedValue({ code: 'invalid_cover', message: 'arquivo /mnt/segredo/capa.bmp' });
		await expect(new DesktopStub().importCourseCover('course-1')).rejects.toThrow('A capa precisa ser PNG, JPEG ou WebP de até 4 MiB.');
		vi.mocked(invoke).mockRejectedValue({ code: 'cover_unavailable', message: 'falha ao gravar em C:\\segredo' });
		await expect(new DesktopStub().removeCourseCover('course-1')).rejects.toThrow('Não foi possível atualizar a capa do curso.');
		vi.mocked(invoke).mockRejectedValue({ code: 'algo_desconhecido', message: 'CAMINHO privado /mnt/cursos' });
		await expect(new DesktopStub().removeCourseCover('course-1')).rejects.toThrow('Não foi possível remover a capa do curso.');
		vi.mocked(invoke).mockRejectedValue({ message: 'sem código /etc/shadow' });
		await expect(new DesktopStub().importCourseCover('course-1')).rejects.toThrow('Não foi possível salvar a capa do curso.');
	});
	it('reports the update check as disabled on the desktop without touching fetch', async () => {
		const fetchMock = vi.fn();
		vi.stubGlobal('fetch', fetchMock);
		const desktop: Transport = new DesktopStub();
		await expect(desktop.checkUpdate(true)).resolves.toMatchObject({ enabled: false, available: false, error: null });
		expect(fetchMock).not.toHaveBeenCalled();
		expect(invoke).not.toHaveBeenCalled();
		vi.unstubAllGlobals();
	});
});
