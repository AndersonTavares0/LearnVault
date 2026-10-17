import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { findLesson } from './router';
import { desktopMediaUrl, DesktopStub, HttpTransport, normalizeCatalog, normalizeRecentStudy, normalizeStudyProgress, normalizeUser, STUDY_NOT_AVAILABLE, type Transport } from './transport';

// O Tauri só é carregado no desktop: fora dele o módulo é substituído pelo mock, de modo
// que a suíte nunca toca `window.__TAURI_INTERNALS__`.
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

describe('transport contract', () => {
	it('exposes the HTTP transport seam without routing assumptions', () => {
		const methods: (keyof Transport)[] = ['me', 'login', 'logout', 'catalog', 'importCourse', 'importCoursePath', 'removeCourse', 'rescan', 'updateCredentials', 'studyProgress', 'recordPosition', 'setCompleted', 'recentStudy', 'mediaUrl'];
		expect(methods).toHaveLength(14);
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
		const catalog = normalizeCatalog({ complete: true, loose_files: [], categories: [{ title: 'Física', path: '', role: 'course', course_id: 'course-1', files: [], loose_files: [{ media_id: 'loose', name: 'nota.pdf', path: 'nota.pdf', size: 3, kind: 'material', availability: 'missing', unverified: true }], children: [{ title: 'Módulo', path: 'mod', role: 'module', files: [], loose_files: [], children: [{ title: 'Aula', path: 'mod/aula', role: 'lesson', files: [{ media_id: 'm1', name: 'aula.mp4', path: 'mod/aula.mp4', size: 10, kind: 'media', availability: 'available', unverified: false }, { media_id: 's1', name: 'aula.vtt', path: 'mod/aula.vtt', size: 2, kind: 'subtitle', availability: 'available', unverified: false }], loose_files: [], children: [] }] }] }] });
		expect(catalog.complete).toBe(true);
		expect(catalog.nodes[0]).toMatchObject({ title: 'Física', role: 'course', courseId: 'course-1' });
		expect(catalog.nodes[0].looseFiles[0]).toMatchObject({ mediaId: 'loose', path: 'nota.pdf', availability: 'missing', unverified: true });
		expect(catalog.nodes[0].children[0].children[0].files[0].path).toBe('mod/aula.mp4');
		expect(catalog.nodes[0].children[0].children[0].files[0].mediaId).toBe('m1');
		expect(catalog.nodes[0].children[0].children[0].files[1].kind).toBe('subtitle');
		// Nós internos não carregam identidade de curso: ela vive apenas no topo.
		expect(catalog.nodes[0].children[0].courseId).toBeUndefined();
	});
	it('accepts an empty Rust catalog without course wrappers', () => {
		const catalog = normalizeCatalog({ complete: true, categories: [], loose_files: [{ name: 'nota.pdf', path: 'nota.pdf', size: 3, kind: 'material' }] });
		expect(catalog.nodes).toEqual([]);
		expect(catalog.looseFiles[0].path).toBe('nota.pdf');
	});
	it('rejects a node with a non-string course identity', () => {
		expect(() => normalizeCatalog({ complete: true, loose_files: [], categories: [{ title: 'Curso', path: '', role: 'course', course_id: 42, files: [], loose_files: [], children: [] }] })).toThrow('Resposta inválida do servidor');
	});
	it('keeps shallow recursive lessons and files addressable by path', () => {
		const catalog = normalizeCatalog({ complete: true, loose_files: [], nodes: [{ title: 'Categoria', path: 'cat', role: 'category', files: [{ name: 'cat.pdf', path: 'cat/cat.pdf', size: 1, kind: 'material' }], loose_files: [], children: [{ title: 'Aula', path: 'cat/aula', role: 'lesson', files: [{ name: 'aula.mp4', path: 'cat/aula.mp4', size: 4, kind: 'media' }], loose_files: [], children: [] }] }] });
		expect(catalog.nodes[0].children[0].path).toBe('cat/aula');
		expect(catalog.nodes[0].children[0].files[0].path).toBe('cat/aula.mp4');
	});
	it('keeps the Go tree and material files compatible', () => {
		const catalog = normalizeCatalog({ categories: [{ title: 'Categoria', courses: [{ title: 'Curso', path: 'cat/curso', modules: [{ title: 'Módulo', lessons: [{ title: 'Aula', path: 'aula', media: [], subtitles: [{ name: 'legenda.vtt', path: 'legenda.vtt', size: 1 }], materials: [{ name: 'slide.pdf', path: 'slide.pdf', size: 2 }] }] }] }] }] });
		const course = catalog.nodes[0];
		expect(course.role).toBe('course');
		expect(course.courseId).toBe('cat/curso');
		expect(course.children[0].children[0].files.map((file) => file.path)).toEqual(['legenda.vtt', 'slide.pdf']);
	});
	it('projects a realistic Go catalog into course nodes addressable by the SPA', () => {
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
		// A interface organiza por cursos: cada curso do formato Go vira um nó de topo.
		expect(catalog.nodes.length).toBeGreaterThan(0);
		const course = catalog.nodes[0];
		expect(course.role).toBe('course');
		expect(course.courseId).toBe('programming/go');
		expect(course.children[0].children[0].path).toBe('programming/go/basics/first');
		expect(catalog.looseFiles[0].path).toBe('intro.pdf');
		// The player resolves the lesson from the canonical nodes collection via findLesson.
		const lesson = findLesson(catalog, 'programming/go', 'programming/go/basics/first');
		expect(lesson?.media[0]).toMatchObject({ path: 'programming/go/basics/first/video.mp4', kind: 'media' });
		expect(lesson?.subtitles[0]).toMatchObject({ path: 'programming/go/basics/first/video.vtt', kind: 'subtitle' });
		expect(lesson?.materials[0]).toMatchObject({ path: 'programming/go/basics/first/notes.pdf', kind: 'material' });
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
		expect(catalog.nodes[0].title).toBe('Curso');
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
});
