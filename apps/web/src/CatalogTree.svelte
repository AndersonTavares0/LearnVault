<script lang="ts">
	import CatalogTree from './CatalogTree.svelte';
	import LooseFiles from './LooseFiles.svelte';
	import { countCatalogFiles, countCatalogNode, formatCount, formatCourseMeta, formatLessonMeta } from './catalog';
	import { href } from './router';
	import type { CatalogNode } from './transport';

	// `expandAll` abre os módulos enquanto a busca está ativa, para que um resultado
	// dentro de um módulo não fique escondido atrás do disclosure recolhido.
	let { nodes, expandAll = false }: { nodes: CatalogNode[]; expandAll?: boolean } = $props();
</script>
{#each nodes as node (node.path)}
	{#if node.role === 'module'}
		<details class="course-section" open={expandAll}>
			<summary>
				<span class="section-title">{node.title}</span>
				<span class="section-meta">{formatCount(countCatalogNode(node).lessons, 'aula', 'aulas')}</span>
				<svg class="section-chevron" viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
			</summary>
			<ul class="lesson-list">
				{#each node.children as child (child.path)}<li><CatalogTree nodes={[child]} {expandAll} /></li>{/each}
			</ul>
		</details>
	{:else if node.role === 'lesson'}
		{@const counts = countCatalogFiles(node)}
		{@const meta = formatLessonMeta(counts)}
		<a class="lesson-row" href={href('player', node.path)}>
			<svg class="lesson-glyph" viewBox="0 0 16 16" aria-hidden="true">
				{#if counts.videos > 0}<path d="M5.5 3.5l6.5 4.5-6.5 4.5z" />{:else}<path d="M4.5 2.5h7v11h-7z" fill="none" stroke="currentColor" stroke-width="1.4" />{/if}
			</svg>
			<span class="lesson-title">{node.title}</span>
			{#if meta}<span class="lesson-meta">{meta}</span>{/if}
		</a>
		{#if node.children.length}<div class="lesson-children">{#each node.children as child (child.path)}<CatalogTree nodes={[child]} {expandAll} />{/each}</div>{/if}
	{:else}
		{@const meta = formatCourseMeta(countCatalogNode(node))}
		<section class="course-block">
			<header class="course-header">
				<h2>{node.title}</h2>
				{#if meta}<p class="course-meta">{meta}</p>{/if}
			</header>
			{#if node.files.length || node.looseFiles.length}<LooseFiles files={[...node.files, ...node.looseFiles]} heading="Arquivos soltos" />{/if}
			{#if node.children.length}<div class="course-body">{#each node.children as child (child.path)}<CatalogTree nodes={[child]} {expandAll} />{/each}</div>{/if}
		</section>
	{/if}
{/each}
