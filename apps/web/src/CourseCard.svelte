<script lang="ts">
	import CourseCover from './CourseCover.svelte';
	import { countCatalogNode, formatCourseCardMeta } from './catalog';
	import { tr } from './i18n.svelte';
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
		onRemoveCover,
		inProgress = false,
		favorite = false,
		onToggleFavorite
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
		inProgress?: boolean;
		favorite?: boolean;
		onToggleFavorite?: (course: CatalogNode) => void;
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
			{#if course.sourceFolder}<span class="lv-course-source-badge" title={tr(`Source folder: ${course.sourceFolder}`, `Pasta de origem: ${course.sourceFolder}`)}>{course.sourceFolder}</span>{/if}
			{#if inProgress}<span class="lv-course-card__state"><span aria-hidden="true"></span>{tr('In progress', 'Em andamento')}</span>{/if}
		</span>
	</a>
	{#if onToggleFavorite}
		<button
			class="lv-course-card__favorite"
			class:is-favorite={favorite}
			type="button"
			aria-pressed={favorite}
			aria-label={favorite ? tr(`Remove "${course.title}" from favorites`, `Remover "${course.title}" dos favoritos`) : tr(`Favorite course "${course.title}"`, `Favoritar curso "${course.title}"`)}
			title={favorite ? tr('Remove from favorites', 'Remover dos favoritos') : tr('Favorite course', 'Favoritar curso')}
			onclick={() => onToggleFavorite(course)}
		>
			<svg viewBox="0 0 24 24" width="16" height="16" fill={favorite ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
				<polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" />
			</svg>
		</button>
	{/if}
	{#if canManage}
		<button
			class="lv-course-card__menu-toggle"
			type="button"
			aria-label={tr(`Cover actions for ${course.title}`, `Ações de capa de ${course.title}`)}
			aria-expanded={menuOpen}
			onclick={() => onMenuOpen?.(!menuOpen)}
		>
			<span aria-hidden="true">⋯</span>
		</button>
		{#if menuOpen}
			<div class="lv-course-card__menu" role="group" aria-label={tr(`Cover of ${course.title}`, `Capa de ${course.title}`)}>
				{#if confirmRemove}
					<p class="lv-course-card__confirm" role="alert">{tr(`Remove the cover of “${course.title}”?`, `Remover a capa de “${course.title}”?`)}</p>
					<button type="button" onclick={() => { confirmRemove = false; onRemoveCover?.(course); }}>{tr('Remove cover', 'Remover capa')}</button>
					<button type="button" onclick={() => (confirmRemove = false)}>{tr('Cancel', 'Cancelar')}</button>
				{:else}
					<button type="button" onclick={() => onChangeCover?.(course)}>
						{manualCoverUrl ? tr('Change cover', 'Trocar capa') : tr('Add cover', 'Adicionar capa')}
					</button>
					{#if manualCoverUrl}
						<button type="button" onclick={() => (confirmRemove = true)}>{tr('Remove cover', 'Remover capa')}</button>
					{/if}
				{/if}
			</div>
		{/if}
	{/if}
</div>
