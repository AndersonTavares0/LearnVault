<script lang="ts">
	import { countCatalogNode, formatCourseCardMeta } from './catalog';
	import { courseCover } from './cover';
	import { href } from './router';
	import { courseIdOf, type CatalogNode } from './transport';

	let { course, variant = '' }: { course: CatalogNode; variant?: '' | 'alt' | 'violet' } = $props();
	const counts = $derived(countCatalogNode(course));
	const meta = $derived(formatCourseCardMeta(counts));
	const cover = $derived(courseCover(course.title));
</script>
<a class="course" href={href('course', courseIdOf(course))}>
	<div class="cover {variant}" aria-hidden="true">{cover.monogram}</div>
	<div class="course-body">
		<h3>{course.title}</h3>
		{#if meta}<p>{meta}</p>{/if}
	</div>
</a>
