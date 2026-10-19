// O `invoke` do Tauri entra apenas no bundle desktop: no bundle HTTP o `DesktopStub` é
// eliminado por dead-code elimination junto com este import, de modo que nem o Tauri entra
// na SPA web nem um `fetch` entra no desktop.
import { invoke } from '@tauri-apps/api/core';

export type User = { userId: string; username: string; role: string; isAdmin: boolean; mustChangeCredentials: boolean };
export type FileEntry = { name: string; path: string; size: number; mediaId?: string; mtimeSeconds?: number; kind?: string; availability?: string; unverified?: boolean };
export type Lesson = { title: string; path: string; media: FileEntry[]; subtitles: FileEntry[]; materials: FileEntry[]; availability?: string; unverified?: boolean };
// Cada nó de topo do catálogo é um curso com identidade opaca (`courseId`). Nós internos
// não carregam a identidade: a árvore inteira pertence ao curso do topo.
export type CatalogNode = { title: string; path: string; role: string; courseId?: string; files: FileEntry[]; looseFiles: FileEntry[]; children: CatalogNode[] };
export type Catalog = { complete: boolean; looseFiles: FileEntry[]; nodes: CatalogNode[] };
// A seleção nativa de uma pasta de curso é específica do desktop: no HTTP o servidor
// registra cursos por caminho e nenhum caminho cruza o frontend.
export type TransportKind = 'http' | 'desktop';
export type StudyProgress = { mediaId: string; positionSeconds: number; completed: boolean; updatedAt: number | null; recorded: boolean };
export type RecentStudy = { items: StudyProgress[] };
// Estado da verificação de atualização, tal como o servidor local devolve em `/api/update`.
// Uma falha de rede do servidor nunca vira erro HTTP: chega como `error` preenchido e
// `available: false`. `enabled: false` significa que a checagem está desligada por
// configuração (nenhuma chamada de rede acontece).
export type UpdateStatus = {
	current: string;
	enabled: boolean;
	available: boolean;
	latest: string | null;
	releasedAt: string | null;
	url: string | null;
	checkedAt: string | null;
	error: string | null;
};
export const STUDY_NOT_AVAILABLE = 'study_not_available';

// Protocolo customizado do desktop para mídia local. O webview aponta somente pelo
// `mediaId` autorizado; nenhum caminho cruza a fronteira do webview.
export const DESKTOP_MEDIA_SCHEME = 'learnvault-media';
export const DESKTOP_MEDIA_HOST = 'localhost';

// No Chromium embutido (WebView2 no Windows e o webview do Android) um esquema customizado
// do Tauri só é resolvido na forma `http://<esquema>.localhost`; o runtime reverte esse
// prefixo por requisição antes de chamar o handler, que continua vendo
// `learnvault-media://localhost/...`. No Linux (WebKitGTK) e no macOS (WKWebView) o esquema
// é resolvido direto. Sem o prefixo correto o Chromium não resolve `learnvault-media://` e a
// mídia não carrega. A detecção usa apenas o `userAgent` do webview atual, sem
// dependência nova.
export function desktopMediaUrl(mediaId: string, userAgent: string): string {
	const base = /Windows|Android/i.test(userAgent) ? `http://${DESKTOP_MEDIA_SCHEME}.localhost` : `${DESKTOP_MEDIA_SCHEME}://${DESKTOP_MEDIA_HOST}`;
	return `${base}/${encodeURIComponent(mediaId)}`;
}

export interface Transport {
	me(): Promise<User | null>;
	login(username: string, password: string): Promise<User>;
	logout(): Promise<void>;
	catalog(): Promise<Catalog>;
	// Desktop: dispara o seletor nativo e registra a pasta escolhida como um curso.
	importCourse(): Promise<void>;
	// Servidor: registra um curso pelo caminho informado pelo admin.
	importCoursePath(path: string): Promise<void>;
	// Remove um curso do registro, sem tocar nos arquivos.
	removeCourse(courseId: string): Promise<void>;
	rescan(): Promise<void>;
	updateCredentials(username: string, password: string, currentPassword?: string): Promise<User>;
	studyProgress(mediaId: string): Promise<StudyProgress>;
	recordPosition(mediaId: string, positionSeconds: number): Promise<StudyProgress>;
	setCompleted(mediaId: string, completed: boolean): Promise<StudyProgress>;
	recentStudy(limit?: number): Promise<RecentStudy>;
	// Verifica a atualização; `force` ignora o intervalo mínimo de cache do servidor.
	checkUpdate(force?: boolean): Promise<UpdateStatus>;
	mediaUrl(file: FileEntry, courseId?: string): string | null;
}

