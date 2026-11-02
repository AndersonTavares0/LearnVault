// O `invoke` do Tauri entra apenas no bundle desktop: no bundle HTTP o `DesktopStub` é
// eliminado por dead-code elimination junto com este import, de modo que nem o Tauri entra
// na SPA web nem um `fetch` entra no desktop.
import { invoke } from '@tauri-apps/api/core';

export type User = { userId: string; username: string; role: string; isAdmin: boolean; mustChangeCredentials: boolean };
export type FileEntry = { name: string; path: string; size: number; mediaId?: string; mtimeSeconds?: number; kind?: string; availability?: string; unverified?: boolean };
export type Lesson = { title: string; path: string; media: FileEntry[]; subtitles: FileEntry[]; materials: FileEntry[]; availability?: string; unverified?: boolean };
// Cada nó de topo do catálogo é um curso com identidade opaca (`courseId`). Nós internos
// não carregam a identidade: a árvore inteira pertence ao curso do topo.
export type CatalogNode = { title: string; path: string; role: string; courseId?: string; coverVersion?: number; files: FileEntry[]; looseFiles: FileEntry[]; children: CatalogNode[] };
export type Catalog = { complete: boolean; looseFiles: FileEntry[]; nodes: CatalogNode[] };
// A seleção nativa de uma pasta de curso é específica do desktop: no HTTP o servidor
// registra cursos por caminho e nenhum caminho cruza o frontend.
export type TransportKind = 'http' | 'desktop';
export type StudyProgress = { mediaId: string; positionSeconds: number; completed: boolean; updatedAt: number | null; recorded: boolean };
export type RecentStudy = { items: StudyProgress[] };
export type StudyDay = { day: string; active: boolean };
export type StudySummary = {
	currentStreak: number;
	longestStreak: number;
	today: string;
	todayActive: boolean;
	atRisk: boolean;
	weekActiveDays: number;
	completedLessons: number;
	days: StudyDay[];
};
export type FocusKind = 'focus' | 'short' | 'long';
export type FocusSessionInput = { id: string; kind: FocusKind; plannedSeconds: number; actualSeconds: number; completed: boolean; startedAt: number; endedAt: number };
export type FocusSessionRecord = FocusSessionInput;
export type FocusDaySummary = { day: string; minutes: number; sessions: number };
export type FocusSummary = { todayMinutes: number; weekMinutes: number; weekSessions: number; totalMinutes: number; totalSessions: number; completionRate: number; bestDay: string | null; days: FocusDaySummary[] };
export type MissionId = 'lesson' | 'focus-two' | 'focus-fifty';
export type DailyMission = { id: MissionId; progress: number; target: number; claimed: boolean };
export type RewardSummary = { xp: number; completedLessons: number; weeklyGoal: number; missions: DailyMission[]; study: StudySummary };
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
	changelog: string | null;
	checkedAt: string | null;
	error: string | null;
};
export type UpdateHistoryEntry = {
	timestamp: string;
	status: string;
	from: string;
	to: string;
	message: string;
};
export type StudyNote = {
	noteId: string;
	mediaId: string;
	relativePath?: string;
	positionSeconds: number;
	content: string;
	createdAt: number;
	updatedAt: number;
};
export type Flashcard = {
	cardId: string;
	mediaId: string;
	front: string;
	back: string;
	intervalDays: number;
	nextDue: number;
	createdAt: number;
	updatedAt: number;
};
export type ReviewRating = 'again' | 'good';
export const STUDY_NOT_AVAILABLE = 'study_not_available';

export type MediaCapabilities = {
	transcoding: boolean;
	systemPlayer?: boolean;
	supportedCodecs?: string[];
};

// Protocolo customizado do desktop para mídia local. O webview aponta somente pelo
// `mediaId` autorizado; nenhum caminho cruza a fronteira do webview.
export const DESKTOP_MEDIA_SCHEME = 'learnvault-media';
export const DESKTOP_MEDIA_HOST = 'localhost';

// Protocolo customizado do desktop para as capas personalizadas dos cursos. O webview aponta
// somente pela identidade opaca do curso: o handler do Rust resolve o arquivo no diretório de
// capas do próprio aplicativo.
export const DESKTOP_COVER_SCHEME = 'learnvault-cover';

// Formatos e limite aceitos para uma capa personalizada. A validação real acontece nos bytes
// iniciais (magic bytes) no servidor/Rust; aqui a lista apenas orienta o seletor de arquivos.
export const COVER_ACCEPT = 'image/png,image/jpeg,image/webp';
export const MAX_COVER_BYTES = 4 * 1024 * 1024;

// No Chromium embutido (WebView2 no Windows e o webview do Android) um esquema customizado
// do Tauri só é resolvido na forma `http://<esquema>.localhost`; o runtime reverte esse
// prefixo por requisição antes de chamar o handler, que continua vendo
// `learnvault-media://localhost/...`. No Linux (WebKitGTK) e no macOS (WKWebView) o esquema
// é resolvido direto. Sem o prefixo correto o Chromium não resolve o esquema e o recurso não
// carrega. A detecção usa apenas o `userAgent` do webview atual, sem dependência nova.
function desktopProtocolBase(scheme: string, userAgent: string): string {
	return /Windows|Android/i.test(userAgent) ? `http://${scheme}.localhost` : `${scheme}://${DESKTOP_MEDIA_HOST}`;
}

