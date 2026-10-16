// O `invoke` do Tauri entra apenas no bundle desktop: no bundle HTTP o `DesktopStub` é
// eliminado por dead-code elimination junto com este import, de modo que nem o Tauri entra
// na SPA web nem um `fetch` entra no desktop.
import { invoke } from '@tauri-apps/api/core';

export type User = { userId: string; username: string; role: string; isAdmin: boolean; mustChangeCredentials: boolean };
export type FileEntry = { name: string; path: string; size: number; mediaId?: string; kind?: string; availability?: string; unverified?: boolean };
export type Lesson = { title: string; path: string; media: FileEntry[]; subtitles: FileEntry[]; materials: FileEntry[]; availability?: string; unverified?: boolean };
export type CatalogNode = { title: string; path: string; role: string; files: FileEntry[]; looseFiles: FileEntry[]; children: CatalogNode[] };
export type ProjectedModule = { title: string; lessons: Lesson[]; looseFiles: FileEntry[] };
export type ProjectedCourse = { title: string; modules: ProjectedModule[]; looseFiles: FileEntry[] };
export type ProjectedCategory = { title: string; courses: ProjectedCourse[]; looseFiles: FileEntry[] };
export type Catalog = { complete: boolean; looseFiles: FileEntry[]; nodes: CatalogNode[]; categories: ProjectedCategory[] };
export type PlatformConfig = { configured: boolean };
// A seleção nativa da raiz da biblioteca é específica do desktop: no HTTP o servidor já
// conhece a própria biblioteca e nenhum caminho cruza o frontend.
export type TransportKind = 'http' | 'desktop';
export type StudyProgress = { mediaId: string; positionSeconds: number; completed: boolean; updatedAt: number | null; recorded: boolean };
export type RecentStudy = { items: StudyProgress[] };
export const STUDY_NOT_AVAILABLE = 'study_not_available';

export interface Transport {
	me(): Promise<User | null>;
	login(username: string, password: string): Promise<User>;
	logout(): Promise<void>;
	config(): Promise<PlatformConfig>;
	catalog(): Promise<Catalog>;
	chooseLibrary(): Promise<PlatformConfig>;
	rescan(): Promise<void>;
	updateCredentials(username: string, password: string, currentPassword?: string): Promise<User>;
	studyProgress(mediaId: string): Promise<StudyProgress>;
	recordPosition(mediaId: string, positionSeconds: number): Promise<StudyProgress>;
	setCompleted(mediaId: string, completed: boolean): Promise<StudyProgress>;
	recentStudy(limit?: number): Promise<RecentStudy>;
	mediaUrl(path: string): string | null;
}

type RustFile = { media_id?: string; name?: string; path?: string; size?: number; kind?: string; availability?: string; unverified?: boolean };
type RustNode = { title?: string; path?: string; role?: string; files?: RustFile[]; loose_files?: RustFile[]; children?: RustNode[] };
type RustCatalog = { complete?: boolean; categories?: RustNode[]; nodes?: RustNode[]; loose_files?: RustFile[]; looseFiles?: RustFile[] };
type GoFile = { name?: string; path?: string; size?: number; media_id?: string; kind?: string; availability?: string; unverified?: boolean };
type GoCatalog = { categories?: unknown; loose_files?: unknown; looseFiles?: unknown };

function invalidPayload(message: string): Error { return new Error(`Resposta inválida do servidor: ${message}`); }

export function fileStatus(file: FileEntry): string {
	return [file.availability === 'missing' ? 'ausente' : '', file.unverified ? 'não verificado' : ''].filter(Boolean).join(' · ');
}

function normalizeFile(value: RustFile | GoFile): FileEntry {
	if (typeof value.name !== 'string' || typeof value.path !== 'string' || typeof value.size !== 'number') throw invalidPayload('arquivo sem nome, caminho ou tamanho.');
	return { name: value.name, path: value.path, size: value.size, mediaId: value.media_id, kind: value.kind, availability: value.availability, unverified: value.unverified };
}

function lesson(title: string, path: string, files: FileEntry[]): Lesson {
	return {
		title, path,
		media: files.filter((file) => file.kind === 'media'),
		subtitles: files.filter((file) => file.kind === 'subtitle'),
		materials: files.filter((file) => file.kind === 'material')
	};
}