type RustFile = { media_id?: string; name?: string; path?: string; size?: number; mtime_seconds?: number; kind?: string; availability?: string; unverified?: boolean };
type RustNode = { title?: string; path?: string; role?: string; course_id?: string; files?: RustFile[]; loose_files?: RustFile[]; children?: RustNode[] };
type RustCatalog = { complete?: boolean; categories?: RustNode[]; nodes?: RustNode[]; loose_files?: RustFile[]; looseFiles?: RustFile[] };

function invalidPayload(message: string): Error { return new Error(`Resposta inválida do servidor: ${message}`); }

// Limites explícitos da normalização do catálogo. Um payload hostil não pode esgotar a pilha
// (profundidade) nem a memória (nós e arquivos): o payload que ultrapassa um limite é recusado
// inteiro, nunca truncado. São generosos para uma biblioteca real e valem para os dois
// transportes, já que ambos passam por `normalizeCatalog`.
export const CATALOG_LIMITS = { depth: 32, nodes: 50_000, files: 200_000 } as const;

// Orçamento compartilhado pela normalização de um catálogo: conta nós e arquivos de forma
// determinística para recusar payloads grandes demais antes de amplificá-los.
type CatalogBudget = { nodes: number; files: number };

function newCatalogBudget(): CatalogBudget { return { nodes: 0, files: 0 }; }

function countNode(budget: CatalogBudget, depth: number): void {
	if (depth > CATALOG_LIMITS.depth) throw invalidPayload('catálogo profundo demais.');
	budget.nodes += 1;
	if (budget.nodes > CATALOG_LIMITS.nodes) throw invalidPayload('catálogo com nós demais.');
}

function countFile(budget: CatalogBudget): void {
	budget.files += 1;
	if (budget.files > CATALOG_LIMITS.files) throw invalidPayload('catálogo com arquivos demais.');
}

export function fileStatus(file: FileEntry): string {
	return [file.availability === 'missing' ? 'ausente' : '', file.unverified ? 'não verificado' : ''].filter(Boolean).join(' · ');
}

/** Identidade de um curso no catálogo: a opaca do contrato ou o caminho do nó legado. */
export function courseIdOf(node: CatalogNode): string {
	return node.courseId ?? node.path;
}

// Campo opcional presente de um arquivo: aceita ausência, mas um valor presente com tipo
// incorreto (por exemplo `media_id: 42`) é recusado em vez de vazar para a interface.
function optionalFileString(value: unknown, key: string): string | undefined {
	if (value === undefined) return undefined;
	if (typeof value !== 'string') throw invalidPayload(`campo ${key} do arquivo inválido.`);
	return value;
}

function optionalFileBoolean(value: unknown, key: string): boolean | undefined {
	if (value === undefined) return undefined;
	if (typeof value !== 'boolean') throw invalidPayload(`campo ${key} do arquivo inválido.`);
	return value;
}

// O contrato declara o tamanho como `u64`: precisa ser um inteiro seguro e não negativo.
// Um valor fracionário, negativo ou além do inteiro seguro do JavaScript é recusado em vez
// de renderizar um tamanho impossível.
function fileSize(value: unknown): number {
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw invalidPayload('arquivo com tamanho inválido.');
	return value;
}

// Revisões antigas podem omitir mtime; um valor presente precisa ser um segundo Unix inteiro
// representável com segurança pelo JavaScript.
function optionalMtimeSeconds(value: unknown): number | undefined {
	if (value === undefined) return undefined;
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) {
		throw invalidPayload('arquivo com mtime inválido.');
	}
	return value;
}

// Arquivo do contrato canônico Rust/desktop: exige identidade `media_id` não vazia para que
// o arquivo seja endereçável.
function normalizeFile(value: unknown): FileEntry {
	if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalidPayload('arquivo inválido.');
	const file = value as RustFile;
	const name = file.name;
	const path = file.path;
	if (typeof name !== 'string' || typeof path !== 'string') throw invalidPayload('arquivo sem nome ou caminho.');
	const mediaId = optionalFileString(file.media_id, 'media_id');
	if (!mediaId) throw invalidPayload('arquivo sem media_id.');
	return {
		name,
		path,
		size: fileSize(file.size),
		mtimeSeconds: optionalMtimeSeconds(file.mtime_seconds),
		mediaId,
		kind: optionalFileString(file.kind, 'kind'),
		availability: optionalFileString(file.availability, 'availability'),
		unverified: optionalFileBoolean(file.unverified, 'unverified')
	};
}