export function desktopMediaUrl(mediaId: string, userAgent: string): string {
	return `${desktopProtocolBase(DESKTOP_MEDIA_SCHEME, userAgent)}/${encodeURIComponent(mediaId)}`;
}

/** URL da capa personalizada no desktop, endereçada pela identidade opaca do curso. */
export function desktopCoverUrl(courseId: string, userAgent: string): string {
	return `${desktopProtocolBase(DESKTOP_COVER_SCHEME, userAgent)}/${encodeURIComponent(courseId)}`;
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
	studySummary(): Promise<StudySummary>;
	listNotes(mediaId?: string): Promise<StudyNote[]>;
	createNote(mediaId: string, positionSeconds: number, content: string): Promise<StudyNote>;
	updateNote(noteId: string, content: string): Promise<StudyNote>;
	deleteNote(noteId: string): Promise<void>;
	listFlashcards(mediaId: string): Promise<Flashcard[]>;
	createFlashcard(mediaId: string, front: string, back: string): Promise<Flashcard>;
	updateFlashcard(cardId: string, front: string, back: string): Promise<Flashcard>;
	deleteFlashcard(cardId: string): Promise<void>;
	reviewFlashcard(cardId: string, rating: ReviewRating): Promise<Flashcard>;
	recordFocusSession(session: FocusSessionInput): Promise<FocusSessionRecord>;
	listFocusSessions(limit?: number): Promise<FocusSessionRecord[]>;
	focusSummary(): Promise<FocusSummary>;
	rewardSummary(): Promise<RewardSummary>;
	claimDailyMission(missionId: MissionId): Promise<boolean>;
	setWeeklyGoal(goal: number): Promise<number>;
	userAvatarUrl(version?: number | null): string;
	uploadUserAvatar(file: File | Blob): Promise<{ version: number }>;
	deleteUserAvatar(): Promise<void>;
	// Verifica a atualização; `force` ignora o intervalo mínimo de cache do servidor.
	checkUpdate(force?: boolean): Promise<UpdateStatus>;
	installUpdate?(): Promise<boolean>;
	updateHistory(): Promise<UpdateHistoryEntry[]>;
	mediaCapabilities(): Promise<MediaCapabilities>;
	mediaUrl(file: FileEntry, courseId?: string): string | null;
	transcodeUrl(file: FileEntry, courseId?: string, startSeconds?: number, reencode?: boolean): string | null;
	openInSystemPlayer?(file: FileEntry): Promise<boolean>;
	// URL da capa personalizada, derivada da versão exposta pelo catálogo. Sem versão (sem
	// capa) devolve `null`, e a interface cai na miniatura ou na capa gerada.
	coverUrl(courseId: string, coverVersion?: number | null): string | null;
	// Desktop: dispara o seletor nativo de imagens no Rust (o arquivo não cruza o IPC).
	// Servidor: envia o arquivo escolhido pelo usuário no navegador.
	importCourseCover(courseId: string, file?: File | null): Promise<void>;
	removeCourseCover(courseId: string): Promise<boolean>;
	// Ordenação manual de módulos e aulas entre irmãos. `parent` é o caminho relativo do
	// nó pai (vazio para a lista de topo); `children` é a ordem desejada dos filhos, que
	// precisa ser uma permutação exata dos irmãos.
	setCourseOrder(courseId: string, parent: string, children: string[]): Promise<void>;
	// Restaura a ordem automática de um pai específico ou, sem `parent`, do curso inteiro.
	// Devolve `true` quando havia uma preferência a remover.
	resetCourseOrder(courseId: string, parent?: string): Promise<boolean>;
}

type RustFile = { media_id?: string; name?: string; path?: string; size?: number; mtime_seconds?: number; kind?: string; availability?: string; unverified?: boolean };
type RustNode = { title?: string; path?: string; role?: string; course_id?: string; cover_version?: number; files?: RustFile[]; loose_files?: RustFile[]; children?: RustNode[] };
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

