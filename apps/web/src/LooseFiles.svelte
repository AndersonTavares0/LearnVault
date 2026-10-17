<script lang="ts">
 import { fileStatus, transport, type FileEntry } from './transport';
 import { href } from './router';
 let { files, heading }: { files: FileEntry[]; heading: string } = $props();
</script>
<section class="loose-files" aria-label={heading}>
	<strong>{heading}</strong>
	<ul>
		{#each files as file}
			{@const missing = file.availability?.trim().toLowerCase() === 'missing'}
			{@const url = missing ? null : transport.mediaUrl(file)}
			<li>
				{#if url && file.kind === 'media'}
					<a href={href('player', file.path)}>Reproduzir {file.name}</a>
				{:else if url}
					<a href={url} download={file.name} target="_blank" rel="noopener noreferrer">Baixar {file.name}</a>
				{:else}
					<span>{file.name}</span>
				{/if}
				{#if fileStatus(file)} <small>({fileStatus(file)})</small>{/if}
			</li>
		{/each}
	</ul>
</section>
