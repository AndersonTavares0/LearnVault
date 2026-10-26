<script lang="ts">
	import { onMount } from 'svelte';
	import {
		POMODORO_MODES,
		modeLabel,
		localDayKey,
		weekDayKeys,
		type PomodoroSettings
	} from './pomodoro';
	import {
		pomodoro,
		persistedFocusSummary,
		refreshFocusData,
		startTimer,
		pauseTimer,
		resetTimer,
		skipTimer,
		setMode,
		saveSettings
	} from './pomodoro-store';

	let settingsOpen = $state(false);
	let settingsDraft = $state<PomodoroSettings>({ ...$pomodoro.settings });
	const summary = $derived($persistedFocusSummary ?? {
		todayMinutes: 0,
		weekMinutes: 0,
		weekSessions: 0,
		totalMinutes: 0,
		totalSessions: 0,
		completionRate: 0,
		bestDay: null,
		days: weekDayKeys().map((day) => ({ day, minutes: 0, sessions: 0 }))
	});
	const today = localDayKey(new Date());
	let audio: AudioContext | null = null;

	const RADIUS = 92;
	const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

	onMount(() => { void refreshFocusData(); });

	$effect(() => {
		settingsDraft = { ...$pomodoro.settings };
	});

	function clock(totalSeconds: number): string {
		const seconds = Math.max(0, Math.ceil(totalSeconds));
		const minutes = Math.floor(seconds / 60);
		return `${String(minutes).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
	}

	function dayLabel(day: string): string {
		const labels = ['seg', 'ter', 'qua', 'qui', 'sex', 'sáb', 'dom'];
		const date = new Date(`${day}T12:00:00`);
		return Number.isNaN(date.getTime()) ? '—' : (labels[(date.getDay() + 6) % 7] ?? '—');
	}

	function currentProgress(): number {
		if (!$pomodoro.totalSeconds) return 0;
		return Math.min(1, Math.max(0, 1 - $pomodoro.remainingSeconds / $pomodoro.totalSeconds));
	}

	function cycleDots(): number[] {
		return Array.from({ length: $pomodoro.settings.longEvery }, (_, index) => index);
	}

	function cyclePosition(): number {
		const done = $pomodoro.cycle % $pomodoro.settings.longEvery;
		return $pomodoro.mode === 'long' ? $pomodoro.settings.longEvery : done;
	}

	function submitSettings(event: SubmitEvent) {
		event.preventDefault();
		saveSettings({ ...settingsDraft });
		settingsOpen = false;
	}

	function resetSettings() {
		settingsDraft = { ...$pomodoro.settings };
	}

	function audioContext(): AudioContext | null {
		try {
			audio ??= new AudioContext();
			if (audio.state === 'suspended') void audio.resume();
			return audio;
		} catch {
			return null;
		}
	}

	function playAlarm(settings: PomodoroSettings) {
		if (!settings.alarmEnabled) return;
		const context = audioContext();
		if (!context) return;
		const now = context.currentTime;
		for (let index = 0; index < settings.alarmRepeats; index += 1) {
			const start = now + index * 0.42;
			const oscillator = context.createOscillator();
			const gain = context.createGain();
			oscillator.type = settings.alarmSound;
			oscillator.frequency.setValueAtTime(660 + index * 70, start);
			gain.gain.setValueAtTime(0, start);
			gain.gain.linearRampToValueAtTime(settings.alarmVolume * 0.18, start + 0.015);
			gain.gain.exponentialRampToValueAtTime(0.0001, start + 0.22);
			oscillator.connect(gain).connect(context.destination);
			oscillator.start(start);
			oscillator.stop(start + 0.25);
		}
	}

	async function enableNotifications() {
		if (typeof Notification === 'undefined') return;
		try {
			await Notification.requestPermission();
		} catch {
			// Permission can be denied or unavailable; the in-app state still works.
		}
	}

	function testAlarm() {
		playAlarm(settingsDraft);
	}
</script>

<svelte:head><title>Pomodoro — LearnVault</title><meta name="description" content="Sessões de foco integradas à sua biblioteca." /></svelte:head>

<div class="lv-page lv-pomodoro">
	<header class="lv-page__head">
		<div>
			<p class="lv-kicker">Tempo de estudo</p>
			<h1>Pomodoro</h1>
			<p class="lv-lede">Sessões de foco com pausas no ritmo certo. O tempo concluído entra nas estatísticas de foco e no XP.</p>
		</div>
		<button class="lv-btn" type="button" aria-expanded={settingsOpen} onclick={() => (settingsOpen = !settingsOpen)}>
			{settingsOpen ? 'Fechar ajustes' : 'Ajustar timer'}
		</button>
	</header>

	{#if settingsOpen}
		<form class="lv-pomodoro__settings" onsubmit={submitSettings}>
			<div class="lv-pomodoro__settings-grid">
				<label>Foco (min)<input type="number" min="1" max="120" bind:value={settingsDraft.focusMinutes} /></label>
				<label>Pausa curta (min)<input type="number" min="1" max="60" bind:value={settingsDraft.shortMinutes} /></label>
				<label>Pausa longa (min)<input type="number" min="1" max="120" bind:value={settingsDraft.longMinutes} /></label>
				<label>Pausa longa a cada<input type="number" min="2" max="12" bind:value={settingsDraft.longEvery} /></label>
				<label class="lv-pomodoro__check"><input type="checkbox" bind:checked={settingsDraft.autoStartNext} />Iniciar o próximo bloco automaticamente</label>
				<label class="lv-pomodoro__check"><input type="checkbox" bind:checked={settingsDraft.alarmEnabled} />Tocar alarme ao terminar</label>
				<label>Som do alarme
					<select bind:value={settingsDraft.alarmSound}>
						<option value="sine">Suave</option>
						<option value="triangle">Claro</option>
						<option value="square">Marcante</option>
					</select>
				</label>
				<label>Volume<input type="range" min="0" max="1" step="0.05" bind:value={settingsDraft.alarmVolume} /></label>
				<label>Repetições<input type="number" min="1" max="5" bind:value={settingsDraft.alarmRepeats} /></label>
				<label class="lv-pomodoro__check"><input type="checkbox" bind:checked={settingsDraft.notificationsEnabled} />Notificações do sistema</label>
			</div>
			<div class="lv-pomodoro__settings-actions">
				<button class="lv-btn lv-btn--primary" type="submit">Salvar ajustes</button>
				<button class="lv-btn" type="button" onclick={testAlarm}>Testar alarme</button>
				<button class="lv-btn lv-btn--ghost" type="button" onclick={enableNotifications}>Permitir notificações</button>
				<button class="lv-btn lv-btn--ghost" type="button" onclick={resetSettings}>Descartar</button>
			</div>
		</form>
	{/if}

	<div class="lv-pomodoro__layout">
		<section class="lv-pomodoro__timer" aria-labelledby="timer-heading">
			<h2 id="timer-heading" class="visually-hidden">Timer Pomodoro</h2>
			<div class="lv-pomodoro__modes" role="group" aria-label="Modo do timer">
				{#each POMODORO_MODES as item (item)}
					<button type="button" class:is-active={$pomodoro.mode === item} aria-pressed={$pomodoro.mode === item} onclick={() => setMode(item)}>{modeLabel(item)}</button>
				{/each}
			</div>
			<div class="lv-pomodoro__dial" style:--pomodoro-progress={`${currentProgress()}`}>
				<svg viewBox="0 0 220 220" aria-hidden="true">
					<circle class="lv-pomodoro__track" cx="110" cy="110" r={RADIUS} />
					<circle class="lv-pomodoro__arc" cx="110" cy="110" r={RADIUS} stroke-dasharray={CIRCUMFERENCE} stroke-dashoffset={CIRCUMFERENCE * (1 - currentProgress())} />
				</svg>
				<div class="lv-pomodoro__readout">
					<span class="lv-pomodoro__mode">{modeLabel($pomodoro.mode)}</span>
					<strong>{clock($pomodoro.remainingSeconds)}</strong>
					<span class="lv-pomodoro__cycle">Ciclo {$pomodoro.cycle + 1}</span>
				</div>
			</div>
			<div class="lv-pomodoro__dots" aria-label="Progresso dos ciclos">
				{#each cycleDots() as dot (dot)}
					<span class:is-done={dot < cyclePosition()}></span>
				{/each}
			</div>
			<div class="lv-pomodoro__actions">
				{#if $pomodoro.status === 'running'}
					<button class="lv-btn lv-btn--primary" type="button" onclick={pauseTimer}>Pausar</button>
				{:else}
					<button class="lv-btn lv-btn--primary" type="button" onclick={startTimer}>{$pomodoro.status === 'paused' ? 'Retomar' : 'Começar'}</button>
				{/if}
				<button class="lv-btn" type="button" onclick={resetTimer}>Reiniciar</button>
				<button class="lv-btn lv-btn--ghost" type="button" onclick={skipTimer}>Pular</button>
			</div>
			<p class="lv-pomodoro__hint" role="status" aria-live="polite">
				{#if $pomodoro.status === 'running'}Concentre-se em uma coisa por vez.
				{:else if $pomodoro.status === 'paused'}Timer pausado.
				{:else if $pomodoro.lastCompleted}Pronto. {$pomodoro.lastCompleted.mode === 'focus' ? 'Faça uma pausa.' : 'Volte ao foco.'}
				{:else}Escolha um modo e comece quando estiver pronto.{/if}
			</p>
		</section>

		<aside class="lv-pomodoro__side">
			<section class="lv-panel" aria-labelledby="focus-stats-heading">
				<div class="lv-panel__head"><h2 id="focus-stats-heading">Seu foco</h2><span class="lv-count">semana atual</span></div>
				<div class="lv-pomodoro__stats">
					<div><strong>{summary.todayMinutes}</strong><span>min hoje</span></div>
					<div><strong>{summary.weekMinutes}</strong><span>min na semana</span></div>
					<div><strong>{summary.weekSessions}</strong><span>sessões</span></div>
					<div><strong>{summary.completionRate}%</strong><span>concluídas</span></div>
				</div>
				<div class="lv-pomodoro__week" aria-label="Minutos de foco por dia">
					{#each summary.days as day (day.day)}
						<div class="lv-pomodoro__week-day" class:is-today={day.day === today} class:is-active={day.minutes > 0} title={`${day.minutes} min em ${day.day}`}>
						<span class="lv-pomodoro__week-bar"><span style:height={`${Math.min(100, (day.minutes / Math.max(1, summary.days.find((entry) => entry.day === summary.bestDay)?.minutes ?? 1)) * 100)}%`}></span></span>
							<small>{dayLabel(day.day)}</small>
						</div>
					{/each}
				</div>
				<p class="lv-pomodoro__best">
					{#if summary.bestDay}Melhor dia: {summary.days.find((day) => day.day === summary.bestDay)?.minutes ?? 0} min · {dayLabel(summary.bestDay)}{:else}Sem sessões nesta semana ainda.{/if}
				</p>
			</section>

			<section class="lv-panel lv-pomodoro__note">
				<div class="lv-panel__head"><h2>Como funciona</h2></div>
				<p>Foco de {$pomodoro.settings.focusMinutes} min, pausa de {$pomodoro.settings.shortMinutes} min e pausa longa de {$pomodoro.settings.longMinutes} min a cada {$pomodoro.settings.longEvery} ciclos. Sessões concluídas entram nas estatísticas de foco e no XP.</p>
			</section>
		</aside>
	</div>
</div>
