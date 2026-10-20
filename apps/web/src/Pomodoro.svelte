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
			<div class="pomo-panel-head">
				<strong>Pomodoro</strong>
				<button class="close-button" type="button" aria-label="Fechar Pomodoro" onclick={() => (open = false)}>×</button>
			</div>
			<div class="pomo-clock">
				<strong class="pomo-clock-digits" aria-live="polite">{formatDuration(remaining)}</strong>
				<small>{statusLabel}</small>
			</div>
			<div class="pomo-actions">
				{#if pomodoroState.status === 'running'}
					<button class="primary" type="button" onclick={pause} disabled={busy}>Pausar</button>
				{:else}
					<button class="primary" type="button" onclick={start} disabled={busy}>{pomodoroState.status === 'paused' ? 'Retomar' : 'Iniciar'}</button>
				{/if}
				{#if pomodoroState.status !== 'idle'}<button type="button" onclick={reset} disabled={busy}>Reiniciar</button>{/if}
			</div>
			{#if error}<p class="error" role="alert">{error}</p>{/if}
		</section>
	{/if}
</div>

<style>
	.pomo-wrap { position: relative; min-width: 0; }
	.pomo-chip { min-height: 44px; padding: 8px 12px; border: 1px solid var(--on-nav); border-radius: 8px; background: var(--nav); color: var(--on-nav); font-size: 14px; font-weight: 600; white-space: nowrap; }
	.pomo-chip span { font-variant-numeric: tabular-nums; margin-left: 8px; }
	.pomo-chip:hover { outline: 1px solid currentColor; outline-offset: 1px; }
	.pomo-chip:focus-visible { outline: 3px solid var(--on-nav); outline-offset: 2px; }
	.pomo-panel { position: absolute; z-index: 20; top: calc(100% + 8px); right: 0; width: min(330px, calc(100vw - 40px)); border: 1px solid var(--line-strong); border-radius: 16px; background: var(--surface); color: var(--ink); box-shadow: var(--shadow); }
	.pomo-panel-head { display: flex; align-items: center; justify-content: space-between; padding: 8px 16px; border-bottom: 1px solid var(--line); }
	.pomo-panel-head strong { font-size: 16px; }
	.close-button { width: 44px; height: 44px; border: 0; background: transparent; color: var(--ink); font-size: 24px; }
	.pomo-clock { text-align: center; padding: 24px 16px; }
	.pomo-clock-digits { display: block; color: var(--ink); font-size: 48px; font-weight: 700; line-height: 1; font-variant-numeric: tabular-nums; }
	.pomo-clock small { display: block; margin-top: 12px; color: var(--muted); font-size: 14px; }
	.pomo-actions { display: flex; justify-content: center; gap: 8px; padding: 0 16px 24px; }
	.pomo-actions button { min-height: 44px; padding: 8px 16px; border: 1px solid var(--line-strong); border-radius: 8px; background: var(--surface); color: var(--ink); font-size: 14px; font-weight: 600; }
	.pomo-actions button.primary { border-color: var(--accent); background: var(--accent); color: var(--on-accent); }
	.pomo-actions button:hover:not(:disabled) { outline: 1px solid currentColor; outline-offset: 1px; }
	.pomo-actions button:disabled { cursor: wait; opacity: .55; }
	.error { margin: 0 16px 16px; color: var(--danger); font-size: 14px; }
	@media (max-width: 700px) {
		.pomo-chip { padding: 8px; font-size: 13px; }
		.pomo-chip span { margin-left: 4px; }
		.pomo-panel { position: fixed; top: auto; right: max(20px, env(safe-area-inset-right)); bottom: max(20px, env(safe-area-inset-bottom)); }
	}
</style>