function nodeFiles(node: CatalogNode): FileEntry[] { return [...node.files, ...node.looseFiles]; }

function goFiles(value: unknown): FileEntry[] {
	if (!Array.isArray(value)) throw invalidPayload('lista de arquivos Go inválida.');
	return value.map((file) => normalizeFile(file as GoFile));
}

function goLesson(value: unknown): Lesson {
	if (!value || typeof value !== 'object') throw invalidPayload('aula Go inválida.');
	const item = value as { title?: unknown; path?: unknown; media?: unknown; subtitles?: unknown; materials?: unknown };
	if (typeof item.title !== 'string' || typeof item.path !== 'string') throw invalidPayload('aula Go sem título ou caminho.');
	return { title: item.title, path: item.path, media: goFiles(item.media), subtitles: goFiles(item.subtitles), materials: goFiles(item.materials) };
}

function rustLesson(node: CatalogNode): Lesson {
	const files = nodeFiles(node);
	return { ...lesson(node.title, node.path, files), availability: files.find((file) => file.availability)?.availability, unverified: files.some((file) => file.unverified) };
}

function normalizeRustNode(node: RustNode): CatalogNode {
	if (typeof node.title !== 'string' || typeof node.path !== 'string' || typeof node.role !== 'string' || !Array.isArray(node.files ?? []) || !Array.isArray(node.loose_files ?? []) || !Array.isArray(node.children ?? [])) throw invalidPayload('node Rust incompleto.');
	return { title: node.title, path: node.path, role: node.role, files: (node.files ?? []).map(normalizeFile), looseFiles: (node.loose_files ?? []).map(normalizeFile), children: (node.children ?? []).map(normalizeRustNode) };
}

function projectedModule(node: CatalogNode): ProjectedModule {
	return { title: node.title, looseFiles: node.role === 'lesson' ? [] : [...node.files, ...node.looseFiles], lessons: node.role === 'lesson' ? [rustLesson(node)] : node.children.filter((child) => child.role === 'lesson').map(rustLesson) };
}

function projectRust(nodes: CatalogNode[]): Catalog['categories'] {
	return nodes.filter((node) => node.role === 'category').map((category) => ({
		title: category.title,
		looseFiles: [...category.files, ...category.looseFiles],
		courses: category.children.filter((node) => node.role === 'course').map((course) => ({
			title: course.title,
			looseFiles: [...course.files, ...course.looseFiles],
			modules: course.children.filter((node) => node.role === 'module' || node.role === 'lesson').map(projectedModule)
		})).concat(category.children.filter((node) => node.role === 'lesson').map((node) => ({ title: category.title, looseFiles: [], modules: [projectedModule(node)] })))
	}));
}

function normalizeRustCatalog(value: RustCatalog): Catalog {
	const rawNodes = value.nodes ?? value.categories;
	if (!Array.isArray(rawNodes)) throw invalidPayload('catálogo Rust sem nodes.');
	const looseFiles = value.loose_files ?? value.looseFiles;
	if (!Array.isArray(looseFiles)) throw invalidPayload('catálogo Rust sem loose_files.');
	const nodes = rawNodes.map(normalizeRustNode);
	return { complete: value.complete === true, looseFiles: looseFiles.map(normalizeFile), nodes, categories: projectRust(nodes) };
}

function goTitle(value: unknown, fallback: string): string { return typeof value === 'string' ? value : fallback; }

function tagKind(files: FileEntry[], kind: string): FileEntry[] { return files.map((file) => ({ ...file, kind: file.kind ?? kind })); }

function goLessonNode(value: unknown): CatalogNode {
	const item = goLesson(value);
	const files = [...tagKind(item.media, 'media'), ...tagKind(item.subtitles, 'subtitle'), ...tagKind(item.materials, 'material')];
	return { title: item.title, path: item.path, role: 'lesson', files, looseFiles: [], children: [] };
}