// Arquivo de um nó canônico Rust: consome o orçamento de arquivos.
function normalizeRustFile(value: unknown, budget: CatalogBudget): FileEntry {
	countFile(budget);
	return normalizeFile(value);
}

function lesson(title: string, path: string, files: FileEntry[]): Lesson {
	return {
		title, path,
		media: files.filter((file) => file.kind === 'media'),
		subtitles: files.filter((file) => file.kind === 'subtitle'),
		// `materials` é o balde catch-all da gramática legada: um `kind` desconhecido (ou
		// ausente) não pode fazer um arquivo útil sumir da aula.
		materials: files.filter((file) => file.kind !== 'media' && file.kind !== 'subtitle')
	};
}

function nodeFiles(node: CatalogNode): FileEntry[] { return [...node.files, ...node.looseFiles]; }

// Aula de um nó canônico. `materials` é o balde catch-all (ver `lesson`), então um `kind`
// desconhecido continua acessível: esta é a rotina única usada pela projeção, pelo roteador e
// pela UI, para que abrir ou rotear uma aula nunca descarte um arquivo.
export function lessonFromNode(node: CatalogNode): Lesson {
	const files = nodeFiles(node);
	return { ...lesson(node.title, node.path, files), availability: files.find((file) => file.availability)?.availability, unverified: files.some((file) => file.unverified) };
}

// Resolve um par espelhado snake_case/camelCase de forma determinística. A ausência dos dois
// campos vale como lista vazia; um `null` explícito ou um tipo que não seja lista é recusado
// (nunca tratado como ausente). Quando os dois vêm preenchidos, o snake_case é o contrato
// canônico; valores divergentes são recusados como payload ambíguo em vez de escolher um lado.
function mirroredArray(source: object, snake: string, camel: string): unknown[] {
	const record = source as Record<string, unknown>;
	const hasSnake = Object.prototype.hasOwnProperty.call(record, snake);
	const hasCamel = Object.prototype.hasOwnProperty.call(record, camel);
	if (hasSnake && hasCamel && JSON.stringify(record[snake]) !== JSON.stringify(record[camel])) throw invalidPayload(`campos ${snake} e ${camel} conflitantes.`);
	if (!hasSnake && !hasCamel) return [];
	const raw = hasSnake ? record[snake] : record[camel];
	if (raw === null || !Array.isArray(raw)) throw invalidPayload(`campo ${snake} inválido.`);
	return raw;
}

// Campo de nós opcional: ausente vale como lista vazia; `null` explícito ou
// tipo incorreto é recusado em vez de descartado silenciosamente.
function optionalArray(source: object, key: string): unknown[] {
	const record = source as Record<string, unknown>;
	if (!Object.prototype.hasOwnProperty.call(record, key) || record[key] === undefined) return [];
	const raw = record[key];
	if (raw === null || !Array.isArray(raw)) throw invalidPayload(`campo ${key} inválido.`);
	return raw;
}

function normalizeRustNode(value: unknown, depth: number, budget: CatalogBudget): CatalogNode {
	countNode(budget, depth);
	if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalidPayload('node Rust inválido.');
	const node = value as RustNode;
	if (typeof node.title !== 'string' || typeof node.path !== 'string' || typeof node.role !== 'string') throw invalidPayload('node Rust incompleto.');
	if (node.course_id !== undefined && typeof node.course_id !== 'string') throw invalidPayload('node Rust com identidade de curso inválida.');
	return {
		title: node.title,
		path: node.path,
		role: node.role,
		courseId: node.course_id,
		files: optionalArray(value, 'files').map((file) => normalizeRustFile(file, budget)),
		looseFiles: optionalArray(value, 'loose_files').map((file) => normalizeRustFile(file, budget)),
		children: optionalArray(value, 'children').map((child) => normalizeRustNode(child, depth + 1, budget))
	};
}

