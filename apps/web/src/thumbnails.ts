import type { CatalogNode, FileEntry } from './transport';

const VIDEO_EXTENSIONS = new Set(['mp4', 'webm', 'ogv']);
export const THUMBNAIL_LIMITS = { maxEntries: 256, maxBytes: 64 * 1024 * 1024, maxImageBytes: 256 * 1024 } as const;
export const MAX_CONCURRENT_THUMBNAILS = 2;
export type ThumbnailCacheEntry = { key: string; blob: Blob; lastUsed: number };
export interface ThumbnailCacheAdapter {
	get(key: string): Promise<ThumbnailCacheEntry | null>;
	put(entry: ThumbnailCacheEntry): Promise<void>;
	list(): Promise<ThumbnailCacheEntry[]>;
	delete(key: string): Promise<void>;
}

/** Profile component of the canonical JSON cache key; legacy/test keys share an isolated scope. */
function profileScope(key: string): string {
	try {
		const parts: unknown = JSON.parse(key);
		return Array.isArray(parts) && typeof parts[0] === 'string' ? parts[0] : '';
	} catch {
		return '';
	}
}

/** IndexedDB-backed adapter; each operation resolves only after its transaction commits. */
export class IndexedDbThumbnailCacheAdapter implements ThumbnailCacheAdapter {
	private database: Promise<IDBDatabase> | null = null;

	constructor(
		private readonly factory: IDBFactory = indexedDB,
		private readonly databaseName = 'learnvault-course-thumbnails'
	) {}

	async get(key: string): Promise<ThumbnailCacheEntry | null> {
		return this.transact('readonly', (store) => store.get(key));
	}

	async put(entry: ThumbnailCacheEntry): Promise<void> {
		await this.transact('readwrite', (store) => store.put(entry));
	}

	async list(): Promise<ThumbnailCacheEntry[]> {
		return this.transact('readonly', (store) => store.getAll());
	}

	async delete(key: string): Promise<void> {
		await this.transact('readwrite', (store) => store.delete(key));
	}

	private open(): Promise<IDBDatabase> {
		if (this.database) return this.database;
		const opening = new Promise<IDBDatabase>((resolve, reject) => {
			let request: IDBOpenDBRequest;
			try {
				request = this.factory.open(this.databaseName, 1);
			} catch (error) {
				reject(error);
				return;
			}
			request.onupgradeneeded = () => {
				const db = request.result;
				if (!db.objectStoreNames.contains('thumbnails')) {
					db.createObjectStore('thumbnails', { keyPath: 'key' });
				}
			};
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error ?? new Error('Não foi possível abrir o cache de miniaturas.'));
			request.onblocked = () => reject(new Error('A abertura do cache de miniaturas foi bloqueada.'));
		}).catch((error: unknown) => {
			this.database = null;
			throw error;
		});
		this.database = opening;
		return opening;
	}

	private async transact<T>(
		mode: IDBTransactionMode,
		operation: (store: IDBObjectStore) => IDBRequest<T>
	): Promise<T> {
		const db = await this.open();
		const transaction = db.transaction('thumbnails', mode);
		const committed = new Promise<void>((resolve, reject) => {
			transaction.oncomplete = () => resolve();
			transaction.onerror = () => reject(transaction.error ?? new Error('Transação do cache falhou.'));
			transaction.onabort = () => reject(transaction.error ?? new Error('Transação do cache foi cancelada.'));
		});
		let request: IDBRequest<T>;
		try {
			request = operation(transaction.objectStore('thumbnails'));
		} catch (error) {
			try { transaction.abort(); } catch { /* transaction may already be inactive */ }
			throw error;
		}
		const result = new Promise<T>((resolve, reject) => {
			request.onsuccess = () => resolve(request.result);
			request.onerror = () => reject(request.error ?? new Error('Operação do cache falhou.'));
		});
		const [value] = await Promise.all([result, committed]);
		return value;
	}
}

