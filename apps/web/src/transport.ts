export type User = { username: string; role: string };
export type FileEntry = { name: string; path: string; size: number; media_id?: string; kind?: string; availability?: string; unverified?: boolean };
export type Lesson = { title: string; path: string; media: FileEntry[]; subtitles: FileEntry[]; materials: FileEntry[]; availability?: string; unverified?: boolean };
export type Catalog = { categories: { title: string; courses: { title: string; modules: { title: string; lessons: Lesson[] }[] }[] }[] };

export interface Transport {
	me(): Promise<User | null>;
	login(username: string, password: string): Promise<User>;
	logout(): Promise<void>;
	catalog(): Promise<Catalog>;
	rescan(): Promise<void>;
	mediaUrl(path: string): string;
}

type RustFile = { media_id?: string; name?: string; path?: string; size?: number; kind?: string; availability?: string; unverified?: boolean };
type RustNode = { title?: string; path?: string; role?: string; files?: RustFile[]; loose_files?: RustFile[]; children?: RustNode[] };
type RustCatalog = { categories?: RustNode[]; loose_files?: RustFile[] };
type GoFile = { name?: string; path?: string; size?: number; media_id?: string; kind?: string; availability?: string; unverified?: boolean };

function invalidPayload(message: string): Error { return new Error(`Resposta inválida do servidor: ${message}`); }

function normalizeFile(value: RustFile | GoFile): FileEntry {
	if (typeof value.name !== 'string' || typeof value.path !== 'string' || typeof value.size !== 'number') throw invalidPayload('arquivo sem nome, caminho ou tamanho.');
	return { name: value.name, path: value.path, size: value.size, media_id: value.media_id, kind: value.kind, availability: value.availability, unverified: value.unverified };
}

function lesson(title: string, path: string, files: FileEntry[]): Lesson {
	return {
		title, path,
		media: files.filter((file) => file.kind === 'media'),
		subtitles: files.filter((file) => file.kind === 'subtitle'),
		materials: files.filter((file) => file.kind === 'material')
	};
}

function nodeFiles(node: RustNode): FileEntry[] {
	return [...(node.files ?? []), ...(node.loose_files ?? [])].map(normalizeFile);
}

function syntheticLesson(node: RustNode, files: FileEntry[]): Lesson | null {
	return files.length ? lesson(node.title ?? 'Arquivos', node.path ?? '', files) : null;
}

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

function rustModule(node: RustNode): { title: string; lessons: Lesson[] } {
	const lessons = (node.children ?? []).map((child) => rustLesson(child));
	const own = syntheticLesson(node, nodeFiles(node));
	if (own) lessons.unshift(own);
	return { title: node.title ?? 'Módulo', lessons };
}

function rustLesson(node: RustNode): Lesson {
	const files = nodeFiles(node);
	return { ...(syntheticLesson(node, files) ?? lesson(node.title ?? 'Aula', node.path ?? '', [])), availability: files.find((file) => file.availability)?.availability, unverified: files.some((file) => file.unverified) };
}

function rustCourse(node: RustNode): { title: string; modules: { title: string; lessons: Lesson[] }[] } {
	const modules = (node.children ?? []).map(rustModule);
	const own = syntheticLesson(node, nodeFiles(node));
	if (own) modules.unshift({ title: node.title ?? 'Arquivos', lessons: [own] });
	return { title: node.title ?? 'Curso', modules };
}

function normalizeRustCatalog(value: RustCatalog): Catalog {
	if (!Array.isArray(value.categories)) throw invalidPayload('catálogo Rust sem categories.');
	const categories = value.categories.map((node) => {
		const courses = (node.children ?? []).map(rustCourse);
		const own = syntheticLesson(node, [...(node.files ?? []), ...(node.loose_files ?? [])].map(normalizeFile));
		if (own) courses.unshift({ title: node.title ?? 'Arquivos', modules: [{ title: node.title ?? 'Arquivos', lessons: [own] }] });
		return { title: node.title ?? 'Categoria', courses };
	});
	if (Array.isArray(value.loose_files) && value.loose_files.length) {
		const files = value.loose_files.map(normalizeFile);
		categories.unshift({ title: 'Arquivos avulsos', courses: [{ title: 'Arquivos avulsos', modules: [{ title: 'Arquivos avulsos', lessons: [lesson('Arquivos avulsos', '', files)] }] }] });
	}
	return { categories };
}

