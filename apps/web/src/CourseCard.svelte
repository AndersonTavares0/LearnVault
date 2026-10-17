<script lang="ts">
	import { countCatalogNode, formatCourseCardMeta } from './catalog';
	import { courseCover } from './cover';
	import { href } from './router';
	import { courseIdOf, type CatalogNode } from './transport';

	let { course }: { course: CatalogNode } = $props();
	const counts = $derived(countCatalogNode(course));
	const meta = $derived(formatCourseCardMeta(counts));
	const cover = $derived(courseCover(course.title));
</script>
<a class="course-card" href={href('course', courseIdOf(course))}>
	<span class="course-cover" style:--cover-from={cover.from} style:--cover-to={cover.to} aria-hidden="true">
		<span class="course-monogram">{cover.monogram}</span>
	</span>
	<span class="course-card-title">{course.title}</span>
	{#if meta}<span class="course-card-meta">{meta}</span>{/if}
</a>
