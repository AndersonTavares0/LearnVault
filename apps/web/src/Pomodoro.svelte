<script lang="ts">
	import { onMount } from 'svelte';
	import { transport } from './transport';
	import { formatDuration } from './player';

	let status: 'idle' | 'running' | 'paused' = 'idle';
	let durationSeconds = 1500;
	let elapsedSeconds = 0;
	let anchor = 0;
	let now = Date.now();
	let error = '';
	let busy = false;
	let completionSaved = false;
	$: shownElapsed = status === 'running' ? Math.min(durationSeconds, elapsedSeconds + Math.floor((now - anchor) / 1000)) : elapsedSeconds;
	$: remaining = Math.max(0, durationSeconds - shownElapsed);

	function apply(state: Awaited<ReturnType<typeof transport.pomodoro>>) {
		status = state.status;
		completionSaved = state.status !== 'running' || state.elapsedSeconds >= state.durationSeconds;
		durationSeconds = state.durationSeconds;
		elapsedSeconds = state.elapsedSeconds;
		now = Date.now();
		anchor = now;
		if (status === 'running') elapsedSeconds = Math.min(durationSeconds, elapsedSeconds + Math.max(0, Math.floor(now / 1000) - state.updatedAt));
	}

	async function run(action: () => Promise<Awaited<ReturnType<typeof transport.pomodoro>>>) {
		busy = true;
		error = '';
		try { apply(await action()); } catch (cause) { error = cause instanceof Error ? cause.message : 'Pomodoro indisponível.'; }
		finally { busy = false; }
	}

	onMount(() => {
		void run(() => transport.pomodoro());
		const timer = window.setInterval(() => {
			now = Date.now();
			if (status === 'running' && remaining === 0 && !busy && !completionSaved) {
				completionSaved = true;
				void run(() => transport.pausePomodoro(durationSeconds));
			}
		}, 1000);
		return () => window.clearInterval(timer);
	});
</script>

<details class="pomodoro" aria-label="Temporizador Pomodoro">
	<summary><span class="pomodoro-dot" aria-hidden="true"></span><strong>Pomodoro</strong><output aria-live="off" aria-label={`${formatDuration(remaining)} restantes`}>{formatDuration(remaining)}</output></summary>
	<div class="pomodoro-popover">
		<div class="pomodoro-modes" aria-label="Modo atual"><span class="active">Foco</span></div>
		<div class="pomodoro-time" aria-hidden="true">{formatDuration(remaining)}</div>
		<div class="pomodoro-progress" role="progressbar" aria-label="Progresso do Pomodoro" aria-valuemin="0" aria-valuemax={durationSeconds} aria-valuenow={shownElapsed}><span style:width={`${durationSeconds ? (shownElapsed / durationSeconds) * 100 : 0}%`}></span></div>
		<div class="pomodoro-actions">
			{#if status === 'running'}<button type="button" onclick={() => run(() => transport.pausePomodoro(shownElapsed))} disabled={busy}>Pausar</button>
			{:else}<button type="button" onclick={() => run(() => transport.startPomodoro(durationSeconds))} disabled={busy}>{status === 'paused' ? 'Retomar' : 'Iniciar'}</button>{/if}
			<button type="button" class="text-button" onclick={() => run(() => transport.resetPomodoro())} disabled={busy}>Reiniciar</button>
		</div>
		<p class="pomodoro-cycle">Ciclo de foco · estado {status === 'running' ? 'em andamento' : status === 'paused' ? 'pausado' : 'pronto'}</p>
		{#if error}<span role="status">{error}</span>{/if}
	</div>
</details>
