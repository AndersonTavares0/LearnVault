<script lang="ts">
	import CourseCover from './CourseCover.svelte';
	import { countCatalogNode, formatCourseCardMeta } from './catalog';
	import { href } from './router';
	import { courseIdOf, type CatalogNode, type FileEntry } from './transport';

	let {
		course,
		profile,
		mediaUrl,
		manualCoverUrl = null,
		canManage = false,
		menuOpen = false,
		onMenuOpen,
		onChangeCover,
		onRemoveCover
	}: {
		course: CatalogNode;
		profile: string;
		mediaUrl: (file: FileEntry, courseId: string) => string | null;
		/** Capa personalizada (upload da issue #237); tem precedência sobre a miniatura. */
		manualCoverUrl?: string | null;
		canManage?: boolean;
		menuOpen?: boolean;
		onMenuOpen?: (open: boolean) => void;
		onChangeCover?: (course: CatalogNode) => void;
		onRemoveCover?: (course: CatalogNode) => void;
	} = $props();
	const counts = $derived(countCatalogNode(course));
	const meta = $derived(formatCourseCardMeta(counts));
	let confirmRemove = $state(false);

	$effect(() => {
		if (!menuOpen) confirmRemove = false;
	});

	function openContextMenu(event: MouseEvent) {
		if (!canManage) return;
		event.preventDefault();
		onMenuOpen?.(true);
	}
</script>
<div class="lv-course-card-shell">
	<a class="lv-course-card" href={href('course', courseIdOf(course))} aria-label={course.title} oncontextmenu={openContextMenu}>
		<CourseCover {course} {profile} {mediaUrl} {manualCoverUrl} />
		<span class="lv-course-card__body">
			<span class="lv-course-card__title">{course.title}</span>
			{#if meta}<span class="lv-course-card__meta">{meta}</span>{/if}
		</span>
	</a>
	{#if canManage}
		<button
			class="lv-course-card__menu-toggle"
			type="button"
			aria-label="Ações de capa de {course.title}"
			aria-expanded={menuOpen}
			onclick={() => onMenuOpen?.(!menuOpen)}
		>
			<span aria-hidden="true">⋯</span>
		</button>
		{#if menuOpen}
			<div class="lv-course-card__menu" role="group" aria-label="Capa de {course.title}">
				{#if confirmRemove}
					<p class="lv-course-card__confirm" role="alert">Remover a capa de “{course.title}”?</p>
					<button type="button" onclick={() => { confirmRemove = false; onRemoveCover?.(course); }}>Remover capa</button>
					<button type="button" onclick={() => (confirmRemove = false)}>Cancelar</button>
				{:else}
					<button type="button" onclick={() => onChangeCover?.(course)}>
						{manualCoverUrl ? 'Trocar capa' : 'Adicionar capa'}
					</button>
					{#if manualCoverUrl}
						<button type="button" onclick={() => (confirmRemove = true)}>Remover capa</button>
					{/if}
				{/if}
			</div>
		{/if}
	{/if}
</div>
