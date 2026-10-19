<script lang="ts">
	import LooseFiles from './LooseFiles.svelte';
	import { formatDuration, mediaAvailability, mediaPlaybackKind, playerShortcutAction, selectPlayableMedia, selectPlayableSubtitle, shouldAutoAdvance, srtConversionSupported, srtToWebVtt, studySyncEnabled, subtitleFormat } from './player';
	import { createPositionSaver } from './study';
	import { createDefaultPlayerPreferences, lessonPreferenceKey, updateLessonPreference, type PlayerPreferences } from './player-preferences';
	import { transport, type Lesson, type StudyProgress } from './transport';

	let { lesson, courseId, mediaError, onMediaError, studyEnabled = true, preferences = createDefaultPlayerPreferences(), onPreferencesChange = () => true, autoNext = false, hasNextLesson = false, onAutoNext = () => undefined, onAutoNextChange = () => true, lessonTitle = null, completed, onCompletedChange, onTimeChange }: { lesson: Lesson; courseId: string; mediaError: string; onMediaError: () => void; studyEnabled?: boolean; preferences?: PlayerPreferences; onPreferencesChange?: (next: PlayerPreferences) => boolean; autoNext?: boolean; hasNextLesson?: boolean; onAutoNext?: () => void; onAutoNextChange?: (enabled: boolean) => boolean; lessonTitle?: string | null; completed?: boolean; onCompletedChange?: (mediaId: string, completed: boolean) => void; onTimeChange?: (seconds: number) => void } = $props();

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
	let lastSavedAt = $state(0);
	let lastReportedAt = -1;

	let captionSource = $state<string | null>(null);
	let captionDownload = $state<string | null>(null);
	let captionMessage = $state('');

	const saver = createPositionSaver((id, position) => transport.recordPosition(id, position));

	function saveLessonPreference(patch: { mediaId?: string; subtitleId?: string | null }) {
		onPreferencesChange(updateLessonPreference(preferences, courseId, lesson.path, patch));
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
		onAutoNextChange(enabled);
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
	const canResume = $derived(canResumeFrom(progress, duration));
	// A fonte da verdade da conclusão é a playlist; o progresso local cobre o player
	// isolado (arquivo solto), onde não há lista de aulas.
	function resolveCompleted(fromPlaylist: boolean | undefined, local: StudyProgress | null): boolean {
		if (fromPlaylist !== undefined) return fromPlaylist;
		return local?.completed ?? false;
	}
	const completedNow = $derived(resolveCompleted(completed, progress));
	const showCompleted = $derived(syncEnabled && completedNow);

	export function seek(seconds: number) {
		if (!media) return;
		media.currentTime = Math.max(0, Math.min(media.duration || Number.POSITIVE_INFINITY, seconds));
	}

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

	$effect(() => {
		if (media) {
			media.playbackRate = playbackSpeed;
		}
	});

	function resume() {
		if (!media || !progress?.recorded) return;
		media.currentTime = progress.positionSeconds;
	}

	function reportTime(seconds: number) {
		if (!onTimeChange) return;
		if (Math.abs(seconds - lastReportedAt) < 1) return;
		lastReportedAt = seconds;
		onTimeChange(seconds);
	}

	function handleSeeked() {
		if (media) reportTime(media.currentTime);
	}

	async function handleTimeUpdate() {
		if (!media) return;
		reportTime(media.currentTime);
		if (!syncEnabled || !mediaId || saver.saving) return;
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
			if (onCompletedChange) {
				onCompletedChange(mediaId, true);
			} else {
				try {
					const next = await transport.setCompleted(mediaId, true);
					if (next.mediaId === mediaId) progress = next;
				} catch {
					// Ignora erros ao marcar conclusão automaticamente.
				}
			}
		}
		if (shouldAutoAdvance(autoNext, hasNextLesson)) onAutoNext();
	}
</script>
{#snippet lessonHeading()}
	{#if lessonTitle}<h2 class="player-lesson-title">{lessonTitle}</h2>{/if}
{/snippet}
{#if mediaError}<p class="error" role="alert">{mediaError}</p>{@render lessonHeading()}
{:else if source}
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (o grupo identificado do player recebe foco para ativar atalhos) -->
	<div class="player-container {playbackKind === 'audio' ? 'audio-container' : 'video-container'}" role="group" tabindex="0" onkeydown={handleKeydown} aria-label="Player" aria-keyshortcuts="Space ArrowLeft ArrowRight">
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
			<audio class="player-media player-audio" controls preload="metadata" src={source} bind:this={media} bind:currentTime={currentTime} bind:duration={duration} ontimeupdate={handleTimeUpdate} onseeked={handleSeeked} onended={handleEnded} onerror={onMediaError}>
				{#if captionSource}<track kind="captions" src={captionSource} label="Legenda" default />{/if}
			</audio>
		{:else}
			<!-- svelte-ignore a11y_media_has_caption -->
			<video class="player-media" controls preload="metadata" src={source} bind:this={media} bind:currentTime={currentTime} bind:duration={duration} ontimeupdate={handleTimeUpdate} onseeked={handleSeeked} onended={handleEnded} onerror={onMediaError}>
				{#if captionSource}<track kind="captions" src={captionSource} label="Legenda" default />{/if}
			</video>
		{/if}
		{@render lessonHeading()}
		<div class="player-controls-bar" role="group" aria-label="Preferências de reprodução">
			<label class="auto-next-toggle"><input type="checkbox" checked={autoNext} onchange={(event) => changeAutoNext(event.currentTarget.checked)} />Avançar automaticamente</label>
		</div>
	</div>
{:else if availability === 'missing'}<p class="empty" role="status">A mídia desta aula não está no disco. A aula e o seu progresso foram preservados; indexe os cursos novamente depois de reconectar o disco ou repor o arquivo.</p>{@render lessonHeading()}
{:else}<p class="empty" role="status">Esta aula não possui mídia disponível.</p>{@render lessonHeading()}{/if}
{#if syncEnabled && (progressLoading || progressError || canResume || showCompleted)}
	<div class="study-bar" role="group" aria-label="Progresso da aula">
		{#if progressLoading}<span class="study-hint">Carregando progresso…</span>
		{:else if progressError}<span class="study-hint error">{progressError}</span>
		{:else if canResume}<button class="text-button" onclick={resume} type="button">Retomar de {formatDuration(progress?.positionSeconds ?? 0)}</button>{/if}
		{#if showCompleted}<span class="study-chip">Concluída</span>{/if}
	</div>
{/if}
{#if captionMessage}<p class="empty" role="status">{captionMessage}{#if captionDownload} <a href={captionDownload} download>Baixar legenda</a>{/if}</p>{/if}
{#if lesson.subtitles.length || lesson.materials.length}<LooseFiles files={[...lesson.subtitles, ...lesson.materials]} heading="Materiais" {courseId} />{/if}