function goModuleNode(value: unknown): CatalogNode {
	if (!value || typeof value !== 'object') throw invalidPayload('módulo Go inválido.');
	const item = value as { title?: unknown; path?: unknown; lessons?: unknown };
	if (!Array.isArray(item.lessons)) throw invalidPayload('módulo Go sem lessons.');
	return { title: goTitle(item.title, 'Módulo'), path: goTitle(item.path, ''), role: 'module', files: [], looseFiles: [], children: item.lessons.map(goLessonNode) };
}

function goCourseNode(value: unknown): CatalogNode {
	if (!value || typeof value !== 'object') throw invalidPayload('curso Go inválido.');
	const item = value as { title?: unknown; path?: unknown; modules?: unknown };
	if (!Array.isArray(item.modules)) throw invalidPayload('curso Go sem modules.');
	return { title: goTitle(item.title, 'Curso'), path: goTitle(item.path, ''), role: 'course', files: [], looseFiles: [], children: item.modules.map(goModuleNode) };
}

function goCategoryNode(value: unknown): CatalogNode {
	if (!value || typeof value !== 'object') throw invalidPayload('categoria Go inválida.');
	const item = value as { title?: unknown; path?: unknown; courses?: unknown };
	if (!Array.isArray(item.courses)) throw invalidPayload('categoria Go sem courses.');
	return { title: goTitle(item.title, 'Categoria'), path: goTitle(item.path, ''), role: 'category', files: [], looseFiles: [], children: item.courses.map(goCourseNode) };
}

// Go serializes the projected category/course/module/lesson tree and omits root `kind`/`nodes`;
// normalize it into the same canonical recursive `nodes` collection Rust already provides so the
// SPA renders one shape, and derive `categories` from it to avoid a second parallel tree.
function normalizeGoCatalog(value: GoCatalog): Catalog {
	if (!Array.isArray(value.categories)) throw invalidPayload('catálogo Go sem categories.');
	const looseFiles = goFiles(value.loose_files ?? value.looseFiles ?? []);
	const nodes = value.categories.map(goCategoryNode);
	return { complete: true, looseFiles, nodes, categories: projectRust(nodes) };
}

export function normalizeUser(value: unknown): User {
	if (!value || typeof value !== 'object' || typeof (value as { username?: unknown }).username !== 'string') throw invalidPayload('usuário sem username.');
	const user = value as { user_id?: unknown; username: string; role?: unknown; is_admin?: unknown; must_change_credentials?: unknown };
	return { userId: typeof user.user_id === 'string' || typeof user.user_id === 'number' ? String(user.user_id) : '', username: user.username, isAdmin: user.is_admin === true || user.role === 'admin', role: user.is_admin === true || user.role === 'admin' ? 'admin' : 'user', mustChangeCredentials: user.must_change_credentials === true };
}

export function normalizeConfig(value: unknown): PlatformConfig {
	if (!value || typeof value !== 'object' || typeof (value as { configured?: unknown }).configured !== 'boolean') throw invalidPayload('configuração da plataforma inválida.');
	return { configured: (value as { configured: boolean }).configured };
}

export function normalizeStudyProgress(value: unknown): StudyProgress {
	if (!value || typeof value !== 'object') throw invalidPayload('progresso de estudo ausente.');
	const progress = value as { media_id?: unknown; position_seconds?: unknown; completed?: unknown; updated_at?: unknown; recorded?: unknown };
	if (typeof progress.media_id !== 'string' || typeof progress.position_seconds !== 'number' || typeof progress.completed !== 'boolean' || typeof progress.recorded !== 'boolean') throw invalidPayload('progresso de estudo incompleto.');
	return { mediaId: progress.media_id, positionSeconds: progress.position_seconds, completed: progress.completed, updatedAt: typeof progress.updated_at === 'number' ? progress.updated_at : null, recorded: progress.recorded };
}

export function normalizeRecentStudy(value: unknown): RecentStudy {
	if (!value || typeof value !== 'object' || !Array.isArray((value as { items?: unknown }).items)) throw invalidPayload('histórico de estudo inválido.');
	return { items: (value as { items: unknown[] }).items.map(normalizeStudyProgress) };
}

