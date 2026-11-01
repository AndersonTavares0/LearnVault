<script lang="ts">
	import LooseFiles from './LooseFiles.svelte';
	import StudyNotes from './StudyNotes.svelte';
	import Flashcards from './Flashcards.svelte';
	import {
		formatDuration,
		mediaAvailability,
		mediaPlaybackKind,
		nextSpeed,
		PLAYBACK_SPEEDS,
		playerShortcutAction,
		selectPlayableMedia,
		selectPlayableSubtitle,
		shouldAutoAdvance,
		srtConversionSupported,
		srtToWebVtt,
		studySyncEnabled,
		subtitleFormat,
		type PlaybackSpeed
	} from './player';
	import { createPositionSaver } from './study';
	import { createDefaultPlayerPreferences, lessonPreferenceKey, updateLessonPreference, type PlayerPreferences } from './player-preferences';
	import { transport, type Lesson, type StudyProgress } from './transport';

	let {
		lesson,
		courseId,
		mediaError,
		onMediaError,
		studyEnabled = true,
		preferences = createDefaultPlayerPreferences(),
		onPreferencesChange = () => true,
		autoNext = false,
		hasNextLesson = false,
		hasPreviousLesson = false,
		onAutoNext = () => undefined,
		onAutoNextChange = () => true,
		onPreviousLesson = () => undefined,
		onNextLesson = () => undefined,
		lessonTitle = null,
		initialPosition = null
	}: {
		lesson: Lesson;
		courseId: string;
		mediaError: string;
		onMediaError: (code?: number) => void;
		studyEnabled?: boolean;
		preferences?: PlayerPreferences;
		onPreferencesChange?: (next: PlayerPreferences) => boolean;
		autoNext?: boolean;
		hasNextLesson?: boolean;
		hasPreviousLesson?: boolean;
		onAutoNext?: () => void;
		onAutoNextChange?: (enabled: boolean) => boolean;
		onPreviousLesson?: () => void;
		onNextLesson?: () => void;
		lessonTitle?: string | null;
		initialPosition?: number | null;
	} = $props();

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
	let playerContainerRef: HTMLElement | null = $state(null);
	let currentTime = $state(0);
	let duration = $state(0);
	let lastAppliedPosition = $state<number | null>(null);

	$effect(() => {
		if (source) {
			lastAppliedPosition = null;
			playbackErrorCode = null;
		}
	});

	$effect(() => {
		if (initialPosition && initialPosition > 0 && media && initialPosition !== lastAppliedPosition) {
			media.currentTime = initialPosition;
			lastAppliedPosition = initialPosition;
		}
	});
	let progress: StudyProgress | null = $state(null);
	let progressLoading = $state(false);
	let progressError = $state('');
	let saving = $state(false);
	let lastSavedAt = $state(0);

	let captionSource = $state<string | null>(null);
	let captionDownload = $state<string | null>(null);
	let captionMessage = $state('');
	let playbackErrorCode: number | null = $state(null);

	let studyTab: 'notes' | 'flashcards' = $state('notes');

	function studyTabKey(event: KeyboardEvent) {
		if (event.key === 'ArrowRight' || event.key === 'ArrowLeft') {
			event.preventDefault();
			const next = studyTab === 'notes' ? 'flashcards' : 'notes';
			studyTab = next;
			setTimeout(() => document.getElementById(`study-tab-${next}`)?.focus(), 0);
		} else if (event.key === 'Home') {
			event.preventDefault();
			studyTab = 'notes';
			setTimeout(() => document.getElementById('study-tab-notes')?.focus(), 0);
		} else if (event.key === 'End') {
			event.preventDefault();
			studyTab = 'flashcards';
			setTimeout(() => document.getElementById('study-tab-flashcards')?.focus(), 0);
		}
	}

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

	function changeSpeed(speed: PlaybackSpeed) {
		if (playbackKind === 'audio') {
			onPreferencesChange({ ...preferences, audioSpeed: speed });
		} else {
			onPreferencesChange({ ...preferences, videoSpeed: speed });
		}
	}

	function seekOffset(seconds: number) {
		if (!media) return;
		const target = media.currentTime + seconds;
		media.currentTime = Math.max(0, Math.min(media.duration || Number.POSITIVE_INFINITY, target));
	}

	function toggleFullscreen() {
		if (!playerContainerRef) return;
		if (document.fullscreenElement) {
			void document.exitFullscreen().catch(() => undefined);
		} else {
			void playerContainerRef.requestFullscreen().catch(() => undefined);
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		const target = event.target as HTMLElement | null;
		if (target && (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT' || target.isContentEditable)) {
			return;
		}

		if (event.key === '[' || event.key === '{') {
			event.preventDefault();
			changeSpeed(nextSpeed(playbackSpeed, 'down'));
			return;
		}
		if (event.key === ']' || event.key === '}') {
			event.preventDefault();
			changeSpeed(nextSpeed(playbackSpeed, 'up'));
			return;
		}
		if ((event.key === 'f' || event.key === 'F') && playbackKind === 'video') {
			event.preventDefault();
			toggleFullscreen();
			return;
		}

		const isInteractiveButton = target?.tagName === 'BUTTON';
		if (isInteractiveButton && (event.key === ' ' || event.key === 'Spacebar')) {
			return;
		}

		const isPlayerContext =
			event.target === event.currentTarget ||
			event.target === media ||
			(playerContainerRef !== null && playerContainerRef.contains(target as Node));
		const action = playerShortcutAction(event.key, isPlayerContext);
		if (!action || !media) return;
		event.preventDefault();
		if (action === 'toggle-playback') {
			if (media.paused) void media.play().catch(() => undefined);
			else media.pause();
		} else {
			const offset = action === 'seek-backward' ? -10 : 10;
			seekOffset(offset);
		}
	}

	function canResumeFrom(p: StudyProgress | null, total: number): boolean {
		return syncEnabled && p !== null && p.recorded && p.positionSeconds > 5 && total > p.positionSeconds;
	}
	function showCompletedFrom(_p: StudyProgress | null): boolean {
		return syncEnabled && mediaId !== null;
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
			captionMessage = 'Legendas ASS/SSA contêm estilos avançados não suportados nativamente pelo reprodutor; baixe o arquivo para usar externamente.';
		} else if (format === 'sub') {
			captionMessage = 'Legendas em formato .sub utilizam estrutura gráfica/MicroDVD não suportada nativamente; baixe o arquivo para uso externo.';
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
		void media.play().catch(() => undefined);
	}

	async function toggleCompleted() {
		if (!syncEnabled || !mediaId || saving || saver.saving) return;
		saving = true;
		try {
			const next = await transport.setCompleted(mediaId, !(progress?.completed ?? false));
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

	function handleMediaError() {
		const code = media?.error?.code ?? null;
		playbackErrorCode = code;
		onMediaError(code ?? undefined);
	}

	function handleLoadedMetadata() {
		if (initialPosition && initialPosition > 0 && media) {
			media.currentTime = initialPosition;
			lastAppliedPosition = initialPosition;
		}
	}
</script>
{#snippet lessonHeading()}
	{#if lessonTitle}<h2 class="player-lesson-title">{lessonTitle}</h2>{/if}
{/snippet}
{#if mediaError}
	<div class="lv-player-error" role="alert">
		<strong>{mediaError}</strong>
		{#if playbackErrorCode === 4}
			<p>O arquivo foi localizado, mas o formato ou codec desta mídia não é suportado pelo seu navegador/webview atual (ex.: HEVC/H.265, AC3 ou contêiner específico). É possível assistir com um reprodutor externo ou convertê-lo para MP4 (H.264 / AAC).</p>
		{:else if playbackErrorCode === 3}
			<p>A decodificação do áudio/vídeo falhou. O arquivo pode estar corrompido ou o codec não é suportado pelo hardware.</p>
		{:else}
			<p>O arquivo local pode ter sido movido ou o disco pode estar desconectado. Reconecte a pasta e use “Atualizar cursos” na biblioteca; seu progresso permanece salvo.</p>
		{/if}
		<a class="lv-btn lvui-btn" href="#/home">Ir à biblioteca para atualizar cursos</a>
	</div>{@render lessonHeading()}
{:else if source}
	<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions (o grupo identificado do player recebe foco para ativar atalhos) -->
	<div bind:this={playerContainerRef} class="player-container {playbackKind === 'audio' ? 'audio-container' : 'video-container'}" role="group" tabindex="0" onkeydown={handleKeydown} aria-label="Player" aria-keyshortcuts="Space ArrowLeft ArrowRight k j l [ ] f">
		<div class="player-top-controls">
			{#if availableMedia.length > 1}<label class="player-select-label">Mídia da aula<select aria-label="Mídia da aula" value={playable?.mediaId ?? ''} onchange={selectMedia}>{#each availableMedia as item}<option value={item.mediaId ?? ''}>{item.name}</option>{/each}</select></label>{/if}
			{#if availableSubtitles.length > 0}<label class="player-select-label">Legenda<select aria-label="Legenda da aula" value={lessonPreference.subtitleId === null ? '' : selectedSubtitle?.mediaId ?? ''} onchange={selectSubtitle}><option value="">Sem legendas</option>{#each availableSubtitles as item}<option value={item.mediaId ?? ''}>{item.name}</option>{/each}</select></label>{/if}
		</div>
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
			<audio class="player-media player-audio" controls preload="metadata" src={source} aria-label={`Player da aula${lessonTitle ? `: ${lessonTitle}` : ''}`} bind:this={media} bind:currentTime={currentTime} bind:duration={duration} ontimeupdate={handleTimeUpdate} onended={handleEnded} onloadedmetadata={handleLoadedMetadata} onerror={handleMediaError}>
				{#if captionSource}<track kind="captions" src={captionSource} label="Legenda" default />{/if}
			</audio>
		{:else}
			<!-- svelte-ignore a11y_media_has_caption -->
			<video class="player-media" controls preload="metadata" src={source} aria-label={`Player da aula${lessonTitle ? `: ${lessonTitle}` : ''}`} bind:this={media} bind:currentTime={currentTime} bind:duration={duration} ontimeupdate={handleTimeUpdate} onended={handleEnded} onloadedmetadata={handleLoadedMetadata} onerror={handleMediaError}>
				{#if captionSource}<track kind="captions" src={captionSource} label="Legenda" default />{/if}
			</video>
		{/if}

		<!-- Barra de Controles Rápidos do Player -->
		<div class="player-quick-bar" role="toolbar" aria-label="Controles rápidos do reprodutor">
			<!-- Seek Rápido -->
			<div class="player-btn-group" role="group" aria-label="Busca de tempo">
				<button type="button" class="player-tool-btn" onclick={() => seekOffset(-10)} title="Voltar 10s (seta esquerda ou J)">
					<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
						<path d="M11 17l-5-5 5-5M18 17l-5-5 5-5" />
					</svg>
					<span>-10s</span>
				</button>
				<button type="button" class="player-tool-btn" onclick={() => seekOffset(10)} title="Avançar 10s (seta direita ou L)">
					<span>+10s</span>
					<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
						<path d="M13 17l5-5-5-5M6 17l5-5-5-5" />
					</svg>
				</button>
			</div>

			<!-- Seletor de Velocidade -->
			<div class="player-speed-selector" role="group" aria-label="Velocidade de reprodução">
				<span class="player-tool-label">Velocidade:</span>
				<div class="player-speed-pills">
					{#each PLAYBACK_SPEEDS as sp (sp)}
						<button
							type="button"
							class="player-speed-btn"
							class:is-active={playbackSpeed === sp}
							aria-pressed={playbackSpeed === sp}
							onclick={() => changeSpeed(sp)}
						>
							{sp}×
						</button>
					{/each}
				</div>
			</div>

			<!-- Navegação de Aulas e Tela Cheia -->
			<div class="player-nav-group" role="group" aria-label="Navegação de aulas">
				<button
					type="button"
					class="player-tool-btn player-nav-btn"
					disabled={!hasPreviousLesson}
					onclick={onPreviousLesson}
					title="Ir para aula anterior"
				>
					← Anterior
				</button>
				<button
					type="button"
					class="player-tool-btn player-nav-btn"
					disabled={!hasNextLesson}
					onclick={onNextLesson}
					title="Ir para próxima aula"
				>
					Próxima →
				</button>
				{#if playbackKind === 'video'}
					<button
						type="button"
						class="player-tool-btn player-fullscreen-btn"
						onclick={toggleFullscreen}
						title="Alternar tela cheia (F)"
						aria-label="Tela cheia"
					>
						<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
							<path d="M8 3H5a2 2 0 00-2 2v3m18 0V5a2 2 0 00-2-2h-3m0 18h3a2 2 0 002-2v-3M3 16v3a2 2 0 002 2h3" />
						</svg>
					</button>
				{/if}
			</div>
		</div>

		{@render lessonHeading()}
	</div>
{:else if availability === 'missing'}<div class="lv-player-error" role="status"><strong>Mídia indisponível</strong><p>A mídia desta aula não está no disco. A aula e o seu progresso foram preservados. Reconecte o disco ou reponha o arquivo e use “Atualizar cursos” na biblioteca.</p><a class="lv-btn lvui-btn" href="#/home">Ir à biblioteca para atualizar cursos</a></div>{@render lessonHeading()}
{:else}<p class="empty" role="status">Esta aula não possui mídia disponível.</p>{@render lessonHeading()}{/if}
{#if source || syncEnabled}
	<div class="study-bar" role="group" aria-label="Controles da aula">
		{#if source}
			<label class="auto-next-toggle"><input type="checkbox" checked={autoNext} onchange={(event) => changeAutoNext(event.currentTarget.checked)} />Avançar automaticamente</label>
		{/if}
		{#if syncEnabled}
			<div class="study-bar__actions">
				{#if progressLoading}<span class="study-hint">Carregando progresso…</span>
				{:else if progressError}<span class="study-hint error">{progressError}</span>
				{:else if canResume}<button class="text-button" onclick={resume} type="button">Retomar de {formatDuration(progress?.positionSeconds ?? 0)}</button>
				{:else if showCompleted}
					<div class="study-completion" role="group" aria-label="Conclusão da aula">
						<span class="study-hint">{progress?.completed ? 'Concluído' : 'Não concluído'}</span>
						<button class="lv-btn lvui-btn" onclick={toggleCompleted} disabled={saving || saver.saving} type="button" aria-pressed={progress?.completed}>{progress?.completed ? 'Marcar como não concluído' : 'Marcar como concluído'}</button>
					</div>
				{/if}
			</div>
		{/if}
	</div>
{/if}
{#if captionMessage}<p class="empty" role="status">{captionMessage}{#if captionDownload} <a href={captionDownload} download>Baixar legenda</a>{/if}</p>{/if}
{#if lesson.subtitles.length || lesson.materials.length}<LooseFiles files={[...lesson.subtitles, ...lesson.materials]} heading="Materiais" {courseId} />{/if}
{#if syncEnabled && mediaId}
	<div class="player-study-section">
		<div class="lv-tabs player-study-tabs" role="tablist" aria-label="Ferramentas de estudo da aula">
			<button
				class="lv-tab"
				type="button"
				role="tab"
				id="study-tab-notes"
				aria-controls="study-panel-notes"
				aria-selected={studyTab === 'notes'}
				tabindex={studyTab === 'notes' ? 0 : -1}
				class:is-active={studyTab === 'notes'}
				onclick={() => (studyTab = 'notes')}
				onkeydown={studyTabKey}
			>
				Anotações da aula
			</button>
			<button
				class="lv-tab"
				type="button"
				role="tab"
				id="study-tab-flashcards"
				aria-controls="study-panel-flashcards"
				aria-selected={studyTab === 'flashcards'}
				tabindex={studyTab === 'flashcards' ? 0 : -1}
				class:is-active={studyTab === 'flashcards'}
				onclick={() => (studyTab = 'flashcards')}
				onkeydown={studyTabKey}
			>
				Cartões de revisão
			</button>
		</div>

		{#if studyTab === 'notes'}
			<div id="study-panel-notes" role="tabpanel" aria-labelledby="study-tab-notes">
				<StudyNotes
					{mediaId}
					{currentTime}
					onSeek={(targetSeconds) => {
						if (media) {
							media.currentTime = targetSeconds;
							void media.play().catch(() => undefined);
						}
					}}
				/>
			</div>
		{:else}
			<div id="study-panel-flashcards" role="tabpanel" aria-labelledby="study-tab-flashcards">
				<Flashcards {mediaId} />
			</div>
		{/if}
	</div>
{/if}

<style>
	.player-top-controls {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		flex-wrap: wrap;
		margin-bottom: 0.5rem;
	}

	.player-quick-bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		flex-wrap: wrap;
		padding: 0.5rem 0.75rem;
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		margin-top: 0.5rem;
		font-size: 0.8125rem;
	}

	.player-btn-group,
	.player-nav-group,
	.player-speed-selector {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
	}

	.player-tool-label {
		color: var(--muted);
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
		margin-right: 0.2rem;
	}

	.player-tool-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.3rem;
		background: var(--surface);
		border: 1px solid var(--line);
		color: var(--ink);
		padding: 0.25rem 0.55rem;
		border-radius: calc(var(--radius) - 2px);
		font-size: 0.8125rem;
		font-weight: 500;
		cursor: pointer;
		transition: background-color 0.15s ease, border-color 0.15s ease;
	}

	.player-tool-btn:hover:not(:disabled) {
		background: var(--hover);
		border-color: var(--line-strong);
	}

	.player-tool-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.player-speed-pills {
		display: inline-flex;
		align-items: center;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: calc(var(--radius) - 2px);
		padding: 2px;
		gap: 2px;
	}

	.player-speed-btn {
		background: transparent;
		border: 1px solid transparent;
		color: var(--muted);
		padding: 0.15rem 0.4rem;
		font-size: 0.75rem;
		font-weight: 600;
		border-radius: 4px;
		cursor: pointer;
		transition: all 0.12s ease;
	}

	.player-speed-btn:hover {
		color: var(--ink);
	}

	.player-speed-btn.is-active {
		background: var(--surface-2);
		color: var(--accent);
		border-color: var(--line);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
	}

	.player-fullscreen-btn {
		padding: 0.25rem 0.4rem;
	}

	:global(.player-container:fullscreen) {
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		background: #000000;
		width: 100vw;
		height: 100vh;
		padding: 1rem;
		box-sizing: border-box;
	}

	:global(.player-container:fullscreen .player-media) {
		max-height: calc(100vh - 120px);
		width: 100%;
		object-fit: contain;
	}

	:global(.player-container:fullscreen .player-quick-bar) {
		max-width: 900px;
		width: 100%;
		background: rgba(24, 34, 48, 0.95);
		border-color: rgba(255, 255, 255, 0.15);
	}

	.player-study-section {
		margin-top: 1.5rem;
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.player-study-tabs {
		background: transparent;
		padding: 0;
		border-bottom: 1px solid var(--line);
	}

	@media (max-width: 640px) {
		.player-quick-bar {
			flex-direction: column;
			align-items: stretch;
			gap: 0.65rem;
		}

		.player-btn-group,
		.player-nav-group,
		.player-speed-selector {
			justify-content: space-between;
			width: 100%;
		}
	}
</style>
