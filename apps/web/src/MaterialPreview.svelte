<script lang="ts">
 import { materialPreviewKind, transport, type FileEntry } from './transport';
 let { file, courseId }: { file: FileEntry; courseId: string } = $props();
 const kind = $derived(materialPreviewKind(file.name));
 const url = $derived(transport.mediaUrl(file, courseId));
</script>

{#if url && kind}
 <details class="material-preview">
  <summary>Visualizar {file.name}</summary>
  {#if kind === 'image'}
   <img src={url} alt={file.name} loading="lazy" />
  {:else}
   <iframe src={url} title={`Visualização de ${file.name}`} sandbox="allow-same-origin"></iframe>
  {/if}
 </details>
{/if}
