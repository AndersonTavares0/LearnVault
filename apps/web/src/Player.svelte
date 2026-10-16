<script lang="ts">
	import LooseFiles from './LooseFiles.svelte';
	import { formatDuration, selectPlayableMedia, srtConversionSupported, srtToWebVtt, studySyncEnabled, subtitleFormat } from './player';
	import { createPositionSaver } from './study';
	import { transport, type Lesson, type StudyProgress } from './transport';

	let { lesson, mediaError, onMediaError, studyEnabled = true }: { lesson: Lesson; mediaError: string; onMediaError: () => void; studyEnabled?: boolean } = $props();

	const playable = $derived(selectPlayableMedia(lesson.media));
	const mediaId = $derived(playable?.mediaId ?? null);
	const source = $derived(playable ? transport.mediaUrl(playable.path) : null);
	const syncEnabled = $derived(studySyncEnabled(studyEnabled, mediaId, source));

	let video: HTMLVideoElement | null = $state(null);
	let currentTime = $state(0);
	let duration = $state(0);
	let progress: StudyProgress | null = $state(null);
	let progressLoading = $state(false);
	let progressError = $state('');
	let saving = $state(false);
	let lastSavedAt = $state(0);

	let captionSource = $state<string | null>(null);
	let captionDownload = $state<string | null>(null);
	let captionMessage = $state('');

	const saver = createPositionSaver((id, position) => transport.recordPosition(id, position));

	function canResumeFrom(p: StudyProgress | null, total: number): boolean {
		return syncEnabled && p !== null && p.recorded && p.positionSeconds > 5 && total > p.positionSeconds;
	}
	function showCompletedFrom(p: StudyProgress | null): boolean {
		return syncEnabled && p !== null && p.recorded;
	}
	const canResume = $derived(canResumeFrom(progress, duration));
	const showCompleted = $derived(showCompletedFrom(progress));

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

	$effect(() => {
		if (!syncEnabled) { progress = null; progressError = ''; return; }
		progressLoading = true;
		progressError = '';
		saver.reset();
		let active = true;
		const currentId = mediaId!;
		transport.studyProgress(currentId)
			.then((value) => { if (active) progress = value; })
			.catch((cause) => { if (active) progressError = cause instanceof Error ? cause.message : 'Erro ao ler progresso.'; })
			.finally(() => { if (active) progressLoading = false; });
		return () => { active = false; saver.reset(); };
	});

	function resume() {
		if (!video || !progress?.recorded) return;
		video.currentTime = progress.positionSeconds;
	}

	async function toggleCompleted() {
		if (!syncEnabled || !mediaId || !progress?.recorded || saving || saver.saving) return;
		saving = true;
		try {
			const next = await transport.setCompleted(mediaId, !progress.completed);
			if (mediaId === (playable?.mediaId ?? null)) progress = next;
		} catch (cause) {
			progressError = cause instanceof Error ? cause.message : 'Erro ao atualizar conclusão.';
		} finally {
			saving = false;
		}
	}

	async function handleTimeUpdate() {
		if (!syncEnabled || !mediaId || !video || saving || saver.saving) return;
		const now = performance.now();
		if (now - lastSavedAt < 5000) return;
		const position = Math.floor(video.currentTime);
		if (position <= 0) return;
		lastSavedAt = now;
		const result = await saver.request(mediaId, position);
		if (result.kind === 'saved' && result.progress.mediaId === mediaId) progress = result.progress;
	}

	async function handleEnded() {
		if (!syncEnabled || !mediaId) return;
		try {
			const next = await transport.setCompleted(mediaId, true);
			if (next.mediaId === mediaId) progress = next;
		} catch {
			// Ignora erros ao marcar conclusão automaticamente.
		}
	}
</script>
{#if mediaError}<p class="error" role="alert">{mediaError}</p>
{:else if source}<!-- svelte-ignore a11y_media_has_caption --><video controls preload="metadata" src={source} bind:this={video} bind:currentTime={currentTime} bind:duration={duration} ontimeupdate={handleTimeUpdate} onended={handleEnded} onerror={onMediaError}>{#if captionSource}<track kind="captions" src={captionSource} label="Legenda" default />{/if}</video>
{:else}<p class="empty" role="status">Esta aula não possui mídia disponível.</p>{/if}
{#if syncEnabled}
	<div class="study-bar" role="group" aria-label="Progresso da aula">
		{#if progressLoading}<span class="study-hint">Carregando progresso…</span>
		{:else if progressError}<span class="study-hint error">{progressError}</span>
		{:else if canResume}<button class="text-button" onclick={resume} type="button">Retomar de {formatDuration(progress?.positionSeconds ?? 0)}</button>
		{:else if showCompleted}<span class="study-hint">{progress?.completed ? 'Concluído' : 'Não concluído'}</span>{/if}
		{#if showCompleted}<button class="text-button" onclick={toggleCompleted} disabled={saving || saver.saving} type="button" aria-pressed={progress?.completed}>{progress?.completed ? 'Marcar como não concluído' : 'Marcar como concluído'}</button>{/if}
	</div>
{/if}
{#if captionMessage}<p class="empty" role="status">{captionMessage}{#if captionDownload} <a href={captionDownload} download>Baixar legenda</a>{/if}</p>{/if}
<LooseFiles files={[...lesson.subtitles, ...lesson.materials]} heading="Materiais" />