// Árvore canônica de um payload Rust: `nodes` é preferido. Quando a chave `nodes` está
// presente, o valor precisa ser uma lista — um `null`, string ou qualquer tipo que não seja
// lista é recusado em vez de mascarado por `categories`. Um `nodes` vazio cai de volta para
// `categories` (compatibilidade com projeção de categorias) e, sem `categories`, vale como lista vazia.
function canonicalNodeList(value: RustCatalog): unknown {
	const record = value as Record<string, unknown>;
	const hasNodes = Object.prototype.hasOwnProperty.call(record, 'nodes') && record.nodes !== undefined;
	if (hasNodes) {
		if (!Array.isArray(record.nodes)) throw invalidPayload('campo nodes inválido.');
		if (record.nodes.length > 0) return record.nodes;
	}
	const rawCategories = record.categories;
	if (rawCategories === undefined) return [];
	if (!Array.isArray(rawCategories)) throw invalidPayload('campo categories inválido.');
	return rawCategories;
}

function normalizeRustCatalog(value: RustCatalog): Catalog {
	const rawNodes = canonicalNodeList(value);
	if (!Array.isArray(rawNodes)) throw invalidPayload('catálogo Rust sem nodes.');
	const budget = newCatalogBudget();
	const looseFiles = mirroredArray(value, 'loose_files', 'looseFiles').map((file) => { countFile(budget); return normalizeFile(file); });
	const nodes = rawNodes.map((node) => normalizeRustNode(node, 1, budget));
	return { complete: value.complete === true, looseFiles, nodes };
}