export function normalizeUser(value: unknown): User {
	if (!value || typeof value !== 'object' || typeof (value as { username?: unknown }).username !== 'string') throw invalidPayload('usuário sem username.');
	const user = value as { username: string; role?: unknown; is_admin?: unknown };
	return { username: user.username, role: user.is_admin === true || user.role === 'admin' ? 'admin' : 'user' };
}

export function normalizeCatalog(value: unknown): Catalog {
	if (!value || typeof value !== 'object') throw invalidPayload('catálogo ausente.');
	const payload = value as { categories?: unknown; loose_files?: unknown };
	if (Array.isArray(payload.categories) && payload.categories.length === 0) return { categories: [] };
	if (Array.isArray(payload.categories) && payload.categories.every((item) => item && typeof item === 'object' && ('courses' in item || 'children' in item))) {
		if (payload.categories.some((item) => 'courses' in (item as object))) {
			return { categories: payload.categories.map((category) => {
				if (!category || typeof category !== 'object') throw invalidPayload('categoria Go inválida.');
				const currentCategory = category as { title?: unknown; courses?: unknown };
				if (!Array.isArray(currentCategory.courses)) throw invalidPayload('categoria Go sem courses.');
				return {
					title: typeof currentCategory.title === 'string' ? currentCategory.title : 'Categoria',
					courses: currentCategory.courses.map((course) => {
						if (!course || typeof course !== 'object') throw invalidPayload('curso Go inválido.');
						const currentCourse = course as { title?: unknown; modules?: unknown };
						if (!Array.isArray(currentCourse.modules)) throw invalidPayload('curso Go sem modules.');
						return {
							title: typeof currentCourse.title === 'string' ? currentCourse.title : 'Curso',
							modules: currentCourse.modules.map((module) => {
								if (!module || typeof module !== 'object') throw invalidPayload('módulo Go inválido.');
								const currentModule = module as { title?: unknown; lessons?: unknown };
								if (!Array.isArray(currentModule.lessons)) throw invalidPayload('módulo Go sem lessons.');
								return { title: typeof currentModule.title === 'string' ? currentModule.title : 'Módulo', lessons: currentModule.lessons.map(goLesson) };
							})
						};
					})
				};
			}) };
		}
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
	async catalog() { const response = await fetch('/api/catalog'); if (!response.ok) throw new Error('Não foi possível carregar o catálogo.'); return normalizeCatalog(await response.json()); }
	async rescan() { const response = await fetch('/api/admin/catalog/rescan', { method: 'POST' }); if (!response.ok) throw new Error(response.status === 409 ? 'Já existe um rescan em andamento.' : 'Não foi possível indexar a biblioteca.'); }
	mediaUrl(path: string) { return `/api/media?path=${encodeURIComponent(path)}`; }
}

/** Seam explícita para o build Tauri; IPC pertence à Fase 4. */
class DesktopStub implements Transport {
	me() { return Promise.resolve(null); }
	login() { return Promise.reject(new Error('Transporte desktop ainda não implementado.')); }
	logout() { return Promise.resolve(); }
	catalog() { return Promise.reject(new Error('Transporte desktop ainda não implementado.')); }
	rescan() { return Promise.reject(new Error('Rescan desktop ainda não implementado.')); }
	mediaUrl(path: string) { return path; }
}

export const transport: Transport = __LEARNVAULT_TRANSPORT__ === 'desktop' ? new DesktopStub() : new HttpTransport();
