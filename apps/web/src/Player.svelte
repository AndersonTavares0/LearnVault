<script lang="ts">
	import LooseFiles from './LooseFiles.svelte';
	import { clampSpeed, formatDuration, mediaAvailability, mediaPlaybackKind, PLAYBACK_SPEEDS, playerShortcutAction, selectPlayableMedia, selectPlayableSubtitle, shouldAutoAdvance, srtConversionSupported, srtToWebVtt, studySyncEnabled, subtitleFormat } from './player';
	import { createPositionSaver } from './study';
	import { createDefaultPlayerPreferences, lessonPreferenceKey, updateLessonPreference, type PlayerPreferences } from './player-preferences';
	import { transport, type Lesson, type StudyProgress } from './transport';

	let { lesson, courseId, mediaError, onMediaError, studyEnabled = true, preferences = createDefaultPlayerPreferences(), onPreferencesChange = () => true, autoNext = false, hasNextLesson = false, onAutoNext = () => undefined, onAutoNextChange = () => true }: { lesson: Lesson; courseId: string; mediaError: string; onMediaError: () => void; studyEnabled?: boolean; preferences?: PlayerPreferences; onPreferencesChange?: (next: PlayerPreferences) => boolean; autoNext?: boolean; hasNextLesson?: boolean; onAutoNext?: () => void; onAutoNextChange?: (enabled: boolean) => boolean } = $props();

	const lessonPreference = $derived(preferences.lessons[lessonPreferenceKey(courseId, lesson.path)] ?? {});
	const availableMedia = $derived(lesson.media.filter((file) => file.availability?.trim().toLowerCase() !== 'missing'));
	const availableSubtitles = $derived(lesson.subtitles.filter((file) => file.availability?.trim().toLowerCase() !== 'missing'));
	const playable = $derived(selectPlayableMedia(lesson.media, lessonPreference.mediaId));
	const selectedSubtitle = $derived(selectPlayableSubtitle(lesson.subtitles, lessonPreference.subtitleId));
	const availability = $derived(mediaAvailability(lesson.media));
	const mediaId = $derived(playable?.mediaId ?? null);
	const source = $derived(playable ? transport.mediaUrl(playable, courseId) : null);
	const playbackKind = $derived(playable ? mediaPlaybackKind(playable) : 'video');
	const playbackSpeed = $derived(playbackKind === 'audio' ? preferences.audioSpeed : preferences.videoSpeed);
	const syncEnabled = $derived(studySyncEnabled(studyEnabled, mediaId, source));

	let media: HTMLMediaElement | null = $state(null);
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
	let preferenceMessage = $state('');

	const saver = createPositionSaver((id, position) => transport.recordPosition(id, position));

	function saveLessonPreference(patch: { mediaId?: string; subtitleId?: string | null }) {
		preferenceMessage = onPreferencesChange(updateLessonPreference(preferences, courseId, lesson.path, patch))
			? 'Preferência do player salva.'
			: 'Não foi possível salvar a preferência; ela valerá apenas até fechar o aplicativo.';
	}

	function selectMedia(event: Event) {
		const id = (event.currentTarget as HTMLSelectElement).value;
		if (id) saveLessonPreference({ mediaId: id });
	}

	function selectSubtitle(event: Event) {
		const id = (event.currentTarget as HTMLSelectElement).value;
		saveLessonPreference({ subtitleId: id || null });
	}

	function changeAutoNext(enabled: boolean) {
		preferenceMessage = onAutoNextChange(enabled)
			? 'Preferência de avanço automático salva.'
			: 'Não foi possível salvar a preferência; ela valerá apenas até fechar o aplicativo.';
	}

	function handleKeydown(event: KeyboardEvent) {
		const action = playerShortcutAction(event.key, event.target === event.currentTarget);
		if (!action || !media) return;
		event.preventDefault();
		if (action === 'toggle-playback') {
			if (media.paused) void media.play().catch(() => undefined);
			else media.pause();
		} else {
			const offset = action === 'seek-backward' ? -10 : 10;
			media.currentTime = Math.max(0, Math.min(media.duration || Number.POSITIVE_INFINITY, media.currentTime + offset));
		}
	}

	function canResumeFrom(p: StudyProgress | null, total: number): boolean {
		return syncEnabled && p !== null && p.recorded && p.positionSeconds > 5 && total > p.positionSeconds;
	}
	function showCompletedFrom(p: StudyProgress | null): boolean {
		return syncEnabled && p !== null && p.recorded;
	}
	const canResume = $derived(canResumeFrom(progress, duration));
	const showCompleted = $derived(showCompletedFrom(progress));

	$effect(() => {
		const subtitle = selectedSubtitle;
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
		if (media) media.playbackRate = target;
		const next = playbackKind === 'audio' ? { ...preferences, audioSpeed: target } : { ...preferences, videoSpeed: target };
		preferenceMessage = onPreferencesChange(next)
			? 'Velocidade salva.'
			: 'Não foi possível salvar a velocidade; ela valerá apenas até fechar o aplicativo.';
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
		if (syncEnabled && mediaId) {
			try {
				const next = await transport.setCompleted(mediaId, true);
				if (next.mediaId === mediaId) progress = next;
			} catch {
				// Ignora erros ao marcar conclusão automaticamente.
			}
		}
		if (shouldAutoAdvance(autoNext, hasNextLesson)) onAutoNext();
	}
</script>
{#if mediaError}<p class="error" role="alert">{mediaError}</p>
{:else if source}
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (the labeled player group is keyboard-focusable for shortcuts) -->
	<div class="player-container {playbackKind === 'audio' ? 'audio-container' : 'video-container'}" role="group" tabindex="0" onkeydown={handleKeydown} aria-label="Player. Espaço reproduz ou pausa; setas esquerda e direita avançam ou retrocedem 10 segundos.">
		{#if availableMedia.length > 1}<label class="player-select-label">Mídia da aula<select aria-label="Mídia da aula" value={playable?.mediaId ?? ''} onchange={selectMedia}>{#each availableMedia as item}<option value={item.mediaId ?? ''}>{item.name}</option>{/each}</select></label>{/if}
		{#if availableSubtitles.length > 0}<label class="player-select-label">Legenda<select aria-label="Legenda da aula" value={lessonPreference.subtitleId === null ? '' : selectedSubtitle?.mediaId ?? ''} onchange={selectSubtitle}><option value="">Sem legendas</option>{#each availableSubtitles as item}<option value={item.mediaId ?? ''}>{item.name}</option>{/each}</select></label>{/if}
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
		<label class="auto-next-toggle"><input type="checkbox" checked={autoNext} onchange={(event) => changeAutoNext(event.currentTarget.checked)} />Avançar automaticamente</label>
		</div>
		{#if preferenceMessage}<p class="catalog-message" role="status">{preferenceMessage}</p>{/if}
	</div>
{:else if availability === 'missing'}<p class="empty" role="status">A mídia desta aula não está no disco. A aula e o seu progresso foram preservados; indexe os cursos novamente depois de reconectar o disco ou repor o arquivo.</p>
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
<LooseFiles files={[...lesson.subtitles, ...lesson.materials]} heading="Materiais" {courseId} />