export function normalizeUser(value: unknown): User {
	if (!value || typeof value !== 'object' || typeof (value as { username?: unknown }).username !== 'string') throw invalidPayload('usuário sem username.');
	const user = value as { user_id?: unknown; username: string; role?: unknown; is_admin?: unknown; must_change_credentials?: unknown };
	return { userId: typeof user.user_id === 'string' || typeof user.user_id === 'number' ? String(user.user_id) : '', username: user.username, isAdmin: user.is_admin === true || user.role === 'admin', role: user.is_admin === true || user.role === 'admin' ? 'admin' : 'user', mustChangeCredentials: user.must_change_credentials === true };
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

// Campo de atualização opcional: `null`/ausente valem como nulo; um tipo que não seja
// string é recusado em vez de vazar para a interface.
function optionalUpdateString(value: unknown, key: string): string | null {
	if (value === undefined || value === null) return null;
	if (typeof value !== 'string') throw invalidPayload(`campo ${key} da atualização inválido.`);
	return value;
}

export function normalizeUpdateStatus(value: unknown): UpdateStatus {
	if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalidPayload('estado de atualização ausente.');
	const status = value as { current?: unknown; enabled?: unknown; available?: unknown; latest?: unknown; releasedAt?: unknown; url?: unknown; checkedAt?: unknown; error?: unknown };
	if (typeof status.current !== 'string' || typeof status.enabled !== 'boolean' || typeof status.available !== 'boolean') throw invalidPayload('estado de atualização incompleto.');
	return {
		current: status.current,
		enabled: status.enabled,
		available: status.available,
		latest: optionalUpdateString(status.latest, 'latest'),
		releasedAt: optionalUpdateString(status.releasedAt, 'releasedAt'),
		url: optionalUpdateString(status.url, 'url'),
		checkedAt: optionalUpdateString(status.checkedAt, 'checkedAt'),
		error: optionalUpdateString(status.error, 'error')
	};
}

// Envelope de categoria da projeção de categorias: `{ children }`.
function isCategoryEnvelope(item: unknown): boolean {
	return item !== null && typeof item === 'object' && !Array.isArray(item) && 'children' in item;
}

// `categories[].courses` era o shape legado exclusivo do serviço Go descontinuado.
function hasLegacyCourses(categories: unknown): boolean {
	if (!Array.isArray(categories) || categories.length === 0) return false;
	for (const item of categories) {
		if (item !== null && typeof item === 'object' && !Array.isArray(item) && 'courses' in item) {
			return true;
		}
	}
	return false;
}

export function normalizeCatalog(value: unknown): Catalog {
	if (!value || typeof value !== 'object') throw invalidPayload('catálogo ausente.');
	const record = value as Record<string, unknown>;
	// O contrato canônico Rust/desktop exige explicitamente `complete: bool`.
	// Payloads sem o marcador ou com valor não booleano são rejeitados (fail-closed).
	const hasComplete = Object.prototype.hasOwnProperty.call(record, 'complete') && record.complete !== undefined;
	if (!hasComplete || typeof record.complete !== 'boolean') throw invalidPayload('campo complete ausente ou inválido.');

	// `nodes` presente precisa ser lista: `null`, string ou tipo inválido é payload malformado e
	// nunca pode ser escondido por um fallback silencioso para `categories`.
	const hasNodes = Object.prototype.hasOwnProperty.call(record, 'nodes') && record.nodes !== undefined;
	if (hasNodes && !Array.isArray(record.nodes)) throw invalidPayload('campo nodes inválido.');
	const payload = value as { categories?: unknown };

	// Com nós canônicos, `categories` é a projeção legada e é ignorada.
	if (Array.isArray(record.nodes) && record.nodes.length > 0) return normalizeRustCatalog(value as RustCatalog);

	// Sem nós canônicos, campos só-do-Go (`categories[].courses`) não podem ser consumidos
	// como nós nem virar conteúdo vazio: falha fechada.
	if (hasLegacyCourses(payload.categories)) throw invalidPayload('catálogo canônico com campos legados do Go.');
	if (Array.isArray(payload.categories) && payload.categories.every(isCategoryEnvelope)) return normalizeRustCatalog(value as RustCatalog);
	if (hasNodes || Array.isArray(payload.categories)) return normalizeRustCatalog(value as RustCatalog);
	throw invalidPayload('formato de catálogo desconhecido.');
}

// Lê o `error` estável do corpo JSON de uma resposta falha, quando existir.
async function responseError(response: Response): Promise<string | null> {
	try {
		const payload = await response.json() as { error?: unknown };
		return typeof payload?.error === 'string' ? payload.error : null;
	} catch {
		return null;
	}
}

async function importErrorMessage(response: Response): Promise<string> {
	switch (await responseError(response)) {
		case 'invalid_course_path': return 'A pasta informada não existe ou não pode ser lida no servidor.';
		case 'course_already_registered': return 'Este curso já está importado.';
		default: return response.status === 503 ? 'O servidor está ocupado; tente de novo em instantes.' : 'Não foi possível importar o curso.';
	}
}

export class HttpTransport implements Transport {
	async me() { const response = await fetch('/api/me'); return response.ok ? normalizeUser(await response.json()) : null; }
	async login(username: string, password: string) {
		const response = await fetch('/api/login', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ username, password }) });
		if (!response.ok) throw new Error(response.status === 401 ? 'Usuário ou senha incorretos.' : 'Não foi possível entrar.');
		return normalizeUser(await response.json());
	}
	async logout() { await fetch('/api/logout', { method: 'POST' }); }
	importCourse() { return Promise.reject(new Error('O desktop é quem escolhe pastas no disco; no servidor informe o caminho do curso.')); }
	async importCoursePath(path: string) {
		const response = await fetch('/api/admin/courses', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ path }) });
		if (!response.ok) throw new Error(await importErrorMessage(response));
	}
	async removeCourse(courseId: string) {
		const response = await fetch(`/api/admin/courses/${encodeURIComponent(courseId)}`, { method: 'DELETE' });
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível remover o curso.');
	}
	async catalog() { const response = await fetch('/api/catalog'); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar o catálogo.'); return normalizeCatalog(await response.json()); }
	async rescan() { const response = await fetch('/api/admin/catalog/rescan', { method: 'POST' }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 409 ? 'Já existe um rescan em andamento.' : response.status === 400 ? 'Nenhum curso foi importado ainda.' : 'Não foi possível indexar os cursos.'); }
	async updateCredentials(username: string, password: string, currentPassword = password) { const response = await fetch('/api/account/credentials', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ current_password: currentPassword, username, password }) }); if (!response.ok) throw new Error(response.status === 422 ? 'Usuário ou senha inválidos.' : response.status === 401 ? 'Senha atual incorreta.' : 'Não foi possível atualizar as credenciais.'); return normalizeUser(await response.json()); }
	async studyProgress(mediaId: string) { const response = await fetch(`/api/study/progress?media_id=${encodeURIComponent(mediaId)}`); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 404 ? 'Mídia não encontrada.' : 'Não foi possível ler o progresso.'); return normalizeStudyProgress(await response.json()); }
	async recordPosition(mediaId: string, positionSeconds: number) { const response = await fetch('/api/study/position', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ media_id: mediaId, position_seconds: positionSeconds }) }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 404 ? 'Mídia não encontrada.' : response.status === 422 ? 'Posição inválida.' : 'Não foi possível salvar o progresso.'); return normalizeStudyProgress(await response.json()); }
	async setCompleted(mediaId: string, completed: boolean) { const response = await fetch('/api/study/completed', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ media_id: mediaId, completed }) }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 404 ? 'Mídia não encontrada.' : 'Não foi possível atualizar a conclusão.'); return normalizeStudyProgress(await response.json()); }
	async recentStudy(limit?: number) { const url = limit === undefined ? '/api/study/recent' : `/api/study/recent?limit=${limit}`; const response = await fetch(url); if (!response.ok) throw response.status === 404 ? Object.assign(new Error('Estudo não disponível neste servidor.'), { code: STUDY_NOT_AVAILABLE }) : new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar o histórico.'); return normalizeRecentStudy(await response.json()); }
	// O servidor local faz a checagem por HTTPS e devolve o estado; o navegador só o exibe.
	// `force=1` (checagem manual) ignora o intervalo mínimo de cache do servidor.
	async checkUpdate(force = false) { const response = await fetch(force ? '/api/update?force=1' : '/api/update'); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível verificar a atualização.'); return normalizeUpdateStatus(await response.json()); }
	mediaUrl(file: FileEntry, courseId?: string) { return `/api/media?course=${encodeURIComponent(courseId ?? '')}&path=${encodeURIComponent(file.path)}`; }
}

// Códigos estáveis de erro IPC e a mensagem sanitizada correspondente. Um código
// desconhecido ou ausente nunca repassa o texto recebido: cai no fallback da chamada,
// para que nenhuma mensagem arbitrária (potencialmente com caminhos) chegue à interface.
const DESKTOP_ERROR_MESSAGES: Record<string, string> = {
	catalog_unavailable: 'catálogo indisponível',
	database_unavailable: 'banco de dados local indisponível',
	course_path_invalid: 'A pasta escolhida não é um curso válido.',
	courses_config_unavailable: 'Não foi possível salvar a configuração do aplicativo.',
	no_courses_registered: 'Nenhum curso foi importado ainda.',
	rescan_in_progress: 'Já existe uma indexação em andamento.',
	rescan_unavailable: 'Não foi possível indexar os cursos.',
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
	async catalog() { try { return normalizeCatalog(await invoke<unknown>('desktop_catalog')); } catch (cause) { throw desktopError(cause, 'Não foi possível carregar o catálogo.'); } }
	// O webview apenas dispara o seletor nativo no Rust; nenhum caminho é enviado nem
	// devolvido. Cancelar preserva a lista anterior de cursos.
	async importCourse() { try { await invoke<unknown>('desktop_import_course'); } catch (cause) { throw desktopError(cause, 'Não foi possível importar o curso.'); } }
	importCoursePath(_path: string) { return Promise.reject(new Error('No desktop a pasta é escolhida pelo seletor nativo do sistema.')); }
	// A remoção envia somente a identidade opaca do curso, nunca um caminho.
	async removeCourse(courseId: string) { try { await invoke<unknown>('desktop_remove_course', { courseId }); } catch (cause) { throw desktopError(cause, 'Não foi possível remover o curso.'); } }
	// O rescan desktop é sempre explícito: o webview só o dispara a pedido do usuário. A
	// resposta é um resumo sanitizado que não interessa ao shell, que recarrega o catálogo.
	async rescan() { try { await invoke<unknown>('desktop_rescan'); } catch (cause) { throw desktopError(cause, 'Não foi possível indexar os cursos.'); } }
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
	// O desktop não hospeda `crates/server` nem possui canal IPC de atualização: a
	// checagem fica desligada, sem tocar a rede. Isso preserva a regra de que o bundle
	// desktop não contém nenhum `fetch` do aplicativo.
	checkUpdate() { return Promise.resolve({ current: '', enabled: false, available: false, latest: null, releasedAt: null, url: null, checkedAt: null, error: null } satisfies UpdateStatus); }
	// Toda mídia entregue ao player usa a identidade autorizada. O caminho catalogado é
	// deliberadamente ignorado, inclusive no desktop, para que ele nunca atravesse o webview.
	mediaUrl(file: FileEntry, _courseId?: string) {
		if (!file.mediaId || !['media', 'subtitle', 'material'].includes(file.kind ?? '')) return null;
		return desktopMediaUrl(file.mediaId, navigator.userAgent);
	}
}

export const transportKind: TransportKind = __LEARNVAULT_TRANSPORT__ === 'desktop' ? 'desktop' : 'http';
export const transport: Transport = transportKind === 'desktop' ? new DesktopStub() : new HttpTransport();
