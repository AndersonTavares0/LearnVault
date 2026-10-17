<script lang="ts">
 import { fileStatus, transport, type FileEntry } from './transport';
 let { files, heading, courseId }: { files: FileEntry[]; heading: string; courseId?: string } = $props();
</script>
<section class="loose-files" aria-label={heading}><strong>{heading}</strong><ul>{#each files as file}<li>{#if transport.mediaUrl(file, courseId)}<a href={transport.mediaUrl(file, courseId)}>{file.name}{#if fileStatus(file)} <small>({fileStatus(file)})</small>{/if}</a>{:else}<span>{file.name}{#if fileStatus(file)} <small>({fileStatus(file)})</small>{/if}</span>{/if}</li>{/each}</ul></section>
