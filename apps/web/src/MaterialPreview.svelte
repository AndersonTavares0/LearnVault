<script lang="ts">
 import { materialPreviewKind, transport, type FileEntry } from './transport';
 let { file, courseId }: { file: FileEntry; courseId: string } = $props();
 const kind = $derived(materialPreviewKind(file.name));
 const url = $derived(transport.mediaUrl(file, courseId));
</script>

{#if url && kind}
 <details class="material-preview">
  <summary><span class="preview-icon" aria-hidden="true">{kind === 'image' ? 'IMG' : kind === 'pdf' ? 'PDF' : 'TXT'}</span><span class="preview-label">Visualizar {file.name}</span></summary>
  {#if kind === 'image'}
   <img src={url} alt={file.name} loading="lazy" />
  {:else}
   <iframe src={url} title={`Visualização de ${file.name}`} sandbox="allow-same-origin"></iframe>
  {/if}
 </details>
{/if}

<style>
 .material-preview {
  margin-top: 8px;
  overflow: hidden;
  border: 1px solid var(--line);
  border-radius: 7px;
  background: var(--surface);
  color: var(--text);
  font: 12px/1.45 Inter, ui-sans-serif, system-ui, "Segoe UI", sans-serif;
 }

 summary {
  display: flex;
  min-height: 42px;
  align-items: center;
  gap: 10px;
  padding: 8px;
  cursor: pointer;
  list-style: none;
 }

 summary::-webkit-details-marker { display: none; }

 summary:hover { background: var(--hover); }

 summary:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: -2px;
 }

 .preview-icon {
  display: grid;
  width: 30px;
  height: 34px;
  flex: 0 0 30px;
  place-items: center;
  border: 1px solid var(--line);
  border-radius: 4px;
  background: var(--bg);
  color: var(--accent);
  font-size: 9px;
  font-weight: 800;
 }

 .preview-label {
  min-width: 0;
  overflow-wrap: anywhere;
  color: var(--text);
  font-weight: 650;
 }

 img, iframe {
  display: block;
  width: 100%;
  border: 0;
  border-top: 1px solid var(--line);
  background: var(--bg);
 }

 img {
  max-height: 420px;
  object-fit: contain;
 }

 iframe { height: 420px; }

 @media (max-width: 700px) {
  img { max-height: 300px; }
  iframe { height: 300px; }
 }
</style>