// Versão da capa personalizada de um curso: um segundo Unix inteiro não negativo. Uma
// revisão antiga do servidor simplesmente não envia o campo (sem capa).
function optionalCoverVersion(value: unknown): number | undefined {
	if (value === undefined) return undefined;
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw invalidPayload('curso com versão de capa inválida.');
	return value;
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
		coverVersion: optionalCoverVersion(node.cover_version),
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

export function normalizeStudySummary(value: unknown): StudySummary {
	if (!value || typeof value !== 'object') throw invalidPayload('resumo de estudo ausente.');
	const summary = value as {
		current_streak?: unknown;
		longest_streak?: unknown;
		today?: unknown;
		today_active?: unknown;
		at_risk?: unknown;
		week_active_days?: unknown;
		completed_lessons?: unknown;
		days?: unknown;
	};
	if (
		!Number.isInteger(summary.current_streak) ||
		!Number.isInteger(summary.longest_streak) ||
		typeof summary.today !== 'string' ||
		!/^\d{4}-\d{2}-\d{2}$/.test(summary.today) ||
		!Number.isInteger(summary.week_active_days) ||
		!Number.isInteger(summary.completed_lessons) ||
		typeof summary.today_active !== 'boolean' ||
		typeof summary.at_risk !== 'boolean' ||
		!Array.isArray(summary.days) ||
		summary.days.length !== 7
	) throw invalidPayload('resumo de estudo incompleto.');
	const days = summary.days.map((entry) => {
		if (!entry || typeof entry !== 'object') throw invalidPayload('dia do resumo inválido.');
		const day = entry as { day?: unknown; active?: unknown };
		if (typeof day.day !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(day.day) || typeof day.active !== 'boolean') throw invalidPayload('dia do resumo inválido.');
		return { day: day.day, active: day.active };
	});
	return {
		currentStreak: summary.current_streak as number,
		longestStreak: summary.longest_streak as number,
		today: summary.today as string,
		todayActive: summary.today_active as boolean,
		atRisk: summary.at_risk as boolean,
		weekActiveDays: summary.week_active_days as number,
		completedLessons: summary.completed_lessons as number,
		days
	};
}

function safeInteger(value: unknown): value is number {
	return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0;
}

export class FocusSessionConflict extends Error {
	constructor() { super('Esta sessão já foi registrada com outros dados.'); this.name = 'FocusSessionConflict'; }
}

export class InvalidFocusSession extends Error {
	constructor() { super('Os dados da sessão de foco não são válidos.'); this.name = 'InvalidFocusSession'; }
}

export function normalizeFocusSession(value: unknown): FocusSessionRecord {
	if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalidPayload('sessão de foco ausente.');
	const session = value as Record<string, unknown>;
	if (typeof session.id !== 'string' || !session.id || !['focus', 'short', 'long'].includes(String(session.kind)) || !safeInteger(session.planned_seconds) || !safeInteger(session.actual_seconds) || typeof session.completed !== 'boolean' || !safeInteger(session.started_at) || !safeInteger(session.ended_at) || session.actual_seconds > session.planned_seconds || session.ended_at < session.started_at || session.actual_seconds > session.ended_at - session.started_at) throw invalidPayload('sessão de foco inválida.');
	return { id: session.id, kind: session.kind as FocusKind, plannedSeconds: session.planned_seconds, actualSeconds: session.actual_seconds, completed: session.completed, startedAt: session.started_at, endedAt: session.ended_at };
}

export function normalizeFocusSummary(value: unknown): FocusSummary {
	if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalidPayload('resumo de foco ausente.');
	const summary = value as Record<string, unknown>;
	if (!safeInteger(summary.today_minutes) || !safeInteger(summary.week_minutes) || !safeInteger(summary.week_sessions) || !safeInteger(summary.total_minutes) || !safeInteger(summary.total_sessions) || !safeInteger(summary.completion_rate) || summary.completion_rate > 100 || !(summary.best_day === null || typeof summary.best_day === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(summary.best_day)) || !Array.isArray(summary.days) || summary.days.length !== 7) throw invalidPayload('resumo de foco incompleto.');
	const days = summary.days.map((value) => {
		if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalidPayload('dia de foco inválido.');
		const day = value as Record<string, unknown>;
		if (typeof day.day !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(day.day) || !safeInteger(day.minutes) || !safeInteger(day.sessions)) throw invalidPayload('dia de foco inválido.');
		return { day: day.day, minutes: day.minutes, sessions: day.sessions };
	});
	return { todayMinutes: summary.today_minutes, weekMinutes: summary.week_minutes, weekSessions: summary.week_sessions, totalMinutes: summary.total_minutes, totalSessions: summary.total_sessions, completionRate: summary.completion_rate, bestDay: summary.best_day as string | null, days };
}

export function normalizeRewardSummary(value: unknown): RewardSummary {
	if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalidPayload('resumo de recompensas ausente.');
	const summary = value as Record<string, unknown>;
	if (!safeInteger(summary.xp) || !safeInteger(summary.completed_lessons) || !Number.isInteger(summary.weekly_goal) || (summary.weekly_goal as number) < 1 || (summary.weekly_goal as number) > 7 || !Array.isArray(summary.missions)) throw invalidPayload('resumo de recompensas incompleto.');
	const missions = summary.missions.map((value) => {
		if (!value || typeof value !== 'object' || Array.isArray(value)) throw invalidPayload('missão inválida.');
		const mission = value as Record<string, unknown>;
		if (!['lesson', 'focus-two', 'focus-fifty'].includes(String(mission.id)) || !safeInteger(mission.progress) || !safeInteger(mission.target) || mission.target === 0 || mission.progress > mission.target || typeof mission.claimed !== 'boolean') throw invalidPayload('missão inválida.');
		return { id: mission.id as MissionId, progress: mission.progress, target: mission.target, claimed: mission.claimed };
	});
	if (!summary.study) throw invalidPayload('resumo de atividade ausente.');
	return { xp: summary.xp, completedLessons: summary.completed_lessons, weeklyGoal: summary.weekly_goal as number, missions, study: normalizeStudySummary(summary.study) };
}

function normalizeWeeklyGoal(value: unknown): number {
	if (!value || typeof value !== 'object' || !Number.isInteger((value as { weekly_goal?: unknown }).weekly_goal) || ((value as { weekly_goal: number }).weekly_goal < 1 || (value as { weekly_goal: number }).weekly_goal > 7)) throw invalidPayload('meta semanal inválida.');
	return (value as { weekly_goal: number }).weekly_goal;
}

function normalizeClaim(value: unknown): boolean {
	if (!value || typeof value !== 'object' || typeof (value as { claimed?: unknown }).claimed !== 'boolean') throw invalidPayload('resgate de missão inválido.');
	return (value as { claimed: boolean }).claimed;
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
	const status = value as { current?: unknown; enabled?: unknown; available?: unknown; latest?: unknown; releasedAt?: unknown; url?: unknown; changelog?: unknown; checkedAt?: unknown; error?: unknown };
	if (typeof status.current !== 'string' || typeof status.enabled !== 'boolean' || typeof status.available !== 'boolean') throw invalidPayload('estado de atualização incompleto.');
	return {
		current: status.current,
		enabled: status.enabled,
		available: status.available,
		latest: optionalUpdateString(status.latest, 'latest'),
		releasedAt: optionalUpdateString(status.releasedAt, 'releasedAt'),
		url: optionalUpdateString(status.url, 'url'),
		changelog: optionalUpdateString(status.changelog, 'changelog'),
		checkedAt: optionalUpdateString(status.checkedAt, 'checkedAt'),
		error: optionalUpdateString(status.error, 'error')
	};
}

export function normalizeStudyNote(raw: unknown): StudyNote {
	if (!raw || typeof raw !== 'object' || Array.isArray(raw)) throw invalidPayload('anotação de estudo inválida.');
	const note = raw as Record<string, unknown>;
	const res: StudyNote = {
		noteId: String(note.note_id ?? note.noteId ?? ''),
		mediaId: String(note.media_id ?? note.mediaId ?? ''),
		positionSeconds: Number(note.position_seconds ?? note.positionSeconds ?? 0),
		content: String(note.content ?? ''),
		createdAt: Number(note.created_at ?? note.createdAt ?? 0),
		updatedAt: Number(note.updated_at ?? note.updatedAt ?? 0)
	};
	if (note.relative_path || note.relativePath) {
		res.relativePath = String(note.relative_path ?? note.relativePath);
	}
	return res;
}

export function normalizeFlashcard(raw: unknown): Flashcard {
	if (!raw || typeof raw !== 'object' || Array.isArray(raw)) throw invalidPayload('cartão de estudo inválido.');
	const card = raw as Record<string, unknown>;
	return {
		cardId: String(card.card_id ?? card.cardId ?? ''),
		mediaId: String(card.media_id ?? card.mediaId ?? ''),
		front: String(card.front ?? ''),
		back: String(card.back ?? ''),
		intervalDays: Number(card.interval_days ?? card.intervalDays ?? 0),
		nextDue: Number(card.next_due ?? card.nextDue ?? 0),
		createdAt: Number(card.created_at ?? card.createdAt ?? 0),
		updatedAt: Number(card.updated_at ?? card.updatedAt ?? 0)
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

// Mensagem acionável para o upload de capa. O servidor decide o formato pelos bytes iniciais,
// então `415` cobre também o arquivo renomeado com uma extensão de imagem.
function coverUploadErrorMessage(status: number): string {
	if (status === 403) return 'credentials_change_required';
	if (status === 404) return 'Curso não encontrado.';
	if (status === 413) return 'A imagem é maior que 4 MiB.';
	if (status === 415) return 'A capa precisa ser PNG, JPEG ou WebP.';
	if (status === 503) return 'O servidor está ocupado; tente de novo em instantes.';
	return 'Não foi possível salvar a capa do curso.';
}

// O valor de `removed` vem do corpo JSON; uma resposta sem corpo válido vale como `false`,
// nunca como sucesso silencioso de uma remoção que não aconteceu.
async function removalFlag(response: Response): Promise<boolean> {
	const payload = await response.json().catch(() => null) as { removed?: unknown } | null;
	return payload?.removed === true;
}

// Mensagem acionável para os endpoints de ordenação. Um `422` indica lista desatualizada
// (não é mais uma permutação dos irmãos), então a interface precisa recarregar.
function orderErrorMessage(status: number): string {
	if (status === 403) return 'credentials_change_required';
	if (status === 404) return 'Curso não encontrado.';
	if (status === 422) return 'A ordem enviada não é válida; recarregue a página.';
	if (status === 503) return 'O servidor está ocupado; tente de novo em instantes.';
	return 'Não foi possível salvar a ordem.';
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
	async studySummary() { const response = await fetch('/api/study/summary'); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar sua evolução.'); return normalizeStudySummary(await response.json()); }
	async listNotes(mediaId?: string) {
		const url = mediaId ? `/api/study/notes?media_id=${encodeURIComponent(mediaId)}` : '/api/study/notes';
		const response = await fetch(url);
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar as anotações.');
		const data = await response.json();
		return (Array.isArray(data.items) ? data.items : []).map(normalizeStudyNote);
	}
	async createNote(mediaId: string, positionSeconds: number, content: string) {
		const response = await fetch('/api/study/notes', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ media_id: mediaId, position_seconds: positionSeconds, content })
		});
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 422 ? 'Conteúdo da anotação inválido.' : 'Não foi possível salvar a anotação.');
		return normalizeStudyNote(await response.json());
	}
	async updateNote(noteId: string, content: string) {
		const response = await fetch(`/api/study/notes/${encodeURIComponent(noteId)}`, {
			method: 'PUT',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ content })
		});
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível atualizar a anotação.');
		return normalizeStudyNote(await response.json());
	}
	async deleteNote(noteId: string) {
		const response = await fetch(`/api/study/notes/${encodeURIComponent(noteId)}`, {
			method: 'DELETE'
		});
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível excluir a anotação.');
	}
	async listFlashcards(mediaId: string): Promise<Flashcard[]> {
		const response = await fetch(`/api/study/flashcards?media_id=${encodeURIComponent(mediaId)}`);
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar os cartões.');
		const data = await response.json();
		return (Array.isArray(data.items) ? data.items : []).map(normalizeFlashcard);
	}
	async createFlashcard(mediaId: string, front: string, back: string): Promise<Flashcard> {
		const response = await fetch('/api/study/flashcards', {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ media_id: mediaId, front, back })
		});
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 422 ? 'Conteúdo do cartão inválido.' : 'Não foi possível salvar o cartão.');
		return normalizeFlashcard(await response.json());
	}
	async updateFlashcard(cardId: string, front: string, back: string): Promise<Flashcard> {
		const response = await fetch(`/api/study/flashcards/${encodeURIComponent(cardId)}`, {
			method: 'PUT',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ front, back })
		});
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 422 ? 'Conteúdo do cartão inválido.' : 'Não foi possível atualizar o cartão.');
		return normalizeFlashcard(await response.json());
	}
	async deleteFlashcard(cardId: string): Promise<void> {
		const response = await fetch(`/api/study/flashcards/${encodeURIComponent(cardId)}`, {
			method: 'DELETE'
		});
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível excluir o cartão.');
	}
	async reviewFlashcard(cardId: string, rating: ReviewRating): Promise<Flashcard> {
		const response = await fetch(`/api/study/flashcards/${encodeURIComponent(cardId)}/review`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ rating })
		});
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 422 ? 'Avaliação inválida.' : 'Não foi possível registrar a revisão.');
		return normalizeFlashcard(await response.json());
	}
	async recordFocusSession(session: FocusSessionInput) {
		const response = await fetch('/api/focus/sessions', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ id: session.id, kind: session.kind, planned_seconds: session.plannedSeconds, actual_seconds: session.actualSeconds, completed: session.completed, started_at: session.startedAt, ended_at: session.endedAt }) });
		if (!response.ok) {
			if (response.status === 409) throw new FocusSessionConflict();
			if (response.status === 422) throw new InvalidFocusSession();
			throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível salvar a sessão de foco.');
		}
		return normalizeFocusSession(await response.json());
	}
	async focusSummary() { const response = await fetch('/api/focus/summary'); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar as estatísticas de foco.'); return normalizeFocusSummary(await response.json()); }
	async listFocusSessions(limit = 100): Promise<FocusSessionRecord[]> {
		const response = await fetch(`/api/focus/sessions?limit=${limit}`);
		if (!response.ok) return [];
		const data = (await response.json()) as { items?: unknown };
		if (!data || !Array.isArray(data.items)) return [];
		return data.items.map((item: Record<string, unknown>) => ({
			id: String(item.id ?? ''),
			kind: item.kind === 'short' || item.kind === 'long' ? item.kind : 'focus',
			plannedSeconds: Number(item.planned_seconds ?? item.plannedSeconds ?? 0),
			actualSeconds: Number(item.actual_seconds ?? item.actualSeconds ?? 0),
			completed: Boolean(item.completed),
			startedAt: Number(item.started_at ?? item.startedAt ?? 0),
			endedAt: Number(item.ended_at ?? item.endedAt ?? 0)
		}));
	}
	userAvatarUrl(version?: number | null) {
		return version ? `/api/user/avatar?v=${version}` : '/api/user/avatar';
	}
	async uploadUserAvatar(file: File | Blob): Promise<{ version: number }> {
		if (!file) throw new Error('Escolha uma imagem para o avatar.');
		if (file.size > 2 * 1024 * 1024) throw new Error('A imagem deve ter no máximo 2 MiB.');
		const response = await fetch('/api/user/avatar', { method: 'PUT', body: file });
		if (!response.ok) {
			if (response.status === 413) throw new Error('A imagem é maior que 2 MiB.');
			if (response.status === 415 || response.status === 422) throw new Error('Formato de imagem não suportado. Use PNG, JPEG ou WebP.');
			throw new Error('Não foi possível salvar o avatar.');
		}
		const data = (await response.json()) as { version?: unknown };
		return { version: Number(data.version ?? Date.now()) };
	}
	async deleteUserAvatar(): Promise<void> {
		const response = await fetch('/api/user/avatar', { method: 'DELETE' });
		if (!response.ok) throw new Error('Não foi possível remover o avatar.');
	}
	async rewardSummary() { const response = await fetch('/api/rewards/summary'); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar suas recompensas.'); return normalizeRewardSummary(await response.json()); }
	async claimDailyMission(missionId: MissionId) { const response = await fetch(`/api/rewards/missions/${encodeURIComponent(missionId)}/claim`, { method: 'POST' }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 409 ? 'Conclua a missão antes de resgatá-la.' : 'Não foi possível resgatar a missão.'); return normalizeClaim(await response.json()); }
	async setWeeklyGoal(goal: number) { if (!Number.isInteger(goal) || goal < 1 || goal > 7) throw new Error('A meta semanal precisa ser de 1 a 7 dias.'); const response = await fetch('/api/rewards/weekly-goal', { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ weekly_goal: goal }) }); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 422 ? 'A meta semanal precisa ser de 1 a 7 dias.' : 'Não foi possível salvar a meta semanal.'); return normalizeWeeklyGoal(await response.json()); }
	// O servidor local faz a checagem por HTTPS e devolve o estado; o navegador só o exibe.
	// `force=1` (checagem manual) ignora o intervalo mínimo de cache do servidor.
	async checkUpdate(force = false) { const response = await fetch(force ? '/api/update?force=1' : '/api/update'); if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível verificar a atualização.'); return normalizeUpdateStatus(await response.json()); }
	async installUpdate(): Promise<boolean> { return Promise.resolve(false); }
	async updateHistory(): Promise<UpdateHistoryEntry[]> {
		const response = await fetch('/api/update/history');
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : 'Não foi possível carregar o histórico de atualizações.');
		const data = (await response.json()) as { items?: unknown };
		if (!data || !Array.isArray(data.items)) return [];
		return data.items.map((item: any) => ({
			timestamp: typeof item?.timestamp === 'string' ? item.timestamp : '',
			status: typeof item?.status === 'string' ? item.status : 'unknown',
			from: typeof item?.from === 'string' ? item.from : '',
			to: typeof item?.to === 'string' ? item.to : '',
			message: typeof item?.message === 'string' ? item.message : ''
		}));
	}
	async mediaCapabilities(): Promise<MediaCapabilities> {
		try {
			const response = await fetch('/api/media/capabilities');
			if (!response.ok) return { transcoding: false };
			const data = await response.json();
			return {
				transcoding: Boolean(data?.transcoding),
				supportedCodecs: Array.isArray(data?.supported_codecs) ? data.supported_codecs : []
			};
		} catch {
			return { transcoding: false };
		}
	}
	mediaUrl(file: FileEntry, courseId?: string) { return `/api/media?course=${encodeURIComponent(courseId ?? '')}&path=${encodeURIComponent(file.path)}`; }
	transcodeUrl(file: FileEntry, courseId?: string, startSeconds: number = 0, reencode: boolean = false) {
		const params = new URLSearchParams({
			course: courseId ?? '',
			path: file.path,
			start: String(Math.max(0, startSeconds))
		});
		if (reencode) {
			params.set('reencode', 'true');
		}
		return `/api/media/transcode?${params.toString()}`;
	}
	openInSystemPlayer(_file: FileEntry): Promise<boolean> {
		return Promise.resolve(false);
	}
	// A versão (mtime em segundos) exposta pelo catálogo entra como `?v=` para invalidar a
	// imagem em cache do navegador quando a capa é trocada. Sem capa, não há URL manual.
	coverUrl(courseId: string, coverVersion?: number | null) {
		if (coverVersion === undefined || coverVersion === null) return null;
		return `/api/courses/${encodeURIComponent(courseId)}/cover?v=${coverVersion}`;
	}
	async importCourseCover(courseId: string, file?: File | null) {
		if (!file) throw new Error('Escolha uma imagem para a capa.');
		// O limite é verificado antes de enviar: um arquivo grande demais nunca ocupa a rede.
		if (file.size > MAX_COVER_BYTES) throw new Error('A imagem é maior que 4 MiB.');
		const response = await fetch(`/api/admin/courses/${encodeURIComponent(courseId)}/cover`, { method: 'PUT', body: file });
		if (!response.ok) throw new Error(coverUploadErrorMessage(response.status));
	}
	async removeCourseCover(courseId: string) {
		const response = await fetch(`/api/admin/courses/${encodeURIComponent(courseId)}/cover`, { method: 'DELETE' });
		if (!response.ok) throw new Error(response.status === 403 ? 'credentials_change_required' : response.status === 404 ? 'Curso não encontrado.' : 'Não foi possível remover a capa do curso.');
		return removalFlag(response);
	}
	async setCourseOrder(courseId: string, parent: string, children: string[]) {
		const response = await fetch(`/api/admin/courses/${encodeURIComponent(courseId)}/order`, { method: 'PUT', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ parent, children }) });
		if (!response.ok) throw new Error(orderErrorMessage(response.status));
	}
	async resetCourseOrder(courseId: string, parent?: string) {
		const query = parent === undefined ? '' : `?parent=${encodeURIComponent(parent)}`;
		const response = await fetch(`/api/admin/courses/${encodeURIComponent(courseId)}/order${query}`, { method: 'DELETE' });
		if (!response.ok) throw new Error(orderErrorMessage(response.status));
		return removalFlag(response);
	}
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
	invalid_position: 'Posição inválida.',
	course_not_found: 'Curso não encontrado.',
	invalid_cover: 'A capa precisa ser PNG, JPEG ou WebP de até 4 MiB.',
	cover_unavailable: 'Não foi possível atualizar a capa do curso.',
	invalid_order: 'A ordem enviada não é válida; recarregue a página.',
	order_unavailable: 'Não foi possível salvar a ordem do curso.',
	invalid_focus_session: 'Os dados da sessão de foco não são válidos.',
	focus_session_conflict: 'Esta sessão já foi registrada com outros dados.',
	mission_not_complete: 'Conclua a missão antes de resgatá-la.',
	invalid_weekly_goal: 'A meta semanal precisa ser de 1 a 7 dias.',
	update_check_failed: 'Não foi possível conectar ao servidor de atualizações.',
	update_unavailable: 'Não foi possível verificar ou aplicar a atualização.'
};