export function normalizeCatalog(value: unknown): Catalog {
	if (!value || typeof value !== 'object') throw invalidPayload('catálogo ausente.');
	const payload = value as { categories?: unknown; nodes?: unknown; loose_files?: unknown; looseFiles?: unknown };
	if (Array.isArray(payload.nodes)) return normalizeRustCatalog(value as RustCatalog);
	if (Array.isArray(payload.categories) && payload.categories.every((item) => item && typeof item === 'object' && ('courses' in item || 'children' in item))) {
		if (payload.categories.some((item) => 'courses' in (item as object))) return normalizeGoCatalog(value as GoCatalog);
		return normalizeRustCatalog(value as RustCatalog);
	}
	throw invalidPayload('formato de catálogo desconhecido.');
}

export class HttpTransport implements Transport {
	async me() { const response = await fetch('/api/me'); return response.ok ? normalizeUser(await response.json()) : null; }
	async login(username: string, password: string) {
		const response = await fetch('/api/login', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ username, password }) });
		if (!response.ok) throw new Error(response.status === 401 ? 'Usuário ou senha incorretos.' : 'Não foi possível entrar.');
		return normalizeUser(await response.json());
	}
	async logout() { await fetch('/api/logout', { method: 'POST' }); }
	config() { return Promise.resolve({ configured: true }); }
	chooseLibrary() { return Promise.reject(new Error('A biblioteca do servidor é definida no servidor.')); }
	async catalog() { const response = await fetch('/api/catalog'); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar o catálogo.'); return normalizeCatalog(await response.json()); }
	async rescan() { const response = await fetch('/api/admin/catalog/rescan', { method: 'POST' }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 409 ? 'Já existe um rescan em andamento.' : 'Não foi possível indexar a biblioteca.'); }
	async updateCredentials(username: string, password: string, currentPassword = password) { const response = await fetch('/api/account/credentials', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ current_password: currentPassword, username, password }) }); if (!response.ok) throw new Error(response.status === 422 ? 'Usuário ou senha inválidos.' : response.status === 401 ? 'Senha atual incorreta.' : 'Não foi possível atualizar as credenciais.'); return normalizeUser(await response.json()); }
	async studyProgress(mediaId: string) { const response = await fetch(`/api/study/progress?media_id=${encodeURIComponent(mediaId)}`); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 404 ? 'Mídia não encontrada.' : 'Não foi possível ler o progresso.'); return normalizeStudyProgress(await response.json()); }
	async recordPosition(mediaId: string, positionSeconds: number) { const response = await fetch('/api/study/position', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ media_id: mediaId, position_seconds: positionSeconds }) }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 404 ? 'Mídia não encontrada.' : response.status === 422 ? 'Posição inválida.' : 'Não foi possível salvar o progresso.'); return normalizeStudyProgress(await response.json()); }
	async setCompleted(mediaId: string, completed: boolean) { const response = await fetch('/api/study/completed', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ media_id: mediaId, completed }) }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 404 ? 'Mídia não encontrada.' : 'Não foi possível atualizar a conclusão.'); return normalizeStudyProgress(await response.json()); }
	async recentStudy(limit?: number) { const url = limit === undefined ? '/api/study/recent' : `/api/study/recent?limit=${limit}`; const response = await fetch(url); if (!response.ok) throw response.status === 404 ? Object.assign(new Error('Estudo não disponível neste servidor.'), { code: STUDY_NOT_AVAILABLE }) : new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar o histórico.'); return normalizeRecentStudy(await response.json()); }
	mediaUrl(path: string) { return `/api/media?path=${encodeURIComponent(path)}`; }
}

// Códigos estáveis de erro IPC e a mensagem sanitizada correspondente. Um código
// desconhecido ou ausente nunca repassa o texto recebido: cai no fallback da chamada,
// para que nenhuma mensagem arbitrária (potencialmente com caminhos) chegue à interface.
const DESKTOP_ERROR_MESSAGES: Record<string, string> = {
	catalog_unavailable: 'catálogo indisponível',
	database_unavailable: 'banco de dados local indisponível',
	library_root_invalid: 'A pasta escolhida não é uma biblioteca válida.',
	library_config_unavailable: 'Não foi possível salvar a configuração do aplicativo.',
	library_not_configured: 'Nenhuma biblioteca está configurada.',
	rescan_in_progress: 'Já existe uma indexação em andamento.',
	rescan_unavailable: 'Não foi possível indexar a biblioteca.',
	study_unavailable: 'Não foi possível sincronizar o progresso de estudo.',
	media_not_found: 'Mídia não encontrada.',
	invalid_position: 'Posição inválida.'
};

