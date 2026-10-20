<script lang="ts">
	import { onMount } from 'svelte';
	import { formatDuration } from './player';
	import { transport, type Pomodoro } from './transport';

	let pomodoroState = $state<Pomodoro>({ status: 'idle', durationSeconds: 1500, elapsedSeconds: 0, updatedAt: 0 });
	let busy = $state(false);
	let error = $state('');
	let open = $state(false);
	let ticker: ReturnType<typeof setInterval> | null = null;

	const remaining = $derived(Math.max(0, pomodoroState.durationSeconds - pomodoroState.elapsedSeconds));
	const statusLabel = $derived(pomodoroState.status === 'running' ? 'Em andamento' : pomodoroState.status === 'paused' ? 'Pausada' : 'Pronta');

	function stopTicker() {
		if (ticker) {
			clearInterval(ticker);
			ticker = null;
		}
	}

	function startTicker() {
		stopTicker();
		if (pomodoroState.status !== 'running') return;
		ticker = setInterval(() => {
			if (pomodoroState.status !== 'running') {
				stopTicker();
				return;
			}
			pomodoroState = { ...pomodoroState, elapsedSeconds: Math.min(pomodoroState.durationSeconds, pomodoroState.elapsedSeconds + 1) };
			if (pomodoroState.elapsedSeconds >= pomodoroState.durationSeconds) stopTicker();
		}, 1000);
	}

	async function load() {
		try {
			pomodoroState = await transport.pomodoro();
			error = '';
			startTicker();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível carregar o Pomodoro.';
			stopTicker();
		}
	}

	async function start() {
		if (busy) return;
		busy = true;
		error = '';
		try {
			pomodoroState = await transport.startPomodoro(pomodoroState.durationSeconds || 1500);
			startTicker();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível iniciar o Pomodoro.';
		} finally {
			busy = false;
		}
	}

	async function pause() {
		if (busy) return;
		busy = true;
		error = '';
		try {
			pomodoroState = await transport.pausePomodoro(pomodoroState.elapsedSeconds);
			stopTicker();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível pausar o Pomodoro.';
		} finally {
			busy = false;
		}
	}

	async function reset() {
		if (busy) return;
		busy = true;
		error = '';
		try {
			pomodoroState = await transport.resetPomodoro();
			stopTicker();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível reiniciar o Pomodoro.';
		} finally {
			busy = false;
		}
	}

	async function toggle() {
		open = !open;
		if (open) await load();
	}

	onMount(() => {
		void load();
		return stopTicker;
	});
</script>
<div class="pomo-wrap">
	<button class="pomo-chip" type="button" aria-expanded={open} aria-controls="pomodoro-panel" onclick={toggle}>
		Pomodoro <span>{formatDuration(remaining)}</span>
	</button>
	{#if open}
		<section id="pomodoro-panel" class="pomo-panel" aria-label="Temporizador Pomodoro">
			<div class="pomo-panel-head"><strong>Pomodoro</strong><button class="text-button" type="button" aria-label="Fechar Pomodoro" onclick={() => (open = false)}>×</button></div>
			<p class="pomo-status">{statusLabel}</p>
			<strong class="pomo-clock" aria-live="polite">{formatDuration(remaining)}</strong>
			<div class="pomo-actions">
				{#if pomodoroState.status === 'running'}
					<button class="secondary" type="button" onclick={pause} disabled={busy}>Pausar</button>
				{:else}
					<button class="primary" type="button" onclick={start} disabled={busy}>{pomodoroState.status === 'paused' ? 'Retomar' : 'Iniciar'}</button>
				{/if}
				{#if pomodoroState.status !== 'idle'}<button class="text-button" type="button" onclick={reset} disabled={busy}>Reiniciar</button>{/if}
			</div>
			{#if error}<p class="error" role="alert">{error}</p>{/if}
		</section>
	{/if}
</div>