// O comando devolve o erro como objeto serializado `{ code, message }`, não como `Error`.
function desktopError(cause: unknown, fallback: string): Error {
	const payload = cause as { code?: unknown } | null;
	if (payload && typeof payload === 'object' && payload.code === 'focus_session_conflict') return new FocusSessionConflict();
	if (payload && typeof payload === 'object' && payload.code === 'invalid_focus_session') return new InvalidFocusSession();
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
	async studySummary() { try { return normalizeStudySummary(await invoke<unknown>('desktop_study_summary')); } catch (cause) { throw desktopError(cause, 'Não foi possível carregar sua evolução.'); } }
	listNotes(_mediaId?: string): Promise<StudyNote[]> { return Promise.resolve([]); }
	createNote(mediaId: string, positionSeconds: number, content: string): Promise<StudyNote> {
		return Promise.resolve({ noteId: 'desktop-stub', mediaId, positionSeconds, content, createdAt: 0, updatedAt: 0 });
	}
	updateNote(noteId: string, content: string): Promise<StudyNote> {
		return Promise.resolve({ noteId, mediaId: '', positionSeconds: 0, content, createdAt: 0, updatedAt: 0 });
	}
	deleteNote(_noteId: string): Promise<void> { return Promise.resolve(); }
	listFlashcards(_mediaId: string): Promise<Flashcard[]> { return Promise.resolve([]); }
	createFlashcard(mediaId: string, front: string, back: string): Promise<Flashcard> {
		return Promise.resolve({ cardId: 'desktop-stub', mediaId, front, back, intervalDays: 0, nextDue: 0, createdAt: 0, updatedAt: 0 });
	}
	updateFlashcard(cardId: string, front: string, back: string): Promise<Flashcard> {
		return Promise.resolve({ cardId, mediaId: '', front, back, intervalDays: 0, nextDue: 0, createdAt: 0, updatedAt: 0 });
	}
	deleteFlashcard(_cardId: string): Promise<void> { return Promise.resolve(); }
	reviewFlashcard(cardId: string, _rating: ReviewRating): Promise<Flashcard> {
		return Promise.resolve({ cardId, mediaId: '', front: '', back: '', intervalDays: 0, nextDue: 0, createdAt: 0, updatedAt: 0 });
	}
	async recordFocusSession(session: FocusSessionInput) { try { return normalizeFocusSession(await invoke<unknown>('desktop_record_focus_session', { session })); } catch (cause) { throw desktopError(cause, 'Não foi possível salvar a sessão de foco.'); } }
	listFocusSessions(_limit?: number): Promise<FocusSessionRecord[]> { return Promise.resolve([]); }
	async focusSummary() { try { return normalizeFocusSummary(await invoke<unknown>('desktop_focus_summary')); } catch (cause) { throw desktopError(cause, 'Não foi possível carregar as estatísticas de foco.'); } }
	async rewardSummary() { try { return normalizeRewardSummary(await invoke<unknown>('desktop_reward_summary')); } catch (cause) { throw desktopError(cause, 'Não foi possível carregar suas recompensas.'); } }
	async claimDailyMission(missionId: MissionId) { try { return await invoke<boolean>('desktop_claim_daily_mission', { missionId }); } catch (cause) { throw desktopError(cause, 'Não foi possível resgatar a missão.'); } }
	async setWeeklyGoal(goal: number) { try { if (!Number.isInteger(goal) || goal < 1 || goal > 7) throw new Error(); const saved = await invoke<unknown>('desktop_set_weekly_goal', { weeklyGoal: goal }); if (!Number.isInteger(saved) || (saved as number) < 1 || (saved as number) > 7) throw invalidPayload('meta semanal inválida.'); return saved as number; } catch (cause) { if (cause instanceof Error && cause.message.startsWith('Resposta inválida')) throw cause; throw desktopError(cause, 'Não foi possível salvar a meta semanal.'); } }
	userAvatarUrl(_version?: number | null) { return ''; }
	uploadUserAvatar(_file: File | Blob): Promise<{ version: number }> { return Promise.resolve({ version: 1 }); }
	deleteUserAvatar(): Promise<void> { return Promise.resolve(); }
	// O desktop verifica atualizações oficiais via plugin oficial do Tauri 2 e IPC nativo.
	async checkUpdate(_force = false): Promise<UpdateStatus> {
		try {
			const info = await invoke<{
				available: boolean;
				currentVersion: string;
				version: string | null;
				body: string | null;
				date: string | null;
			}>('desktop_check_update');
			return {
				current: info.currentVersion,
				enabled: true,
				available: info.available,
				latest: info.version ?? null,
				releasedAt: info.date ?? null,
				url: info.version ? `https://github.com/AndersonTavares0/LearnVault/releases/tag/v${encodeURIComponent(info.version.replace(/^v/, ''))}` : null,
				changelog: info.body ?? null,
				checkedAt: new Date().toISOString(),
				error: null
			};
		} catch (cause) {
			const err = desktopError(cause, 'Falha ao verificar atualização.');
			return {
				current: '',
				enabled: true,
				available: false,
				latest: null,
				releasedAt: null,
				url: null,
				changelog: null,
				checkedAt: new Date().toISOString(),
				error: err.message
			};
		}
	}
	async installUpdate(): Promise<boolean> {
		try {
			return await invoke<boolean>('desktop_install_update');
		} catch (cause) {
			throw desktopError(cause, 'Não foi possível instalar a atualização.');
		}
	}
	updateHistory(): Promise<UpdateHistoryEntry[]> { return Promise.resolve([]); }
	async mediaCapabilities(): Promise<MediaCapabilities> {
		try {
			const res = await invoke<{ transcoding?: boolean; systemPlayer?: boolean }>('desktop_media_capabilities');
			return {
				transcoding: Boolean(res?.transcoding),
				systemPlayer: Boolean(res?.systemPlayer ?? true)
			};
		} catch {
			return { transcoding: false, systemPlayer: true };
		}
	}
	// Toda mídia entregue ao player usa a identidade autorizada. O caminho catalogado é
	// deliberadamente ignorado, inclusive no desktop, para que ele nunca atravesse o webview.
	mediaUrl(file: FileEntry, _courseId?: string) {
		if (!file.mediaId || !['media', 'subtitle', 'material'].includes(file.kind ?? '')) return null;
		return desktopMediaUrl(file.mediaId, navigator.userAgent);
	}
	transcodeUrl(_file: FileEntry, _courseId?: string, _startSeconds: number = 0, _reencode: boolean = false) {
		return null;
	}
	async openInSystemPlayer(file: FileEntry): Promise<boolean> {
		if (!file.mediaId) return false;
		try {
			return await invoke<boolean>('desktop_open_in_system_player', { mediaId: file.mediaId });
		} catch {
			return false;
		}
	}
	// A capa é endereçada pela identidade opaca do curso no protocolo `learnvault-cover://`,
	// com a mesma inversão de prefixo por plataforma da mídia.
	coverUrl(courseId: string, coverVersion?: number | null) {
		if (coverVersion === undefined || coverVersion === null) return null;
		return `${desktopCoverUrl(courseId, navigator.userAgent)}?v=${coverVersion}`;
	}
	// O seletor nativo roda no Rust: o webview envia apenas a identidade do curso e nenhum
	// byte de arquivo cruza o IPC. O parâmetro é mantido para o contrato comum dos transportes.
	async importCourseCover(courseId: string, _file?: File | null) { try { await invoke<unknown>('desktop_import_course_cover', { courseId }); } catch (cause) { throw desktopError(cause, 'Não foi possível salvar a capa do curso.'); } }
	async removeCourseCover(courseId: string) { try { const result = await invoke<{ removed?: unknown } | null>('desktop_remove_course_cover', { courseId }); return result?.removed === true; } catch (cause) { throw desktopError(cause, 'Não foi possível remover a capa do curso.'); } }
	// A ordem é endereçada pela identidade opaca do curso e por caminhos relativos já
	// catalogados; nenhum caminho do sistema de arquivos cruza o IPC.
	async setCourseOrder(courseId: string, parent: string, children: string[]) { try { await invoke<unknown>('desktop_set_course_order', { courseId, parent, children }); } catch (cause) { throw desktopError(cause, 'Não foi possível salvar a ordem.'); } }
	// Sem `parent`, o comando restaura o curso inteiro; o argumento é omitido em vez de
	// enviado como `undefined`.
	async resetCourseOrder(courseId: string, parent?: string) { try { const result = await invoke<boolean | null>('desktop_reset_course_order', parent === undefined ? { courseId } : { courseId, parent }); return result === true; } catch (cause) { throw desktopError(cause, 'Não foi possível restaurar a ordem.'); } }
}

export const transportKind: TransportKind = __LEARNVAULT_TRANSPORT__ === 'desktop' ? 'desktop' : 'http';
export const transport: Transport = transportKind === 'desktop' ? new DesktopStub() : new HttpTransport();
