<script lang="ts">
 import LooseFiles from './LooseFiles.svelte';
 import { transport, type Lesson } from './transport';
 let { lesson, mediaError, onMediaError }: { lesson: Lesson; mediaError: string; onMediaError: () => void } = $props();
 let source = $derived(transport.mediaUrl(lesson.media[0]?.path ?? ''));
</script>
{#if mediaError}<p class="error" role="alert">{mediaError}</p>{:else if source}<video controls preload="metadata" src={source} onerror={onMediaError}><track kind="captions" src={lesson.subtitles[0] ? transport.mediaUrl(lesson.subtitles[0].path) ?? undefined : 'data:text/vtt,WEBVTT'} label="Legenda" default /></video>{:else}<p class="empty" role="status">Esta aula não possui mídia disponível.</p>{/if}
<LooseFiles files={[...lesson.subtitles, ...lesson.materials]} heading="Materiais" />
