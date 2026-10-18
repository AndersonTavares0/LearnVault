import { describe, expect, it, vi } from 'vitest';
import { IndexedDbThumbnailCacheAdapter, ThumbnailCache, ThumbnailTaskQueue, thumbnailCacheKey, selectThumbnailSource, captureTimeSeconds, fitThumbnailDimensions, captureVideoThumbnail, getOrCreateThumbnail, observeThumbnailVisibility, loadCourseThumbnail, type ThumbnailCacheAdapter, type ThumbnailCacheEntry, type VideoCaptureDependencies } from './thumbnails';
import type { CatalogNode } from './transport';

function lesson(title: string, files: CatalogNode['files']): CatalogNode {
	return { title, path: title, role: 'lesson', files, looseFiles: [], children: [] };
}

function file(name: string, overrides: Partial<CatalogNode['files'][number]> = {}): CatalogNode['files'][number] {
	return { name, path: name, size: 100, mediaId: name, mtimeSeconds: 1, kind: 'media', availability: 'available', ...overrides };
}

function course(children: CatalogNode[]): CatalogNode {
	return { title: 'Course', path: 'course-id', role: 'course', courseId: 'course-id', files: [], looseFiles: [], children };
}

describe('automatic thumbnail source selection', () => {
	it('uses the first eligible video in canonical depth-first lesson order', () => {
		const tree = course([
			{ title: 'Module', path: 'module', role: 'module', files: [], looseFiles: [], children: [
				lesson('Lesson 1', [file('notes.pdf'), file('audio.mp3'), file('intro.mp4')]),
				lesson('Lesson 2', [file('next.webm')])
			] }
		]);

		expect(selectThumbnailSource(tree)?.mediaId).toBe('intro.mp4');
	});

	it('skips unavailable, unverified, unsupported, or revisionless videos', () => {
		const tree = course([
			lesson('Lesson 1', [
				file('missing.mp4', { availability: 'missing' }),
				file('uncertain.mp4', { unverified: true }),
				file('old.mp4', { mtimeSeconds: undefined }),
				file('movie.mkv')
			]),
			lesson('Lesson 2', [file('valid.mp4')])
		]);

		expect(selectThumbnailSource(tree)?.mediaId).toBe('valid.mp4');
	});

	it('skips videos with missing, zero, or unsafe oversized file sizes', () => {
		const tree = course([
			lesson('Lesson 1', [
				file('missing-size.mp4', { size: undefined }),
				file('empty.mp4', { size: 0 }),
				file('oversized.mp4', { size: Number.MAX_SAFE_INTEGER + 1 })
			]),
			lesson('Lesson 2', [file('valid.mp4')])
		]);

		expect(selectThumbnailSource(tree)?.mediaId).toBe('valid.mp4');
	});

	it('returns no source when the course has no eligible lesson video', () => {
		expect(selectThumbnailSource(course([lesson('Lesson', [file('sound.ogg')])]))).toBeNull();
	});
});

class MemoryThumbnailCache implements ThumbnailCacheAdapter {
	readonly entries = new Map<string, ThumbnailCacheEntry>();
	async get(key: string) { return this.entries.get(key) ?? null; }
	async put(entry: ThumbnailCacheEntry) { this.entries.set(entry.key, entry); }
	async list() { return [...this.entries.values()]; }
	async delete(key: string) { this.entries.delete(key); }
}

