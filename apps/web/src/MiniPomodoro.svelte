<script lang="ts">
import { onMount } from 'svelte';
	import { formatClock, modeLabel, POMODORO_MODES, type PomodoroMode } from './pomodoro';
import { focusSync, pauseTimer, pomodoro, refreshFocusData, resetTimer, retryFocusSessions, setFocusOutboxScope, setMode, startTimer } from './pomodoro-store';
	import { href } from './router';

	let { courseTitle = null, lessonTitle = null, hiddenDock = false, syncScope = null }: { courseTitle?: string | null; lessonTitle?: string | null; hiddenDock?: boolean; syncScope?: string | null } = $props();
	let expanded = $state(false);
	let lastHandled = 0;
	let audio: AudioContext | null = null;

	const modeNames: Record<PomodoroMode, string> = { focus: 'Foco', short: 'Pausa curta', long: 'Pausa longa' };
	const clock = $derived(formatClock($pomodoro.remainingSeconds));
	const progress = $derived($pomodoro.totalSeconds > 0 ? Math.min(1, Math.max(0, 1 - $pomodoro.remainingSeconds / $pomodoro.totalSeconds)) : 0);
	const running = $derived($pomodoro.status === 'running');
	const controlLabel = $derived(running ? 'Pausar Pomodoro' : $pomodoro.status === 'paused' ? 'Retomar Pomodoro' : 'Iniciar Pomodoro');

	onMount(() => {
		void refreshFocusData();
	});

	$effect(() => {
		setFocusOutboxScope(syncScope);
		if (syncScope) void refreshFocusData();
	});

	$effect(() => {
		const completed = $pomodoro.lastCompleted;
		if (!completed || completed.at === lastHandled) return;
		lastHandled = completed.at;
		playAlarm();
		sendNotification(completed.mode);
	});

	function playAlarm() {
		const settings = $pomodoro.settings;
		if (!settings.alarmEnabled) return;
		try {
			audio ??= new AudioContext();
			if (audio.state === 'suspended') void audio.resume();
			for (let index = 0; index < settings.alarmRepeats; index += 1) {
				const start = audio.currentTime + index * 0.42;
				const oscillator = audio.createOscillator();
				const gain = audio.createGain();
				oscillator.type = settings.alarmSound;
				oscillator.frequency.setValueAtTime(660 + index * 70, start);
				gain.gain.setValueAtTime(0, start);
				gain.gain.linearRampToValueAtTime(settings.alarmVolume * 0.18, start + 0.015);
				gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.22);
				oscillator.connect(gain).connect(audio.destination);
				oscillator.start(start);
				oscillator.stop(start + 0.25);
			}
		} catch { /* O timer continua funcional se o navegador bloquear áudio. */ }
	}

	function sendNotification(mode: PomodoroMode) {
		if (!$pomodoro.settings.notificationsEnabled || typeof Notification === 'undefined' || Notification.permission !== 'granted') return;
		try {
			new Notification(mode === 'focus' ? 'Foco concluído' : 'Pausa concluída', {
				body: mode === 'focus' ? 'Hora de descansar um pouco.' : 'Volte ao foco quando estiver pronto.',
				tag: 'learnvault-pomodoro'
			});
		} catch { /* Notificações do sistema são opcionais; o estado segue visível no app. */ }
	}

	$effect(() => {
		if ($focusSync.savedAt !== null) void refreshFocusData();
	});
</script>

<aside class="lv-pomo-dock" class:is-expanded={expanded} class:is-hidden={hiddenDock} aria-label="Pomodoro integrado">
	{#if expanded}
		<div class="lv-pomo-dock__panel">
			<div class="lv-pomo-dock__head">
				<div>
					<strong>{lessonTitle ?? courseTitle ?? 'Pomodoro'}</strong>
					<span>{lessonTitle ? courseTitle : 'Timer disponível em qualquer tela'}</span>
				</div>
				<button class="lv-pomo-dock__close" type="button" aria-label="Recolher Pomodoro" onclick={() => (expanded = false)}>×</button>
			</div>
			<div class="lv-pomo-dock__modes" role="group" aria-label="Modo do Pomodoro">
				{#each POMODORO_MODES as mode}
					<button type="button" class:is-active={$pomodoro.mode === mode} aria-pressed={$pomodoro.mode === mode} onclick={() => setMode(mode)}>{modeNames[mode]}</button>
				{/each}
			</div>
			<div class="lv-pomo-dock__clock">
				<span class="lv-pomo-dock__ring" style:--pomo-progress={`${progress * 100}%`} aria-hidden="true"></span>
				<div><span>{modeLabel($pomodoro.mode)}</span><strong>{clock}</strong></div>
			</div>
			<div class="lv-pomo-dock__actions">
				<button class="lv-btn lv-btn--primary" type="button" onclick={() => running ? pauseTimer() : startTimer()}>{controlLabel}</button>
				<button class="lv-btn" type="button" onclick={resetTimer}>Reiniciar</button>
				<a class="lv-text-link" href={href('focus')}>Configurações e estatísticas</a>
			</div>
			{#if $focusSync.error}
				<p class="lv-pomo-dock__sync-error" role="status">{$focusSync.error}</p>
				<button class="lv-btn" type="button" disabled={$focusSync.saving} onclick={() => void retryFocusSessions()}>{$focusSync.saving ? 'Salvando…' : 'Tentar salvar de novo'}</button>
			{:else if $focusSync.pending.length > 0}
				<p class="lv-pomo-dock__sync-note" role="status">{$focusSync.pending.length} sessão(ões) aguardando sincronização.</p>
			{/if}
		</div>
	{:else}
		<div class="lv-pomo-dock__compact">
			<button class="lv-pomo-dock__summary" type="button" aria-expanded="false" aria-label={`Abrir Pomodoro: ${modeLabel($pomodoro.mode)}, ${clock}`} onclick={() => (expanded = true)}>
				<span class="lv-pomo-dock__ring" style:--pomo-progress={`${progress * 100}%`} aria-hidden="true"></span>
				<span class="lv-pomo-dock__time">{clock}</span>
				<span class="lv-pomo-dock__mode">{$pomodoro.mode === 'focus' ? 'Foco' : 'Pausa'}</span>
			</button>
			<button class="lv-pomo-dock__quick" type="button" aria-label={controlLabel} title={controlLabel} onclick={() => running ? pauseTimer() : startTimer()}>
				{#if running}<span aria-hidden="true">Ⅱ</span>{:else}<span aria-hidden="true">▶</span>{/if}
			</button>
		</div>
	{/if}
</aside>
