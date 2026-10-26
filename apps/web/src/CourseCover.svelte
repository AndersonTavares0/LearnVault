<script module lang="ts">
	import {
		IndexedDbThumbnailCacheAdapter,
		ThumbnailCache,
		ThumbnailTaskQueue,
		loadCourseThumbnail,
		observeThumbnailVisibility
	} from './thumbnails';
	import type { FileEntry } from './transport';

	const thumbnailCache = new ThumbnailCache(new IndexedDbThumbnailCacheAdapter());
	const thumbnailQueue = new ThumbnailTaskQueue();
</script>

<script lang="ts">
	import { courseCover, selectCourseCoverSource } from './cover';
	import type { CatalogNode } from './transport';

	let {
		course,
		profile,
		mediaUrl,
		manualCoverUrl = null,
		size = 'card'
	}: {
		course: CatalogNode;
		profile: string;
		mediaUrl: (file: FileEntry, courseId: string) => string | null;
		manualCoverUrl?: string | null;
		size?: 'card' | 'hero';
	} = $props();

	let thumbnailUrl = $state<string | null>(null);
	const cover = $derived(courseCover(course.title));
	const coverSource = $derived(selectCourseCoverSource({ manualCoverUrl, thumbnailUrl }));
	let objectUrl: string | null = null;

	function revokeThumbnail() {
		if (objectUrl) {
			try {
				URL.revokeObjectURL(objectUrl);
			} catch {
				// A stale object URL must not break card teardown.
			}
		}
		objectUrl = null;
		thumbnailUrl = null;
	}

	function cleanupThumbnail(
		node: HTMLElement,
		initial: { course: CatalogNode; profile: string; manualCover: string | null }
	) {
		let current = initial;
		let active = true;
		let generation = 0;
		const observe = () => observeThumbnailVisibility(node, async () => {
			const request = ++generation;
			const blob = await loadCourseThumbnail({
				course: current.course,
				profile: current.profile,
				cache: thumbnailCache,
				queue: thumbnailQueue,
				mediaUrl,
				manualCoverUrl: current.manualCover
			});
			if (blob && active && request === generation) {
				revokeThumbnail();
				objectUrl = URL.createObjectURL(blob);
				thumbnailUrl = objectUrl;
			}
		});
		let visibility = observe();
		return {
			update(next: { course: CatalogNode; profile: string; manualCover: string | null }) {
				if (
					next.course === current.course &&
					next.profile === current.profile &&
					next.manualCover === current.manualCover
				)
					return;
				current = next;
				generation++;
				visibility.destroy();
				revokeThumbnail();
				visibility = observe();
			},
			destroy() {
				active = false;
				generation++;
				visibility.destroy();
				revokeThumbnail();
			}
		};
	}
</script>

<span
	class="lv-cover lv-cover--{size}"
	class:lv-cover--image={coverSource !== 'generated'}
	use:cleanupThumbnail={{ course, profile, manualCover: manualCoverUrl }}
	style:--cover-from={cover.from}
	style:--cover-to={cover.to}
	style:--cover-angle={`${cover.angle}deg`}
	aria-hidden="true"
>
	{#if coverSource === 'manual'}<img class="lv-cover__image" src={manualCoverUrl ?? ''} alt="" width="600" height="400" loading={size === 'hero' ? 'eager' : 'lazy'} decoding="async" />
	{:else if coverSource === 'thumbnail'}<img class="lv-cover__image" src={thumbnailUrl ?? ''} alt="" width="600" height="400" loading={size === 'hero' ? 'eager' : 'lazy'} decoding="async" />{/if}
	<span class="lv-cover__pattern lv-cover__pattern--{cover.pattern}"></span>
	<span class="lv-cover__spine"><span>{cover.code}</span></span>
	{#if coverSource === 'generated'}<span class="lv-cover__monogram">{cover.monogram}</span>{/if}
</span>
