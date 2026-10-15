<script lang="ts">
 import { fileStatus, transport, type FileEntry } from './transport';
 let { files, heading }: { files: FileEntry[]; heading: string } = $props();
</script>
<section class="loose-files" aria-label={heading}><strong>{heading}</strong>{#each files as file}<a href={transport.mediaUrl(file.path)}>{file.name}{#if fileStatus(file)} <small>({fileStatus(file)})</small>{/if}</a>{/each}</section>
