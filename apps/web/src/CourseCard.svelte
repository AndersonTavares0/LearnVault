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
	import { countCatalogNode, formatCourseCardMeta } from './catalog';
	import { courseCover, selectCourseCoverSource } from './cover';
	import { href } from './router';
	import { courseIdOf, type CatalogNode } from './transport';

	let {
		course,
		profile,
		mediaUrl,
		manualCoverUrl = null
	}: {
		course: CatalogNode;
		profile: string;
		mediaUrl: (file: FileEntry, courseId: string) => string | null;
		/** Capa personalizada (upload da issue #237); tem precedência sobre a miniatura. */
		manualCoverUrl?: string | null;
	} = $props();
	const counts = $derived(countCatalogNode(course));
	const meta = $derived(formatCourseCardMeta(counts));
	const cover = $derived(courseCover(course.title));
	let thumbnailUrl = $state<string | null>(null);
	let objectUrl: string | null = null;
	/** Capa efetiva: manual > miniatura automática > gerada pelo título. */
	const coverSource = $derived(selectCourseCoverSource({ manualCoverUrl, thumbnailUrl }));

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
<a class="course-card" href={href('course', courseIdOf(course))}>
	<span class="course-cover" use:cleanupThumbnail={{ course, profile, manualCover: manualCoverUrl }} style:--cover-from={cover.from} style:--cover-to={cover.to} style:--cover-angle={`${cover.angle}deg`} aria-hidden="true">
		{#if coverSource === 'manual'}<img class="course-cover-image" src={manualCoverUrl ?? ''} alt="" />
		{:else if coverSource === 'thumbnail'}<img class="course-thumbnail" src={thumbnailUrl ?? ''} alt="" />{/if}
		<span class="course-monogram">{cover.monogram}</span>
	</span>
	<span class="course-card-title">{course.title}</span>
	{#if meta}<span class="course-card-meta">{meta}</span>{/if}
</a>
