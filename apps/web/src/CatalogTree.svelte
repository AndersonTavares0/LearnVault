<script lang="ts">
 import LooseFiles from './LooseFiles.svelte';
 import { lessonFromNode, type CatalogNode, type Lesson } from './transport';
 export let nodes: CatalogNode[] = [];
 export let openLesson: (lesson: Lesson) => void;
 function files(node: CatalogNode) { return [...node.files, ...node.looseFiles]; }
</script>
{#each nodes as node}
 <section class:category={node.role === 'category'} class="catalog-node">
  <h2>{node.title}</h2>
  {#if node.files.length || node.looseFiles.length}<LooseFiles files={files(node)} heading="Arquivos soltos" />{/if}
  {#if node.role === 'lesson'}
   {@const lesson = lessonFromNode(node)}
   <button class="lesson" onclick={() => openLesson(lesson)}><span>{node.title}</span><small>{lesson.media.length} vídeo · {lesson.materials.length + lesson.subtitles.length} materiais</small></button>
  {/if}
  {#if node.children.length}<svelte:self nodes={node.children} {openLesson} />{/if}
 </section>
{/each}
