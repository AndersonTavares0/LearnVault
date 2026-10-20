<script lang="ts">
	import LooseFiles from './LooseFiles.svelte';
	import { clampSpeed, formatDuration, mediaAvailability, mediaPlaybackKind, PLAYBACK_SPEEDS, type PlaybackSpeed, selectPlayableMedia, selectPlayableSubtitle, srtConversionSupported, srtToWebVtt, studySyncEnabled, subtitleFormat } from './player';
	import { createPositionSaver } from './study';
	import { transport, type Lesson, type StudyProgress } from './transport';

	let { lesson, courseId, mediaError, onMediaError, studyEnabled = true, materialsInPanel = false }: { lesson: Lesson; courseId: string; mediaError: string; onMediaError: () => void; studyEnabled?: boolean; materialsInPanel?: boolean } = $props();

	const playable = $derived(selectPlayableMedia(lesson.media));
	const availability = $derived(mediaAvailability(lesson.media));
	const mediaId = $derived(playable?.mediaId ?? null);
	const source = $derived(playable ? transport.mediaUrl(playable, courseId) : null);
	const playbackKind = $derived(playable ? mediaPlaybackKind(playable) : 'video');
	const syncEnabled = $derived(studySyncEnabled(studyEnabled, mediaId, source));

	let media: HTMLMediaElement | null = $state(null);
	let playbackSpeed = $state<PlaybackSpeed>(1);
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
		const subtitle = selectPlayableSubtitle(lesson.subtitles);
		const url = subtitle ? transport.mediaUrl(subtitle, courseId) : null;
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

	function changeSpeed(speed: number) {
		const target = clampSpeed(speed);
		playbackSpeed = target;
		if (media) media.playbackRate = target;
	}

	$effect(() => {
		if (media) {
			media.playbackRate = playbackSpeed;
		}
	});

	function resume() {
		if (!media || !progress?.recorded) return;
		media.currentTime = progress.positionSeconds;
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
		if (!syncEnabled || !mediaId || !media || saving || saver.saving) return;
		const now = performance.now();
		if (now - lastSavedAt < 5000) return;
		const position = Math.floor(media.currentTime);
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
{#if mediaError}<div class="video unavailable"><div class="play" aria-hidden="true">▶</div><span class="mode-note">Área de vídeo · proporção 16:9</span><p class="error" role="alert">{mediaError}</p></div>
{:else if source}
	<div class="player-container {playbackKind === 'audio' ? 'audio-container' : 'video-container'}">
		{#if playbackKind === 'audio'}
			<div class="audio-banner" aria-hidden="true">
				<div class="audio-icon-wrap">
					<svg class="audio-glyph" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
						<path d="M9 18V5l12-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="18" cy="16" r="3" />
					</svg>
				</div>
				<span class="audio-label">Faixa de áudio · {lesson.title}</span>
			</div>
			<!-- svelte-ignore a11y_media_has_caption -->
			<audio class="player-media player-audio" controls preload="metadata" src={source} bind:this={media} bind:currentTime={currentTime} bind:duration={duration} ontimeupdate={handleTimeUpdate} onended={handleEnded} onerror={onMediaError}>
				{#if captionSource}<track kind="captions" src={captionSource} label="Legenda" default />{/if}
			</audio>
		{:else}
			<!-- svelte-ignore a11y_media_has_caption -->
			<video class="player-media" controls preload="metadata" src={source} bind:this={media} bind:currentTime={currentTime} bind:duration={duration} ontimeupdate={handleTimeUpdate} onended={handleEnded} onerror={onMediaError}>
				{#if captionSource}<track kind="captions" src={captionSource} label="Legenda" default />{/if}
			</video>
		{/if}
		<div class="player-controls-bar" role="toolbar" aria-label="Controles de reprodução">
			<span class="control-label" id="speed-label">Velocidade:</span>
			<div class="speed-buttons" role="group" aria-labelledby="speed-label">
				{#each PLAYBACK_SPEEDS as speed}
					<button type="button" class="speed-button" class:active={playbackSpeed === speed} onclick={() => changeSpeed(speed)} aria-pressed={playbackSpeed === speed}>
						{speed}x
					</button>
				{/each}
			</div>
		</div>
	</div>
{:else if availability === 'missing'}<p class="empty" role="status">A mídia desta aula não está no disco. A aula e o seu progresso foram preservados; indexe os cursos novamente depois de reconectar o disco ou repor o arquivo.</p>
{:else}<p class="empty" role="status">Esta aula não possui mídia disponível.</p>{/if}
<div class="vtitle"><h2>{lesson.title}</h2>{#if syncEnabled && showCompleted}<button class="btn" type="button" onclick={toggleCompleted} disabled={saving || saver.saving} aria-pressed={progress?.completed}>✓ {progress?.completed ? 'Marcar como não concluída' : 'Marcar como concluída'}</button>{/if}</div>
{#if syncEnabled}
	<div class="study-bar" role="group" aria-label="Progresso da aula">
		{#if progressLoading}<span class="study-hint">Carregando progresso…</span>
		{:else if progressError}<span class="study-hint error">{progressError}</span>
		{:else if canResume}<button class="text-button" onclick={resume} type="button">Retomar de {formatDuration(progress?.positionSeconds ?? 0)}</button>
		{:else if showCompleted}<span class="study-hint">{progress?.completed ? 'Concluído' : 'Não concluído'}</span>{/if}
	</div>
{/if}
{#if captionMessage}<p class="empty" role="status">{captionMessage}{#if captionDownload} <a href={captionDownload} download>Baixar legenda</a>{/if}</p>{/if}
{#if !materialsInPanel}<LooseFiles files={[...lesson.subtitles, ...lesson.materials]} heading="Materiais" {courseId} />{/if}
