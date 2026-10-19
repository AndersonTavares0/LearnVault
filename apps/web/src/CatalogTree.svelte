<script lang="ts">
	import CatalogTree from './CatalogTree.svelte';
	import LooseFiles from './LooseFiles.svelte';
	import { countCatalogFiles, countCatalogNode, formatCourseMeta, formatLessonCounts, formatLessonMeta, isLessonMissing } from './catalog';
	import { MOVE_DOWN, MOVE_UP, canMove, moveSibling } from './order';
	import { href } from './router';
	import type { CatalogNode } from './transport';

	// `expandAll` abre os módulos enquanto a busca está ativa, para que um resultado
	// dentro de um módulo não fique escondido atrás do disclosure recolhido.
	// `courseId` viaja com a subárvore para que links de aula e de mídia carreguem a
	// identidade do curso, que desempata caminhos relativos iguais entre cursos.
	//
	// `manage`, `parentPath`, `siblings` e `onReorder` ligam os controles de ordenação
	// manual: `siblings` é a ordem completa dos irmãos (para que mover um nó saiba a nova
	// lista a persistir) e `parentPath` é o caminho do pai (vazio para a lista de topo do
	// curso). Sem `onReorder` — ou para um leitor — nenhum controle aparece.
	let {
		nodes,
		courseId,
		expandAll = false,
		manage = false,
		parentPath = '',
		siblings = nodes.map((node) => node.path),
		onReorder
	}: {
		nodes: CatalogNode[];
		courseId: string;
		expandAll?: boolean;
		manage?: boolean;
		parentPath?: string;
		siblings?: string[];
		onReorder?: (parentPath: string, order: string[]) => void;
	} = $props();

	// Um clique em um controle dentro de `<summary>` não pode alternar o módulo.
	function move(event: MouseEvent, path: string, delta: number): void {
		event.preventDefault();
		event.stopPropagation();
		onReorder?.(parentPath, moveSibling(siblings, path, delta));
	}
</script>
{#snippet orderControls(node: CatalogNode)}
	{#if manage && siblings.length > 1}
		<span class="order-controls">
			<button class="text-button order-button" type="button" disabled={!canMove(siblings, node.path, MOVE_UP)} aria-label={`Mover "${node.title}" para cima`} onclick={(event) => move(event, node.path, MOVE_UP)}>↑</button>
			<button class="text-button order-button" type="button" disabled={!canMove(siblings, node.path, MOVE_DOWN)} aria-label={`Mover "${node.title}" para baixo`} onclick={(event) => move(event, node.path, MOVE_DOWN)}>↓</button>
		</span>
	{/if}
{/snippet}
{#each nodes as node (node.path)}
	{#if node.role === 'module'}
		<details class="course-section" open={expandAll}>
			<summary>
				<span class="section-title">{node.title}</span>
				<span class="section-meta">{formatLessonCounts(countCatalogNode(node))}</span>
				{@render orderControls(node)}
				<svg class="section-chevron" viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
			</summary>
			<ul class="lesson-list">
				{#each node.children as child (child.path)}<li><CatalogTree nodes={[child]} {courseId} {expandAll} {manage} parentPath={node.path} siblings={node.children.map((entry) => entry.path)} {onReorder} /></li>{/each}
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
			<div class="lesson-row">
				<svg class="lesson-glyph" viewBox="0 0 16 16" aria-hidden="true">
					{#if counts.videos > 0}<path d="M5.5 3.5l6.5 4.5-6.5 4.5z" />{:else}<path d="M4.5 2.5h7v11h-7z" fill="none" stroke="currentColor" stroke-width="1.4" />{/if}
				</svg>
				<span class="lesson-title">{node.title}</span>
				<small class="lesson-meta">(ausente)</small>
				{#if meta}<span class="lesson-meta">{meta}</span>{/if}
			</div>
		{:else}
			<a class="lesson-row" href={href('player', courseId, node.path)}>
				<svg class="lesson-glyph" viewBox="0 0 16 16" aria-hidden="true">
					{#if counts.videos > 0}<path d="M5.5 3.5l6.5 4.5-6.5 4.5z" />{:else}<path d="M4.5 2.5h7v11h-7z" fill="none" stroke="currentColor" stroke-width="1.4" />{/if}
				</svg>
				<span class="lesson-title">{node.title}</span>
				{#if meta}<span class="lesson-meta">{meta}</span>{/if}
			</a>
		{/if}
		{@render orderControls(node)}
		{#if node.children.length}<div class="lesson-children">{#each node.children as child (child.path)}<CatalogTree nodes={[child]} {courseId} {expandAll} {manage} parentPath={node.path} siblings={node.children.map((entry) => entry.path)} {onReorder} />{/each}</div>{/if}
	{:else}
		{@const meta = formatCourseMeta(countCatalogNode(node))}
		<section class="course-block">
			<header class="course-header">
				<h2>{node.title}</h2>
				{#if meta}<p class="course-meta">{meta}</p>{/if}
				{@render orderControls(node)}
			</header>
			{#if node.files.length || node.looseFiles.length}<LooseFiles files={[...node.files, ...node.looseFiles]} heading="Arquivos soltos" {courseId} />{/if}
			{#if node.children.length}<div class="course-body">{#each node.children as child (child.path)}<CatalogTree nodes={[child]} {courseId} {expandAll} {manage} parentPath={node.path} siblings={node.children.map((entry) => entry.path)} {onReorder} />{/each}</div>{/if}
		</section>
	{/if}
{/each}