describe('bounded thumbnail cache', () => {
	it('separates profiles and invalidates when source revision changes', async () => {
		const source = file('lesson.mp4', { mediaId: 'media-1', size: 500, mtimeSeconds: 123 });
		const first = thumbnailCacheKey('user-a', 'course-1', source);
		const otherProfile = thumbnailCacheKey('user-b', 'course-1', source);
		const changedSource = thumbnailCacheKey('user-a', 'course-1', { ...source, mtimeSeconds: 124 });
		expect(first).not.toBe(otherProfile);
		expect(first).not.toBe(changedSource);
		expect(() => thumbnailCacheKey('user-a', 'course-1', { ...source, mediaId: '' })).toThrow();
		expect(() => thumbnailCacheKey('user-a', 'course-1', { ...source, size: 0 })).toThrow();
		expect(() => thumbnailCacheKey('user-a', 'course-1', { ...source, mtimeSeconds: -1 })).toThrow();

		const adapter = new MemoryThumbnailCache();
		const cache = new ThumbnailCache(adapter);
		await cache.put(first, new Blob(['jpeg']));
		expect(await cache.get(first)).not.toBeNull();
		expect(await cache.get(otherProfile)).toBeNull();
		expect(await cache.get(changedSource)).toBeNull();
	});

	it('rejects oversized entries and evicts least-recently-used entries to stay bounded', async () => {
		const adapter = new MemoryThumbnailCache();
		const cache = new ThumbnailCache(adapter, { maxEntries: 2, maxBytes: 8, maxImageBytes: 5 });
		await cache.put('one', new Blob(['1111']));
		await cache.put('two', new Blob(['2222']));
		await cache.get('one');
		await cache.put('three', new Blob(['3333']));
		expect(await cache.get('two')).toBeNull();
		expect(await cache.get('one')).not.toBeNull();
		expect(await cache.get('three')).not.toBeNull();
		expect(await cache.put('oversized', new Blob(['123456']))).toBe(false);
		expect(await cache.get('oversized')).toBeNull();
	});

	it('enforces entry and byte limits independently for each profile', async () => {
		const adapter = new MemoryThumbnailCache();
		const cache = new ThumbnailCache(adapter, { maxEntries: 1, maxBytes: 8, maxImageBytes: 4 });
		const source = file('lesson.mp4', { mediaId: 'media-1', size: 500, mtimeSeconds: 123 });
		const profileA1 = thumbnailCacheKey('user-a', 'course-1', source);
		const profileA2 = thumbnailCacheKey('user-a', 'course-2', source);
		const profileB = thumbnailCacheKey('user-b', 'course-1', source);

		await cache.put(profileA1, new Blob(['1111']));
		await cache.put(profileB, new Blob(['2222']));
		await cache.put(profileA2, new Blob(['3333']));

		expect(await cache.get(profileA1)).toBeNull();
		expect(await cache.get(profileA2)).not.toBeNull();
		expect(await cache.get(profileB)).not.toBeNull();
	});

	it('enforces the byte quota independently for each profile', async () => {
		const adapter = new MemoryThumbnailCache();
		const cache = new ThumbnailCache(adapter, { maxEntries: 5, maxBytes: 4, maxImageBytes: 4 });
		const source = file('lesson.mp4', { mediaId: 'media-1', size: 500, mtimeSeconds: 123 });
		const profileA1 = thumbnailCacheKey('user-a', 'course-1', source);
		const profileA2 = thumbnailCacheKey('user-a', 'course-2', source);
		const profileB = thumbnailCacheKey('user-b', 'course-1', source);

		await cache.put(profileA1, new Blob(['1111']));
		await cache.put(profileB, new Blob(['2222']));
		await cache.put(profileA2, new Blob(['3333']));

		expect(await cache.get(profileA1)).toBeNull();
		expect(await cache.get(profileA2)).not.toBeNull();
		expect(await cache.get(profileB)).not.toBeNull();
	});

	it('removes invalid stored entries and reports false when limits evict the new item', async () => {
		const adapter = new MemoryThumbnailCache();
		adapter.entries.set('corrupt', { key: 'corrupt', blob: new Blob(['123456']), lastUsed: 0 });
		const cache = new ThumbnailCache(adapter, { maxEntries: 2, maxBytes: 8, maxImageBytes: 5 });
		expect(await cache.get('corrupt')).toBeNull();
		expect(adapter.entries.has('corrupt')).toBe(false);

		const tiny = new ThumbnailCache(adapter, { maxEntries: 0, maxBytes: 0, maxImageBytes: 5 });
		expect(await tiny.put('new', new Blob(['x']))).toBe(false);
		expect(await tiny.get('new')).toBeNull();
	});

	it('degrades to cache misses when storage is unavailable', async () => {
		const broken: ThumbnailCacheAdapter = {
			get: async () => { throw new Error('storage unavailable'); },
			put: async () => { throw new Error('storage unavailable'); },
			list: async () => { throw new Error('storage unavailable'); },
			delete: async () => { throw new Error('storage unavailable'); }
		};
		const cache = new ThumbnailCache(broken);
		expect(await cache.get('key')).toBeNull();
		await expect(cache.put('key', new Blob(['jpeg']))).resolves.toBe(false);
	});
});