export function thumbnailCacheKey(profile: string, courseId: string, source: FileEntry): string {
	if (
		!profile || !courseId || !source.mediaId ||
		!Number.isSafeInteger(source.size) || source.size! <= 0 ||
		!Number.isSafeInteger(source.mtimeSeconds) || source.mtimeSeconds! < 0
	) throw new Error('Fonte sem revisão válida para cache.');
	return JSON.stringify([profile, courseId, source.mediaId, source.size, source.mtimeSeconds, 1]);
}

export class ThumbnailCache {
	private lastUsed = 0;

	constructor(
		private readonly adapter: ThumbnailCacheAdapter,
		private readonly limits: Partial<Record<keyof typeof THUMBNAIL_LIMITS, number>> = THUMBNAIL_LIMITS
	) {}

	async get(key: string): Promise<Blob | null> {
		try {
			const entry = await this.adapter.get(key);
			if (!entry) return null;
			if (!(entry.blob instanceof Blob) || entry.blob.size === 0 || entry.blob.size > this.limit('maxImageBytes') || !Number.isFinite(entry.lastUsed)) {
				await this.adapter.delete(key);
				return null;
			}
			const touched = { ...entry, lastUsed: this.nextTimestamp() };
			await this.adapter.put(touched);
			return touched.blob;
		} catch {
			return null;
		}
	}

	async put(key: string, blob: Blob): Promise<boolean> {
		if (blob.size === 0 || blob.size > this.limit('maxImageBytes')) return false;
		try {
			await this.adapter.put({ key, blob, lastUsed: this.nextTimestamp() });
			const entries = await this.adapter.list();
			const scopedEntries = entries.filter((entry) => profileScope(entry.key) === profileScope(key));
			scopedEntries.sort((a, b) => a.lastUsed - b.lastUsed || a.key.localeCompare(b.key));
			let totalBytes = scopedEntries.reduce((total, entry) => total + entry.blob.size, 0);
			while (scopedEntries.length > this.limit('maxEntries') || totalBytes > this.limit('maxBytes')) {
				const oldest = scopedEntries.shift();
				if (!oldest) break;
				await this.adapter.delete(oldest.key);
				totalBytes -= oldest.blob.size;
			}
			return scopedEntries.some((entry) => entry.key === key);
		} catch {
			return false;
		}
	}

	private nextTimestamp(): number {
		this.lastUsed = Math.max(Date.now(), this.lastUsed + 1);
		return this.lastUsed;
	}

	private limit(key: keyof typeof THUMBNAIL_LIMITS): number {
		return this.limits[key] ?? THUMBNAIL_LIMITS[key];
	}
}

/** Fila compartilhada que limita decodificações concorrentes e libera vagas após erros. */
export class ThumbnailTaskQueue {
	private active = 0;
	private readonly waiters: Array<() => void> = [];

	constructor(private readonly concurrency: number = MAX_CONCURRENT_THUMBNAILS) {
		if (!Number.isSafeInteger(concurrency) || concurrency < 1) throw new Error('Concorrência inválida.');
	}

	async run<T>(task: () => Promise<T>): Promise<T> {
		if (this.active < this.concurrency) this.active += 1;
		else await new Promise<void>((resolve) => this.waiters.push(resolve));
		try {
			return await task();
		} finally {
			const next = this.waiters.shift();
			if (next) next();
			else this.active -= 1;
		}
	}
}

/** Busca o cache antes de decodificar e só persiste capturas válidas. */
export async function getOrCreateThumbnail(
	cache: ThumbnailCache,
	queue: ThumbnailTaskQueue,
	key: string,
	mediaUrl: string,
	capture: (mediaUrl: string) => Promise<Blob | null>
): Promise<Blob | null> {
	const cached = await cache.get(key);
	if (cached) return cached;
	return queue.run(async () => {
		const afterWaiting = await cache.get(key);
		if (afterWaiting) return afterWaiting;
		const blob = await capture(mediaUrl);
		if (blob) await cache.put(key, blob);
		return blob;
	});
}

