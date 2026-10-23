<script lang="ts">
	import { countCatalogNode, formatCourseCardMeta } from './catalog';
	import { courseCover } from './cover';
	import { href } from './router';
	import { courseIdOf, type CatalogNode } from './transport';

	let { course, displayTitle = course.title, layout = 'grid', progressLabel = '', coverSrc = '', progressPercent = null }: {
		course: CatalogNode;
		displayTitle?: string;
		layout?: 'grid' | 'list';
		progressLabel?: string;
		coverSrc?: string;
		progressPercent?: number | null;
	} = $props();
	const counts = $derived(countCatalogNode(course));
	const meta = $derived(formatCourseCardMeta(counts));
	const cover = $derived(courseCover(displayTitle));
	const courseLink = $derived(href('course', courseIdOf(course)));
</script>
<a class="course-card" class:course-card-list={layout === 'list'} href={courseLink} aria-label={`Abrir curso ${displayTitle}`}>
	<span class="course-cover" class:has-image={Boolean(coverSrc)} style:--cover-from={cover.from} style:--cover-to={cover.to} aria-hidden="true">
		{#if coverSrc}<img src={coverSrc} alt="" loading="lazy" onerror={(event) => event.currentTarget.parentElement?.classList.remove('has-image')} />{/if}
		<span class="course-monogram">{cover.monogram}</span>
	</span>
	<span class="course-card-content">
		<span class="course-card-title">{displayTitle}</span>
		{#if meta}<span class="course-card-meta">{meta}</span>{/if}
		{#if progressPercent !== null}<span class="course-progress" aria-label={`${progressPercent}% das aulas concluídas`}><span><i style:width={`${progressPercent}%`}></i></span><small>{progressPercent}% concluído</small></span>
		{:else if progressLabel}<span class="course-progress-label">{progressLabel}</span>{/if}
	</span>
</a>