// O comando devolve o erro como objeto serializado `{ code, message }`, não como `Error`.
function desktopError(cause: unknown, fallback: string): Error {
	const payload = cause as { code?: unknown } | null;
	if (payload && typeof payload === 'object' && typeof payload.code === 'string') {
		const known = DESKTOP_ERROR_MESSAGES[payload.code];
		if (known) return new Error(known);
	}
	return new Error(fallback);
}

export class DesktopStub implements Transport {
	me() { return Promise.resolve(null); }
	login() { return Promise.reject(new Error('Transporte desktop ainda não implementado.')); }
	logout() { return Promise.resolve(); }
	async config() { try { return normalizeConfig(await invoke<unknown>('desktop_config')); } catch (cause) { throw desktopError(cause, 'Não foi possível ler a configuração do aplicativo.'); } }
	// O webview apenas dispara o seletor nativo no Rust; nenhum caminho é enviado nem
	// devolvido. Cancelar preserva a configuração anterior.
	async chooseLibrary() { try { return normalizeConfig(await invoke<unknown>('desktop_select_library')); } catch (cause) { throw desktopError(cause, 'Não foi possível configurar a biblioteca.'); } }
	async catalog() { try { return normalizeCatalog(await invoke<unknown>('desktop_catalog')); } catch (cause) { throw desktopError(cause, 'Não foi possível carregar o catálogo.'); } }
	// O rescan desktop é sempre explícito: o webview só o dispara a pedido do usuário. A
	// resposta é um resumo sanitizado que não interessa ao shell, que recarrega o catálogo.
	async rescan() { try { await invoke<unknown>('desktop_rescan'); } catch (cause) { throw desktopError(cause, 'Não foi possível indexar a biblioteca.'); } }
	updateCredentials() { return Promise.reject(new Error('Transporte desktop ainda não implementado.')); }
	// O progresso de estudo do desktop é o do dono local fixo, escolhido no Rust: o webview
	// envia apenas o `mediaId`, a posição, a conclusão e o limite, e nunca um `owner_id` nem
	// qualquer caminho. Os comandos devolvem o mesmo contrato snake_case do HTTP, então a
	// normalização é a mesma e nenhum `fetch` entra no bundle desktop.
	async studyProgress(mediaId: string) { try { return normalizeStudyProgress(await invoke<unknown>('desktop_study_progress', { mediaId })); } catch (cause) { throw desktopError(cause, 'Não foi possível ler o progresso.'); } }
	async recordPosition(mediaId: string, positionSeconds: number) { try { return normalizeStudyProgress(await invoke<unknown>('desktop_record_position', { mediaId, positionSeconds })); } catch (cause) { throw desktopError(cause, 'Não foi possível salvar o progresso.'); } }
	async setCompleted(mediaId: string, completed: boolean) { try { return normalizeStudyProgress(await invoke<unknown>('desktop_set_completed', { mediaId, completed })); } catch (cause) { throw desktopError(cause, 'Não foi possível atualizar a conclusão.'); } }
	// Sem limite informado, o comando usa o padrão do contrato; por isso o argumento é
	// omitido em vez de enviado como `undefined`.
	async recentStudy(limit?: number) { try { return normalizeRecentStudy(await invoke<unknown>('desktop_recent_study', limit === undefined ? {} : { limit })); } catch (cause) { throw desktopError(cause, 'Não foi possível carregar o histórico de estudo.'); } }
	// O desktop ainda não entrega a mídia da biblioteca: sem fonte não há reprodução, e o
	// player mantém a barra de estudo fechada (ver `studySyncEnabled`).
	mediaUrl(_path: string) { return null; }
}

export const transportKind: TransportKind = __LEARNVAULT_TRANSPORT__ === 'desktop' ? 'desktop' : 'http';
export const transport: Transport = transportKind === 'desktop' ? new DesktopStub() : new HttpTransport();