/** Svelte action that starts thumbnail work once the card is visible. */
export function observeThumbnailVisibility(
	target: Element,
	load: () => void | Promise<void>,
	createObserver: ((callback: IntersectionObserverCallback) => IntersectionObserver) | undefined =
		typeof IntersectionObserver === 'undefined'
			? undefined
			: (callback) => new IntersectionObserver(callback)
): { destroy(): void } {
	let active = true;
	let observer: IntersectionObserver | null = null;
	const start = () => {
		if (!active) return;
		active = false;
		observer?.disconnect();
		try {
			void Promise.resolve(load()).catch(() => {});
		} catch {
			// Thumbnail generation is optional; failures retain the generated cover.
		}
	};

	if (!createObserver) {
		start();
	} else {
		observer = createObserver((entries) => {
			if (entries.some((entry) => entry.target === target && (entry.isIntersecting || entry.intersectionRatio > 0))) {
				start();
			}
		});
		observer.observe(target);
	}

	return {
		destroy() {
			active = false;
			observer?.disconnect();
			observer = null;
		}
	};
}

/** Seleciona o primeiro vídeo confiável seguindo a ordem canônica da árvore. */
export function selectThumbnailSource(course: CatalogNode): FileEntry | null {
	function visit(nodes: CatalogNode[]): FileEntry | null {
		for (const node of nodes) {
			if (node.role === 'lesson') {
				for (const file of node.files) {
					const extension = file.name.split('.').pop()?.toLowerCase();
					if (
						file.kind === 'media' &&
						file.availability?.trim().toLowerCase() !== 'missing' &&
						!file.unverified &&
						file.mediaId &&
						file.mtimeSeconds !== undefined &&
						Number.isSafeInteger(file.mtimeSeconds) &&
						file.mtimeSeconds >= 0 &&
						file.size !== undefined &&
						Number.isSafeInteger(file.size) &&
						file.size > 0 &&
						extension && VIDEO_EXTENSIONS.has(extension)
					) return file;
				}
			}
			const child = visit(node.children);
			if (child) return child;
		}
		return null;
	}

	return visit(course.children);
}

export function captureTimeSeconds(durationSeconds: number): number | null {
	if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return null;
	return Math.min(durationSeconds * 0.1, 10);
}

export function fitThumbnailDimensions(
	width: number,
	height: number,
	maxWidth = 320,
	maxHeight = 180
): { width: number; height: number } | null {
	if (
		![width, height, maxWidth, maxHeight].every(Number.isFinite) ||
		width <= 0 || height <= 0 || maxWidth <= 0 || maxHeight <= 0
	) {
		return null;
	}
	const scale = Math.min(maxWidth / width, maxHeight / height, 1);
	return {
		width: Math.max(1, Math.round(width * scale)),
		height: Math.max(1, Math.round(height * scale))
	};
}

export interface VideoCaptureDependencies {
	createVideo(): HTMLVideoElement;
	createCanvas(): HTMLCanvasElement;
	createObjectUrl(url: string): string;
	revokeObjectUrl(url: string): void;
	waitForMetadata(video: HTMLVideoElement): Promise<void>;
	waitForSeek(video: HTMLVideoElement, time: number): Promise<void>;
	waitForFrame(video: HTMLVideoElement): Promise<void>;
}

const browserCaptureDependencies: VideoCaptureDependencies = {
	createVideo: () => document.createElement('video'),
	createCanvas: () => document.createElement('canvas'),
	createObjectUrl: (url) => url,
	revokeObjectUrl: () => {},
	waitForMetadata: (video) => waitForMediaEvent(video, 'loadedmetadata', () => Number.isFinite(video.duration) && video.duration > 0),
	waitForSeek: (video, time) => {
		if (Math.abs(video.currentTime - time) < 0.01) return Promise.resolve();
		return waitForMediaEvent(video, 'seeked', () => Math.abs(video.currentTime - time) < 0.01, () => { video.currentTime = time; });
	},
	waitForFrame: (video) => {
		if (video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA) return Promise.resolve();
		return waitForMediaEvent(video, 'loadeddata', () => video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA);
	}
};

