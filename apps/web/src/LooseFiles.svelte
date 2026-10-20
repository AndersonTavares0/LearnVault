<script lang="ts">
 import { fileStatus, transport, type FileEntry } from './transport';
 import { href } from './router';
 import MaterialPreview from './MaterialPreview.svelte';
 let { files, heading, courseId }: { files: FileEntry[]; heading: string; courseId?: string } = $props();
</script>
<section class="loose-files" aria-label={heading}>
	<strong>{heading}</strong>
	<ul>
		{#each files as file}
			{@const missing = file.availability?.trim().toLowerCase() === 'missing'}
			{@const url = missing ? null : transport.mediaUrl(file, courseId)}
			<li>
				{#if url && file.kind === 'media'}
					{#if courseId}
						<a href={href('player', courseId, file.path)}>Reproduzir {file.name}</a>
					{:else}
						<a href={href('player', file.path)}>Reproduzir {file.name}</a>
					{/if}
 {:else if url}
  <a href={url} download={file.name} target="_blank" rel="noopener noreferrer">Baixar {file.name}</a>
  {#if courseId}<MaterialPreview {file} {courseId} />{/if}
				{:else}
					<span>{file.name}</span>
				{/if}
				{#if fileStatus(file)} <small>({fileStatus(file)})</small>{/if}
			</li>
		{/each}
	</ul>
</section>
