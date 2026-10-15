<script lang="ts">
  import LooseFiles from './LooseFiles.svelte';
  import { selectPlayableMedia, srtConversionSupported, srtToWebVtt, subtitleFormat } from './player';
  import { transport, type Lesson } from './transport';
  let { lesson, mediaError, onMediaError }: { lesson: Lesson; mediaError: string; onMediaError: () => void } = $props();
  let source = $derived(selectPlayableMedia(lesson.media) ? transport.mediaUrl(selectPlayableMedia(lesson.media)!.path) : null);
  let captionSource = $state<string | null>(null);
  let captionDownload = $state<string | null>(null);
  let captionMessage = $state('');
  $effect(() => {
    const subtitle = lesson.subtitles[0];
    const url = subtitle ? transport.mediaUrl(subtitle.path) : null;
    let active = true;
    let objectUrl: string | null = null;
    captionSource = null;
    captionDownload = url;
    captionMessage = '';
    if (!subtitle || !url) return () => { active = false; };
    const format = subtitleFormat(subtitle.path);
    if (format === 'vtt') {
      captionSource = url;
    } else if (format === 'ass') {
      captionMessage = 'Legendas ASS/SSA não são compatíveis com <track>; baixe o arquivo para usar um conversor.';
    } else if (format === 'srt' && srtConversionSupported) {
      void fetch(url).then((response) => {
        if (!response.ok) throw new Error('Falha ao carregar legenda.');
        return response.text();
      }).then((text) => {
        if (active) {
          objectUrl = URL.createObjectURL(new Blob([srtToWebVtt(text)], { type: 'text/vtt' }));
          captionSource = objectUrl;
        }
      }).catch(() => { if (active) captionMessage = 'Não foi possível converter a legenda SRT.'; });
    } else if (format === 'srt') {
      // O build desktop não faz rede: a conversão sob demanda só existe no build HTTP.
      captionMessage = 'A conversão de legendas SRT exige o servidor HTTP; baixe o arquivo para uso externo.';
    } else {
      captionMessage = 'Formato de legenda não suportado; baixe o arquivo para uso externo.';
    }
    return () => { active = false; if (objectUrl) URL.revokeObjectURL(objectUrl); };
  });
</script>
{#if mediaError}<p class="error" role="alert">{mediaError}</p>{:else if source}<!-- svelte-ignore a11y_media_has_caption --><video controls preload="metadata" src={source} onerror={onMediaError}>{#if captionSource}<track kind="captions" src={captionSource} label="Legenda" default />{/if}</video>{:else}<p class="empty" role="status">Esta aula não possui mídia disponível.</p>{/if}
{#if captionMessage}<p class="empty" role="status">{captionMessage}{#if captionDownload} <a href={captionDownload} download>Baixar legenda</a>{/if}</p>{/if}
<LooseFiles files={[...lesson.subtitles, ...lesson.materials]} heading="Materiais" />