function waitForMediaEvent(
	video: HTMLVideoElement,
	event: string,
	ready: () => boolean,
	start?: () => void,
	timeoutMs = 8_000
): Promise<void> {
	return new Promise((resolve, reject) => {
		const cleanup = () => {
			clearTimeout(timeout);
			video.removeEventListener(event, onEvent);
			video.removeEventListener('error', onError);
		};
		const onEvent = () => {
			if (!ready()) return;
			cleanup();
			resolve();
		};
		const onError = () => { cleanup(); reject(new Error('Falha ao decodificar vídeo.')); };
		const timeout = setTimeout(() => { cleanup(); reject(new Error('Tempo limite ao decodificar vídeo.')); }, timeoutMs);
		video.addEventListener(event, onEvent);
		video.addEventListener('error', onError);
		start?.();
		if (ready()) onEvent();
	});
}

function canvasToJpeg(canvas: HTMLCanvasElement): Promise<Blob | null> {
	return new Promise((resolve) => canvas.toBlob(resolve, 'image/jpeg', 0.82));
}

/** Decodifica uma mídia isolada e exporta um frame; qualquer falha preserva o fallback visual. */
export async function captureVideoThumbnail(
	mediaUrl: string,
	dependencies: VideoCaptureDependencies = browserCaptureDependencies
): Promise<Blob | null> {
	let video: HTMLVideoElement | null = null;
	let canvas: HTMLCanvasElement | null = null;
	let objectUrl: string | null = null;
	try {
		video = dependencies.createVideo();
		video.crossOrigin = 'anonymous';
		video.preload = 'auto';
		video.muted = true;
		video.playsInline = true;
		objectUrl = dependencies.createObjectUrl(mediaUrl);
		video.src = objectUrl;
		video.load();
		await dependencies.waitForMetadata(video);
		const time = captureTimeSeconds(video.duration);
		const dimensions = fitThumbnailDimensions(video.videoWidth, video.videoHeight);
		if (time === null || !dimensions) return null;
		await dependencies.waitForSeek(video, time);
		await dependencies.waitForFrame(video);
		video.pause();
		canvas = dependencies.createCanvas();
		canvas.width = dimensions.width;
		canvas.height = dimensions.height;
		const context = canvas.getContext('2d');
		if (!context) return null;
		context.drawImage(video, 0, 0, dimensions.width, dimensions.height);
		const blob = await canvasToJpeg(canvas);
		if (!blob || blob.type !== 'image/jpeg' || blob.size === 0 || blob.size > THUMBNAIL_LIMITS.maxImageBytes) return null;
		return blob;
	} catch {
		return null;
	} finally {
		try { video?.pause(); } catch { /* cleanup must not override the generated-cover fallback */ }
		try { video?.removeAttribute('src'); } catch { /* best-effort cleanup */ }
		try { video?.load(); } catch { /* best-effort cleanup */ }
		try { video?.remove(); } catch { /* best-effort cleanup */ }
		try { canvas?.remove(); } catch { /* best-effort cleanup */ }
		if (objectUrl !== null) {
			try { dependencies.revokeObjectUrl(objectUrl); } catch { /* best-effort cleanup */ }
		}
	}
}

/**
 * Selects and loads the course thumbnail without allowing optional media failures to escape.
 * Uma capa personalizada (upload da issue #237) tem precedência: quando presente, nenhuma
 * captura é feita, nem mesmo uma leitura de cache.
 */
export async function loadCourseThumbnail(options: {
	course: CatalogNode;
	profile: string;
	cache: ThumbnailCache;
	queue: ThumbnailTaskQueue;
	mediaUrl: (file: FileEntry, courseId: string) => string | null;
	capture?: (url: string) => Promise<Blob | null>;
	manualCoverUrl?: string | null;
}): Promise<Blob | null> {
	try {
		const { course, profile, cache, queue } = options;
		if (options.manualCoverUrl) return null;
		const source = selectThumbnailSource(course);
		if (!source) return null;
		const courseId = course.courseId;
		if (!courseId) return null;

		const key = thumbnailCacheKey(profile, courseId, source);
		const mediaUrl = options.mediaUrl(source, courseId);
		if (!mediaUrl) return null;

		return await getOrCreateThumbnail(cache, queue, key, mediaUrl, options.capture ?? captureVideoThumbnail);
	} catch {
		return null;
	}
}