describe('IndexedDB thumbnail adapter', () => {
	function indexedDbHarness(options: { transactionError?: boolean } = {}) {
		const records = new Map<string, ThumbnailCacheEntry>();
		const transactionEvents: string[] = [];
		const db = {
			objectStoreNames: { contains: () => true },
			createObjectStore: () => undefined,
			close: () => undefined,
			transaction: () => {
				const tx = {
					oncomplete: null as (() => void) | null,
					onerror: null as (() => void) | null,
					onabort: null as (() => void) | null,
					objectStore: () => ({
						get: (key: string) => request(() => records.get(key) ?? null),
						put: (entry: ThumbnailCacheEntry) => request(() => { records.set(entry.key, entry); return entry.key; }),
						getAll: () => request(() => [...records.values()]),
						delete: (key: string) => request(() => { records.delete(key); return undefined; })
					})
				};
				queueMicrotask(() => {
					if (options.transactionError) {
						transactionEvents.push('error');
						tx.onerror?.();
					} else {
						transactionEvents.push('complete');
						tx.oncomplete?.();
					}
				});
				return tx;
			}
		};
		function request<T>(value: () => T) {
			const result = { result: undefined as T | undefined, error: null as DOMException | null, onsuccess: null as (() => void) | null, onerror: null as (() => void) | null };
			queueMicrotask(() => { result.result = value(); result.onsuccess?.(); });
			return result;
		}
		const factory = {
			open: () => {
				const open = { result: db, error: null as DOMException | null, onupgradeneeded: null as (() => void) | null, onsuccess: null as (() => void) | null, onerror: null as (() => void) | null };
				queueMicrotask(() => { open.onupgradeneeded?.(); open.onsuccess?.(); });
				return open;
			}
		};
		return { factory, records, transactionEvents };
	}

	it('reads, writes, lists, and deletes entries only after transaction completion', async () => {
		const harness = indexedDbHarness();
		const adapter = new IndexedDbThumbnailCacheAdapter(harness.factory as unknown as IDBFactory);
		const entry: ThumbnailCacheEntry = { key: 'profile/course/media', blob: new Blob(['jpeg'], { type: 'image/jpeg' }), lastUsed: 1 };
		await adapter.put(entry);
		expect(await adapter.get(entry.key)).toEqual(entry);
		expect(await adapter.list()).toEqual([entry]);
		await adapter.delete(entry.key);
		expect(await adapter.get(entry.key)).toBeNull();
		expect(harness.transactionEvents).toEqual(['complete', 'complete', 'complete', 'complete', 'complete']);
	});

	it('rejects when a transaction aborts even if the request itself succeeded', async () => {
		const harness = indexedDbHarness({ transactionError: true });
		const adapter = new IndexedDbThumbnailCacheAdapter(harness.factory as unknown as IDBFactory);
		await expect(adapter.put({ key: 'k', blob: new Blob(['jpeg']), lastUsed: 1 })).rejects.toThrow();
	});

	it('defers unavailable IndexedDB failure until cache use', async () => {
		vi.stubGlobal('indexedDB', undefined);
		try {
			const adapter = new IndexedDbThumbnailCacheAdapter();
			expect(adapter).toBeDefined();
			await expect(adapter.get('key')).rejects.toThrow();
		} finally {
			vi.unstubAllGlobals();
		}
	});
});

describe('thumbnail capture concurrency', () => {
	it('runs at most two captures concurrently and releases a slot after failure', async () => {
		const queue = new ThumbnailTaskQueue(2);
		let active = 0;
		let maximum = 0;
		const gates: Array<() => void> = [];
		const task = (fail = false) => queue.run(async () => {
			active += 1;
			maximum = Math.max(maximum, active);
			await new Promise<void>((resolve) => gates.push(resolve));
			active -= 1;
			if (fail) throw new Error('decode failed');
			return 'done';
		});

		const first = task(true);
		const second = task();
		const third = task();
		await Promise.resolve();
		expect(active).toBe(2);
		gates.shift()?.();
		await expect(first).rejects.toThrow('decode failed');
		await Promise.resolve();
		expect(active).toBe(2);
		gates.shift()?.();
		gates.shift()?.();
		await expect(Promise.all([second, third])).resolves.toEqual(['done', 'done']);
		expect(maximum).toBe(2);
	});
});

describe('thumbnail frame geometry', () => {
	it('samples ten percent of duration capped at ten seconds', () => {
		expect(captureTimeSeconds(50)).toBe(5);
		expect(captureTimeSeconds(200)).toBe(10);
		expect(captureTimeSeconds(5)).toBe(0.5);
	});

	it('fits dimensions within bounds without changing aspect ratio', () => {
		expect(fitThumbnailDimensions(1920, 1080)).toEqual({ width: 320, height: 180 });
		expect(fitThumbnailDimensions(640, 480)).toEqual({ width: 240, height: 180 });
	});

	it.each([[0, 10], [10, 0], [Number.NaN, 10], [10, Number.POSITIVE_INFINITY]])(
		'rejects invalid video dimensions %s x %s',
		(width, height) => expect(fitThumbnailDimensions(width, height)).toBeNull()
	);
});

