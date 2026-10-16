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

export interface Transport {
	me(): Promise<User | null>;
	login(username: string, password: string): Promise<User>;
	logout(): Promise<void>;
	config(): Promise<PlatformConfig>;
	catalog(): Promise<Catalog>;
	rescan(): Promise<void>;
	updateCredentials(username: string, password: string, currentPassword?: string): Promise<User>;
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
	async catalog() { const response = await fetch('/api/catalog'); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar o catálogo.'); return normalizeCatalog(await response.json()); }
	async rescan() { const response = await fetch('/api/admin/catalog/rescan', { method: 'POST' }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 409 ? 'Já existe um rescan em andamento.' : 'Não foi possível indexar a biblioteca.'); }
	async updateCredentials(username: string, password: string, currentPassword = password) { const response = await fetch('/api/account/credentials', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ current_password: currentPassword, username, password }) }); if (!response.ok) throw new Error(response.status === 422 ? 'Usuário ou senha inválidos.' : response.status === 401 ? 'Senha atual incorreta.' : 'Não foi possível atualizar as credenciais.'); return normalizeUser(await response.json()); }
	mediaUrl(path: string) { return `/api/media?path=${encodeURIComponent(path)}`; }
}

// Códigos estáveis de erro IPC e a mensagem sanitizada correspondente. Um código
// desconhecido ou ausente nunca repassa o texto recebido: cai no fallback da chamada,
// para que nenhuma mensagem arbitrária (potencialmente com caminhos) chegue à interface.
const DESKTOP_ERROR_MESSAGES: Record<string, string> = {
	catalog_unavailable: 'catálogo indisponível',
	database_unavailable: 'banco de dados local indisponível'
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
	async catalog() { try { return normalizeCatalog(await invoke<unknown>('desktop_catalog')); } catch (cause) { throw desktopError(cause, 'Não foi possível carregar o catálogo.'); } }
	rescan() { return Promise.reject(new Error('Rescan desktop ainda não implementado.')); }
	updateCredentials() { return Promise.reject(new Error('Transporte desktop ainda não implementado.')); }
	mediaUrl(_path: string) { return null; }
}

export const transport: Transport = __LEARNVAULT_TRANSPORT__ === 'desktop' ? new DesktopStub() : new HttpTransport();
