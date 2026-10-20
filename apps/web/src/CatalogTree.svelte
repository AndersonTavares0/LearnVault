<script lang="ts">
	import CatalogTree from './CatalogTree.svelte';
	import LooseFiles from './LooseFiles.svelte';
	import { countCatalogFiles, countCatalogNode, formatCourseMeta, formatLessonCounts, formatLessonMeta, isLessonMissing } from './catalog';
	import CatalogOrderControls from './CatalogOrderControls.svelte';
	import { canReorderCatalogSiblings, movedCatalogPaths } from './catalog-order';
 import { href } from './router';
 import { isCoverFile, transport, type CatalogNode } from './transport';

	// `expandAll` abre os módulos enquanto a busca está ativa, para que um resultado
	// dentro de um módulo não fique escondido atrás do disclosure recolhido.
	// `courseId` viaja com a subárvore para que links de aula e de mídia carreguem a
	// identidade do curso, que desempata caminhos relativos iguais entre cursos.
	let {
		nodes,
		courseId,
		expandAll = false,
		organizing = false,
		parentPath = '',
		currentPath = '',
		siblings: suppliedSiblings,
		siblingIndex: suppliedIndex,
		orderSaving = false,
		onSaveOrder,
		onResetOrder
	}: {
		nodes: CatalogNode[];
		courseId: string;
		expandAll?: boolean;
		organizing?: boolean;
		parentPath?: string;
		currentPath?: string;
		siblings?: CatalogNode[];
		siblingIndex?: number;
		orderSaving?: boolean;
		onSaveOrder?: (parentPath: string, childPaths: string[]) => void;
		onResetOrder?: (parentPath: string) => void;
	} = $props();
</script>
{#each nodes as node, index (node.path)}
	{@const siblingNodes = suppliedSiblings ?? nodes}
	{@const itemIndex = suppliedSiblings ? suppliedIndex ?? 0 : index}
	{#if node.role === 'module'}
		<details class="course-section" open={expandAll}>
			<summary class="module">
				<span class="section-chevron" aria-hidden="true">⌄</span>
				<span class="section-title">{node.title}</span>
				<span class="small">{formatLessonCounts(countCatalogNode(node))}</span>
			</summary>
			<ul class="lesson-list">
				{#each node.children as child, childIndex (child.path)}<li><CatalogTree nodes={[child]} {courseId} {expandAll} {organizing} {currentPath} parentPath={node.path} siblings={node.children} siblingIndex={childIndex} {orderSaving} {onSaveOrder} {onResetOrder} /></li>{/each}
			</ul>
			{#if node.files.length || node.looseFiles.length}<LooseFiles files={[...node.files, ...node.looseFiles]} heading="Materiais complementares" {courseId} />{/if}
		</details>
	{:else if node.role === 'lesson'}
		{@const counts = countCatalogFiles(node)}
		{@const meta = formatLessonMeta(counts)}
		{@const missing = isLessonMissing(node)}
		{#if missing}
			<!-- Aula sem mídia disponível: continua listada e legível, mas não abre o player,
			     então a linha é conteúdo estático — sem link e fora da ordem de foco. -->
			<div class="lesson missing">
				<span class="circle" aria-hidden="true"></span>
				<span>{node.title}</span>
				<small class="small">(ausente){#if meta} · {meta}{/if}</small>
			</div>
		{:else}
			<a class="lesson" class:current={node.path === currentPath} href={href('player', courseId, node.path)}>
				<span class="circle" aria-hidden="true"></span>
				<span>{node.title}</span>
				{#if meta}<span class="small">{meta}</span>{/if}
			</a>
		{/if}
		{#if node.children.length}<div class="lesson-children">{#each node.children as child, childIndex (child.path)}<CatalogTree nodes={[child]} {courseId} {expandAll} {organizing} {currentPath} parentPath={node.path} siblings={node.children} siblingIndex={childIndex} {orderSaving} {onSaveOrder} {onResetOrder} />{/each}</div>{/if}
	{:else}
		{@const cover = node.looseFiles.find(isCoverFile)}
		{@const meta = formatCourseMeta(countCatalogNode(node))}
		<section class="course-block">
		<header class="course-header">
			<h2>{node.title}</h2>
			{#if cover && transport.mediaUrl(cover, courseId)}<img class="course-cover" src={transport.mediaUrl(cover, courseId) ?? ''} alt="" loading="lazy" />{/if}
				{#if meta}<p class="course-meta">{meta}</p>{/if}
			</header>
			{#if node.files.length || node.looseFiles.length}<LooseFiles files={[...node.files, ...node.looseFiles]} heading="Arquivos soltos" {courseId} />{/if}
			{#if node.children.length}<div class="course-body">{#each node.children as child, childIndex (child.path)}<CatalogTree nodes={[child]} {courseId} {expandAll} {organizing} {currentPath} parentPath={node.path} siblings={node.children} siblingIndex={childIndex} {orderSaving} {onSaveOrder} {onResetOrder} />{/each}</div>{/if}
		</section>
	{/if}
	{#if organizing && canReorderCatalogSiblings(siblingNodes)}
		<CatalogOrderControls itemTitle={node.title} index={itemIndex} count={siblingNodes.length} busy={orderSaving} onMove={(offset) => { const paths = movedCatalogPaths(siblingNodes, itemIndex, offset); if (paths) onSaveOrder?.(parentPath, paths); }} onReset={() => onResetOrder?.(parentPath)} />
	{/if}
{/each}
