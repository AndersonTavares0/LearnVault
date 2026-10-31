<script lang="ts">
	import CatalogTree from './CatalogTree.svelte';
	import LooseFiles from './LooseFiles.svelte';
	import { countCatalogFiles, countCatalogNode, formatCourseMeta, formatLessonCounts, formatLessonMeta, isLessonMissing } from './catalog';
	import { href } from './router';
	import type { CatalogNode } from './transport';

	// `expandAll` abre os módulos enquanto a busca está ativa, para que um resultado
	// dentro de um módulo não fique escondido atrás do disclosure recolhido.
	// `courseId` viaja com a subárvore para que links de aula e de mídia carreguem a
	// identidade do curso, que desempata caminhos relativos iguais entre cursos.
	//
	// `activePath` é a aula aberta no player. Ela recebe `aria-current`, destaque visual e
	// mantém o módulo que a contém aberto, para que o índice lateral acompanhe o estudo.
	let {
		nodes,
		courseId,
		expandAll = false,
		showFiles = true,
		activePath = null
	}: {
		nodes: CatalogNode[];
		courseId: string;
		expandAll?: boolean;
		showFiles?: boolean;
		activePath?: string | null;
	} = $props();

	// O nó contém a aula aberta quando é a própria aula ou um ancestral dela. Caminhos são
	// relativos e completos, então o separador evita confundir "mod" com "mod-2".
	function containsActive(node: CatalogNode): boolean {
		return activePath !== null && (node.path === activePath || activePath.startsWith(`${node.path}/`));
	}

	// Mantém a aula aberta visível na caixa rolável da lista sem arrastar a página.
	function revealActive(node: HTMLElement, active: boolean) {
		function reveal(): void {
			if (!active) return;
			const box = node.closest('[data-playlist-body]');
			if (!(box instanceof HTMLElement)) return;
			const boxRect = box.getBoundingClientRect();
			const nodeRect = node.getBoundingClientRect();
			if (nodeRect.width === 0 && nodeRect.height === 0) return;
			if (nodeRect.top < boxRect.top) box.scrollTop -= boxRect.top - nodeRect.top + 8;
			else if (nodeRect.bottom > boxRect.bottom) box.scrollTop += nodeRect.bottom - boxRect.bottom + 8;
		}
		reveal();
		return {
			update(next: boolean): void {
				active = next;
				reveal();
			}
		};
	}
</script>
{#each nodes as node (node.path)}
	{#if node.role === 'module'}
		<details class="course-section" open={expandAll || containsActive(node)}>
			<summary>
				<span class="section-title">{node.title}</span>
				<span class="section-meta">{formatLessonCounts(countCatalogNode(node))}</span>
				<svg class="section-chevron" viewBox="0 0 16 16" aria-hidden="true"><path d="M4 6l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
			</summary>
			<ul class="lesson-list">
				{#each node.children as child (child.path)}<li><CatalogTree nodes={[child]} {courseId} {expandAll} {showFiles} {activePath} /></li>{/each}
			</ul>
			{#if showFiles && (node.files.length || node.looseFiles.length)}<LooseFiles files={[...node.files, ...node.looseFiles]} heading="Materiais complementares" {courseId} />{/if}
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
				<span class="lesson-meta lesson-meta--missing">Mídia ausente</span>
				{#if meta}<span class="lesson-meta">{meta}</span>{/if}
			</div>
		{:else}
			<a class="lesson-row" class:active={activePath === node.path} href={href('course', courseId, node.path)} aria-current={activePath === node.path ? 'page' : undefined} use:revealActive={activePath === node.path}>
				<svg class="lesson-glyph" viewBox="0 0 16 16" aria-hidden="true">
					{#if counts.videos > 0}<path d="M5.5 3.5l6.5 4.5-6.5 4.5z" />{:else}<path d="M4.5 2.5h7v11h-7z" fill="none" stroke="currentColor" stroke-width="1.4" />{/if}
				</svg>
				<span class="lesson-title">{node.title}</span>
				{#if meta}<span class="lesson-meta">{meta}</span>{/if}
			</a>
		{/if}
		{#if node.children.length}<div class="lesson-children">{#each node.children as child (child.path)}<CatalogTree nodes={[child]} {courseId} {expandAll} {showFiles} {activePath} />{/each}</div>{/if}
	{:else}
		{@const meta = formatCourseMeta(countCatalogNode(node))}
		<section class="course-block">
			<header class="course-header">
				<h2>{node.title}</h2>
				{#if meta}<p class="course-meta">{meta}</p>{/if}
			</header>
			{#if showFiles && (node.files.length || node.looseFiles.length)}<LooseFiles files={[...node.files, ...node.looseFiles]} heading="Arquivos soltos" {courseId} />{/if}
			{#if node.children.length}<div class="course-body">{#each node.children as child (child.path)}<CatalogTree nodes={[child]} {courseId} {expandAll} {showFiles} {activePath} />{/each}</div>{/if}
		</section>
	{/if}
{/each}
