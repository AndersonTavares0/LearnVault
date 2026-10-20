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
	.pomo-wrap {
		position: relative;
	}

	.pomo-chip {
		border: 1px solid #587078;
		background: #20363e;
		color: #e8f2f2;
		border-radius: 6px;
		padding: 7px 10px;
		font-size: 12px;
		font-weight: 650;
		white-space: nowrap;
	}

	.pomo-chip span {
		color: #8ed1c4;
		font-variant-numeric: tabular-nums;
		margin-left: 5px;
	}

	.pomo-panel {
		position: absolute;
		z-index: 20;
		top: calc(100% + 10px);
		right: 0;
		width: min(330px, calc(100vw - 28px));
		overflow: hidden;
		border: 1px solid var(--line, #d9e1e6);
		border-radius: 9px;
		background: var(--surface, #fff);
		color: var(--ink, #1b2832);
		box-shadow: 0 10px 28px #14232b26;
	}

	.pomo-panel-head {
		display: flex;
		align-items: center;
		padding: 14px 16px;
		border-bottom: 1px solid var(--line, #d9e1e6);
	}

	.pomo-panel-head strong {
		font-size: 14px;
	}

	.close-button {
		margin-left: auto;
		border: 0;
		background: transparent;
		color: var(--muted, #64737e);
		padding: 7px;
		font-size: 18px;
		line-height: 1;
	}

	.pomo-clock {
		display: block;
		text-align: center;
		padding: 19px 16px 9px;
	}

	.pomo-clock-digits {
		display: block;
		color: var(--ink, #1b2832);
		font-size: 48px;
		font-weight: 750;
		line-height: 1;
		letter-spacing: -0.05em;
		font-variant-numeric: tabular-nums;
	}

	.pomo-clock small {
		display: block;
		margin-top: 8px;
		color: var(--muted, #64737e);
	}

	.pomo-actions {
		display: flex;
		justify-content: center;
		gap: 7px;
		padding: 0 14px 15px;
	}

	.pomo-actions button {
		border: 1px solid var(--line, #d9e1e6);
		border-radius: 6px;
		background: var(--surface, #fff);
		color: var(--ink, #344750);
		padding: 9px 12px;
		font-size: 12px;
		font-weight: 650;
	}

	.pomo-actions button.primary {
		border-color: var(--accent, #276f73);
		background: var(--accent, #276f73);
		color: #fff;
	}
:global(:root[data-theme="dark"]) .pomo-actions button.primary {
	color: #14232b;
}

	.pomo-actions button:disabled {
		cursor: wait;
		opacity: 0.65;
	}

	.error {
		margin: 0 14px 14px;
		padding: 10px 11px;
		border-radius: 6px;
		background: var(--amber-soft, #fbf0e4);
		color: var(--amber, #8b531f);
		font-size: 12px;
	}

	.pomo-wrap button:focus-visible {
		outline: 2px solid var(--focus, #276f73);
		outline-offset: 2px;
	}

	@media (max-width: 700px) {
		.pomo-chip {
			padding: 6px 7px;
		}

		.pomo-panel {
			position: fixed;
			top: auto;
			right: 14px;
			bottom: 14px;
		}
	}
</style>
