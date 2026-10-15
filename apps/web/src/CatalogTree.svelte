<script lang="ts">
 import LooseFiles from './LooseFiles.svelte';
 import type { CatalogNode, Lesson } from './transport';
 export let nodes: CatalogNode[] = [];
 export let openLesson: (lesson: Lesson) => void;
 function files(node: CatalogNode) { return [...node.files, ...node.looseFiles]; }
 function asLesson(node: CatalogNode): Lesson {
  const all = files(node);
  return { title: node.title, path: node.path, media: all.filter((f) => f.kind === 'media'), subtitles: all.filter((f) => f.kind === 'subtitle'), materials: all.filter((f) => f.kind === 'material'), availability: all.find((f) => f.availability)?.availability, unverified: all.some((f) => f.unverified) };
 }
</script>
{#each nodes as node}
 <section class:category={node.role === 'category'} class="catalog-node">
  <h2>{node.title}</h2>
  {#if node.files.length || node.looseFiles.length}<LooseFiles files={files(node)} heading="Arquivos soltos" />{/if}
  {#if node.role === 'lesson'}
   <button class="lesson" onclick={() => openLesson(asLesson(node))}><span>{node.title}</span><small>{asLesson(node).media.length} vídeo · {asLesson(node).materials.length + asLesson(node).subtitles.length} materiais</small></button>
  {/if}
  {#if node.children.length}<svelte:self nodes={node.children} {openLesson} />{/if}
 </section>
{/each}
