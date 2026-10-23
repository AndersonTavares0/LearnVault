export type LibraryView = 'grid' | 'list';

export type LibraryPreferences = {
	version: 1;
	displayTitles: Record<string, string>;
	view: LibraryView;
};

const STORAGE_KEY = 'learnvault-library.v1';
const COVER_DB = 'learnvault-course-covers.v1';
const COVER_STORE = 'covers';
const MAX_COVER_BYTES = 5 * 1024 * 1024;

export function emptyLibraryPreferences(): LibraryPreferences {
	return { version: 1, displayTitles: {}, view: 'list' };
}

export function libraryPreferencesKey(userId: string): string {
	return `${STORAGE_KEY}:${encodeURIComponent(userId)}`;
}

export function readLibraryPreferences(
	storage: Pick<Storage, 'getItem'> | null | undefined,
	userId: string
): LibraryPreferences {
	try {
		const raw = storage?.getItem(libraryPreferencesKey(userId));
		if (!raw) return emptyLibraryPreferences();
		const parsed = JSON.parse(raw) as Partial<LibraryPreferences>;
		const displayTitles = parsed.version === 1 && parsed.displayTitles && typeof parsed.displayTitles === 'object' && !Array.isArray(parsed.displayTitles)
			? Object.fromEntries(Object.entries(parsed.displayTitles).filter(([id, title]) => Boolean(id) && typeof title === 'string' && title.trim().length > 0 && title.trim().length <= 120).map(([id, title]) => [id, (title as string).trim()]))
			: {};
		return { version: 1, displayTitles, view: parsed.view === 'grid' ? 'grid' : 'list' };
	} catch {
		return emptyLibraryPreferences();
	}
}

export function writeLibraryPreferences(
	storage: Pick<Storage, 'setItem'> | null | undefined,
	userId: string,
	preferences: LibraryPreferences
): boolean {
	if (!storage) return false;
	try {
		storage.setItem(libraryPreferencesKey(userId), JSON.stringify(preferences));
		return true;
	} catch {
		return false;
	}
}

export function displayTitleFor(title: string): string {
	const cleaned = title
		.trim()
		.replace(/^(?:\d+[._ -]+)+/u, '')
		.replace(/([\p{L}])([\d])/gu, '$1 $2')
		.replace(/([\d])([\p{L}])/gu, '$1 $2')
		.replace(/([\p{Ll}\d])([\p{Lu}])/gu, '$1 $2')
		.replace(/[._-]+/gu, ' ')
		.replace(/\s+/gu, ' ')
		.trim();
	return cleaned || title.trim() || 'Curso';
}

export function validateDisplayTitle(value: string): string | null {
	const title = value.trim();
	if (!title) return 'Informe um título.';
	if (title.length > 120) return 'Use no máximo 120 caracteres.';
	if (/[\u0000-\u001f\u007f]/u.test(title)) return 'O título contém caracteres inválidos.';
	return null;
}

function coverKey(userId: string, courseId: string): string {
	return `${userId}\u0000${courseId}`;
}

function openCoverDatabase(): Promise<IDBDatabase> {
	if (typeof indexedDB === 'undefined') return Promise.reject(new Error('O armazenamento de capas não está disponível neste navegador.'));
	return new Promise((resolve, reject) => {
		const request = indexedDB.open(COVER_DB, 1);
		request.onupgradeneeded = () => {
			if (!request.result.objectStoreNames.contains(COVER_STORE)) request.result.createObjectStore(COVER_STORE, { keyPath: 'key' });
		};
		request.onsuccess = () => resolve(request.result);
		request.onerror = () => reject(new Error('Não foi possível abrir o armazenamento local de capas.'));
	});
}

export async function readLibraryCover(userId: string, courseId: string): Promise<Blob | null> {
	const database = await openCoverDatabase();
	try {
		return await new Promise((resolve, reject) => {
			const request = database.transaction(COVER_STORE, 'readonly').objectStore(COVER_STORE).get(coverKey(userId, courseId));
			request.onsuccess = () => resolve((request.result?.blob as Blob | undefined) ?? null);
			request.onerror = () => reject(new Error('Não foi possível ler a capa personalizada.'));
		});
	} finally {
		database.close();
	}
}

export async function writeLibraryCover(userId: string, courseId: string, file: File): Promise<void> {
	if (!file.type.startsWith('image/')) throw new Error('Escolha um arquivo de imagem.');
	if (file.size > MAX_COVER_BYTES) throw new Error('A imagem deve ter até 5 MB.');
	const database = await openCoverDatabase();
	try {
		await new Promise<void>((resolve, reject) => {
			const transaction = database.transaction(COVER_STORE, 'readwrite');
			transaction.objectStore(COVER_STORE).put({ key: coverKey(userId, courseId), blob: file.slice(0, file.size, file.type) });
			transaction.oncomplete = () => resolve();
			transaction.onerror = () => reject(new Error('Não foi possível salvar a capa neste dispositivo.'));
			transaction.onabort = () => reject(new Error('Não foi possível salvar a capa neste dispositivo.'));
		});
	} finally {
		database.close();
	}
}

export async function removeLibraryCover(userId: string, courseId: string): Promise<void> {
	const database = await openCoverDatabase();
	try {
		await new Promise<void>((resolve, reject) => {
			const transaction = database.transaction(COVER_STORE, 'readwrite');
			transaction.objectStore(COVER_STORE).delete(coverKey(userId, courseId));
			transaction.oncomplete = () => resolve();
			transaction.onerror = () => reject(new Error('Não foi possível remover a capa deste dispositivo.'));
		});
	} finally {
		database.close();
	}
}