describe('video frame capture', () => {
	function captureHarness(options: { duration?: number; frameReady?: boolean; blob?: Blob | null; taint?: boolean } = {}) {
		const events = { revoked: false, videoRemoved: false, canvasRemoved: false, drawn: false };
		const video = {
			crossOrigin: null as string | null,
			preload: '',
			muted: false,
			playsInline: false,
			currentTime: 0,
			videoWidth: 1920,
			videoHeight: 1080,
			duration: options.duration ?? 20,
			readyState: options.frameReady ? 2 : 1,
			paused: true,
			addEventListener() {},
			removeEventListener() {},
			removeAttribute() {},
			load() {},
			pause() {},
			remove() { events.videoRemoved = true; }
		};
		const canvas = {
			width: 0,
			height: 0,
			getContext: () => ({ drawImage: () => { events.drawn = true; if (options.taint) throw new DOMException('tainted', 'SecurityError'); } }),
			toBlob(callback: BlobCallback) { callback(options.blob === undefined ? new Blob(['jpeg'], { type: 'image/jpeg' }) : options.blob); },
			remove() { events.canvasRemoved = true; }
		};
		const deps: VideoCaptureDependencies = {
			createVideo: () => video as unknown as HTMLVideoElement,
			createCanvas: () => canvas as unknown as HTMLCanvasElement,
			createObjectUrl: () => 'blob:synthetic',
			revokeObjectUrl: () => { events.revoked = true; },
			waitForMetadata: async () => {},
			waitForFrame: async () => {},
			waitForSeek: async (_video, time) => { video.currentTime = time; }
		};
		return { deps, events, video, canvas };
	}

	it('captures a bounded JPEG frame at the chosen timestamp and cleans temporary elements', async () => {
		const harness = captureHarness();
		const blob = await captureVideoThumbnail('opaque-media-url', harness.deps);
		expect(harness.video.currentTime).toBe(2);
		expect(harness.canvas.width).toBe(320);
		expect(harness.canvas.height).toBe(180);
		expect(blob?.type).toBe('image/jpeg');
		expect(harness.events).toEqual({ revoked: true, videoRemoved: true, canvasRemoved: true, drawn: true });
	});

	it('fails closed on timeout, tainted canvas, and oversized output while always cleaning up', async () => {
		const timeout = captureHarness();
		timeout.deps.waitForMetadata = async () => { throw new Error('timeout'); };
		await expect(captureVideoThumbnail('opaque', timeout.deps)).resolves.toBeNull();
		expect(timeout.events.videoRemoved).toBe(true);

		const tainted = captureHarness({ taint: true });
		await expect(captureVideoThumbnail('opaque', tainted.deps)).resolves.toBeNull();
		expect(tainted.events.canvasRemoved).toBe(true);

		const oversized = captureHarness({ blob: new Blob([new Uint8Array(256 * 1024 + 1)], { type: 'image/jpeg' }) });
		await expect(captureVideoThumbnail('opaque', oversized.deps)).resolves.toBeNull();
		expect(oversized.events.revoked).toBe(true);
	});

	it('keeps capture failures non-fatal when cleanup itself throws', async () => {
		const harness = captureHarness();
		harness.deps.waitForMetadata = async () => { throw new Error('decode failed'); };
		harness.deps.revokeObjectUrl = () => { throw new Error('cleanup failed'); };
		await expect(captureVideoThumbnail('opaque', harness.deps)).resolves.toBeNull();
	});
});

