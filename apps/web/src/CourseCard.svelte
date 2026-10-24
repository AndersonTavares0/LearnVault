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
		profile = '',
		mediaUrl = () => null,
		coverSrc = '',
		manualCoverUrl = null,
		canManage = false,
		menuOpen = false,
		onMenuOpen,
		onChangeCover,
		onRemoveCover,
		displayTitle = course.title,
		layout = 'grid',
		progressLabel = '',
		progressPercent = null
	}: {
		course: CatalogNode;
		profile?: string;
		mediaUrl?: (file: FileEntry, courseId: string) => string | null;
		coverSrc?: string;
		/** Capa personalizada (upload da issue #237); tem precedência sobre a miniatura. */
		manualCoverUrl?: string | null;
		canManage?: boolean;
		menuOpen?: boolean;
		onMenuOpen?: (open: boolean) => void;
		onChangeCover?: (course: CatalogNode) => void;
		onRemoveCover?: (course: CatalogNode) => void;
		displayTitle?: string;
		layout?: 'grid' | 'list';
		progressLabel?: string;
		progressPercent?: number | null;
	} = $props();
	const counts = $derived(countCatalogNode(course));
	const meta = $derived(formatCourseCardMeta(counts));
	const cover = $derived(courseCover(displayTitle));
	const courseLink = $derived(href('course', courseIdOf(course)));
	let thumbnailUrl = $state<string | null>(null);
	let objectUrl: string | null = null;
	/** Capa efetiva: manual > miniatura automática > gerada pelo título. */
	const coverSource = $derived(coverSrc ? 'external' : selectCourseCoverSource({ manualCoverUrl, thumbnailUrl }));

	function openContextMenu(event: MouseEvent) {
		if (!canManage) return;
		event.preventDefault();
		onMenuOpen?.(true);
	}

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
<div class="course-card-shell" class:course-card-list={layout === 'list'}>
	<a class="course-card" class:course-card-list={layout === 'list'} href={courseLink} aria-label={`Abrir curso ${displayTitle}`} oncontextmenu={openContextMenu}>
		<span class="course-cover" class:has-image={coverSource !== 'generated'} use:cleanupThumbnail={{ course, profile, manualCover: manualCoverUrl }} style:--cover-from={cover.from} style:--cover-to={cover.to} style:--cover-angle={`${cover.angle}deg`} aria-hidden="true">
			{#if coverSource === 'manual'}<img class="course-cover-image" src={manualCoverUrl ?? ''} alt="" />
			{:else if coverSource === 'external'}<img class="course-cover-image" src={coverSrc} alt="" />
			{:else if coverSource === 'thumbnail'}<img class="course-thumbnail" src={thumbnailUrl ?? ''} alt="" />{/if}
			<span class="course-monogram">{cover.monogram}</span>
		</span>
		<span class="course-card-content">
			<span class="course-card-title">{displayTitle}</span>
			{#if meta}<span class="course-card-meta">{meta}</span>{/if}
			{#if progressPercent !== null}<span class="course-progress" aria-label={`${progressPercent}% das aulas concluídas`}><span><i style:width={`${progressPercent}%`}></i></span><small>{progressPercent}% concluído</small></span>
			{:else if progressLabel}<span class="course-progress-label">{progressLabel}</span>{/if}
		</span>
	</a>
	{#if canManage}
		<button
			class="course-card-menu-toggle"
			type="button"
			aria-label="Ações de capa de {course.title}"
			aria-haspopup="menu"
			aria-expanded={menuOpen}
			onclick={() => onMenuOpen?.(!menuOpen)}
		>
			<span aria-hidden="true">⋯</span>
		</button>
		{#if menuOpen}
			<div class="course-card-menu" role="menu" aria-label="Capa de {course.title}">
				<button type="button" role="menuitem" onclick={() => onChangeCover?.(course)}>
					{manualCoverUrl ? 'Trocar capa' : 'Adicionar capa'}
				</button>
				{#if manualCoverUrl}
					<button type="button" role="menuitem" onclick={() => onRemoveCover?.(course)}>Remover capa</button>
				{/if}
			</div>
		{/if}
	{/if}
</div>
