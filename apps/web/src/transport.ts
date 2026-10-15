export type User = { username: string; role: string };
export type Catalog = { categories: { title: string; courses: { title: string; modules: { title: string; lessons: Lesson[] }[] }[] }[] };
export type Lesson = { title: string; path: string; media: FileEntry[]; subtitles: FileEntry[]; materials: FileEntry[] };
export type FileEntry = { name: string; path: string; size: number };

export interface Transport {
	me(): Promise<User | null>;
	login(username: string, password: string): Promise<User>;
	logout(): Promise<void>;
	catalog(): Promise<Catalog>;
	rescan(): Promise<void>;
	mediaUrl(path: string): string;
}

export class HttpTransport implements Transport {
	async me() { const response = await fetch('/api/me'); return response.ok ? response.json() : null; }
	async login(username: string, password: string) {
		const response = await fetch('/api/login', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ username, password }) });
		if (!response.ok) throw new Error(response.status === 401 ? 'Usuário ou senha incorretos.' : 'Não foi possível entrar.');
		return response.json();
	}
	async logout() { await fetch('/api/logout', { method: 'POST' }); }
	async catalog() { const response = await fetch('/api/catalog'); if (!response.ok) throw new Error('Não foi possível carregar o catálogo.'); return response.json(); }
	async rescan() {
		const response = await fetch('/api/admin/catalog/rescan', { method: 'POST' });
		if (!response.ok) throw new Error(response.status === 409 ? 'Já existe um rescan em andamento.' : 'Não foi possível indexar a biblioteca.');
	}
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