describe('course thumbnail orchestration', () => {
	 it('does not resolve a media URL or capture when the course has no eligible source', async () => {
		const mediaUrl = vi.fn(() => '/should-not-be-used');
		const capture = vi.fn(async () => new Blob(['image'], { type: 'image/jpeg' }));
		const result = await loadCourseThumbnail({
			course: course([lesson('Lesson', [file('notes.pdf')])]),
			profile: 'user-1',
			cache: new ThumbnailCache(new MemoryThumbnailCache()),
			queue: new ThumbnailTaskQueue(),
			mediaUrl,
			capture
		});

		expect(result).toBeNull();
		expect(mediaUrl).not.toHaveBeenCalled();
		expect(capture).not.toHaveBeenCalled();
	});

	it('uses the transport URL, profile-scoped cache key, and shared capture queue', async () => {
		const selected = file('lesson.mp4');
		const cache = new ThumbnailCache(new MemoryThumbnailCache());
		const mediaUrl = vi.fn(() => '/api/media?opaque=only');
		const capture = vi.fn(async () => new Blob(['image'], { type: 'image/jpeg' }));
		const options = {
			course: course([lesson('Lesson', [selected])]),
			profile: 'user-1',
			cache,
			queue: new ThumbnailTaskQueue(),
			mediaUrl,
			capture
		};

		const first = await loadCourseThumbnail(options);
		const second = await loadCourseThumbnail({ ...options, profile: 'user-2' });

		expect(mediaUrl).toHaveBeenCalledWith(selected, 'course-id');
		expect(capture).toHaveBeenCalledTimes(2);
		expect(first?.type).toBe('image/jpeg');
		expect(second?.type).toBe('image/jpeg');
	});

	it('does not capture when transport cannot produce a media URL', async () => {
		const capture = vi.fn(async () => new Blob(['image'], { type: 'image/jpeg' }));
		const result = await loadCourseThumbnail({
			course: course([lesson('Lesson', [file('lesson.mp4')])]),
			profile: 'user-1',
			cache: new ThumbnailCache(new MemoryThumbnailCache()),
			queue: new ThumbnailTaskQueue(),
			mediaUrl: () => null,
			capture
		});

		expect(result).toBeNull();
		expect(capture).not.toHaveBeenCalled();
	});

	it('defers to a manual cover without capturing or resolving a media URL', async () => {
		const mediaUrl = vi.fn(() => '/should-not-be-used');
		const capture = vi.fn(async () => new Blob(['image'], { type: 'image/jpeg' }));
		const result = await loadCourseThumbnail({
			course: course([lesson('Lesson', [file('lesson.mp4')])]),
			profile: 'user-1',
			cache: new ThumbnailCache(new MemoryThumbnailCache()),
			queue: new ThumbnailTaskQueue(),
			mediaUrl,
			capture,
			manualCoverUrl: 'learnvault-media://uploaded-cover'
		});

		expect(result).toBeNull();
		expect(mediaUrl).not.toHaveBeenCalled();
		expect(capture).not.toHaveBeenCalled();
	});
});

describe('thumbnail loading', () => {
	it('uses cached images and queues only cache misses', async () => {
		const adapter = new MemoryThumbnailCache();
		const cache = new ThumbnailCache(adapter);
		const queue = new ThumbnailTaskQueue(1);
		const cached = new Blob(['cached'], { type: 'image/jpeg' });
		await cache.put('hit', cached);
		let captures = 0;
		const capture = async () => { captures += 1; return new Blob(['generated'], { type: 'image/jpeg' }); };

		await expect(getOrCreateThumbnail(cache, queue, 'hit', 'opaque-url', capture)).resolves.toEqual(cached);
		await expect(getOrCreateThumbnail(cache, queue, 'miss', 'opaque-url', capture)).resolves.toEqual(new Blob(['generated'], { type: 'image/jpeg' }));
		expect(captures).toBe(1);
		expect(await cache.get('miss')).not.toBeNull();
	});

	it('returns null and does not cache capture failures', async () => {
		const cache = new ThumbnailCache(new MemoryThumbnailCache());
		const result = await getOrCreateThumbnail(cache, new ThumbnailTaskQueue(), 'key', 'opaque-url', async () => null);
		expect(result).toBeNull();
		expect(await cache.get('key')).toBeNull();
	});
});

describe('lazy thumbnail visibility', () => {
	it('runs once after the card enters view and disconnects the observer', () => {
		let onIntersect: IntersectionObserverCallback | undefined;
		let disconnected = 0;
		const target = {} as Element;
		const load = vi.fn();
		const attachment = observeThumbnailVisibility(target, load, (callback) => {
			onIntersect = callback;
			return {
				observe: vi.fn(),
				disconnect: () => { disconnected += 1; },
				unobserve: vi.fn(),
			} as unknown as IntersectionObserver;
		});

		expect(load).not.toHaveBeenCalled();
		onIntersect?.([{ target, isIntersecting: false } as IntersectionObserverEntry], {} as IntersectionObserver);
		expect(load).not.toHaveBeenCalled();
		onIntersect?.([{ target, isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver);
		onIntersect?.([{ target, isIntersecting: true } as IntersectionObserverEntry], {} as IntersectionObserver);
		expect(load).toHaveBeenCalledTimes(1);
		expect(disconnected).toBe(1);
		attachment.destroy();
	});

	it('loads immediately when IntersectionObserver is unavailable', () => {
		const load = vi.fn();
		const attachment = observeThumbnailVisibility({} as Element, load, undefined);
		expect(load).toHaveBeenCalledTimes(1);
		attachment.destroy();
	});
});
