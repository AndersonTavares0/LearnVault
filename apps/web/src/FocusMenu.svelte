<script lang="ts">
	import { onMount } from 'svelte';
	import { formatClock, modeLabel, POMODORO_MODES } from './pomodoro';
	import { focusSync, pauseTimer, pomodoro, resetTimer, retryFocusSessions, setMode, startTimer } from './pomodoro-store';
	import { href } from './router';

	let open = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);
	let popover = $state<HTMLDivElement | null>(null);

	const running = $derived($pomodoro.status === 'running');
	const paused = $derived($pomodoro.status === 'paused');
	const clock = $derived(formatClock($pomodoro.remainingSeconds));
	const status = $derived(running ? 'Em andamento' : paused ? 'Pausado' : 'Pronto');
	const label = $derived(running || paused ? clock : 'Foco');
	const announcement = $derived(`${status}. ${modeLabel($pomodoro.mode)}, ${clock}.`);

	function close() {
		open = false;
		trigger?.focus();
	}

	function toggle() {
		if (open) close();
		else open = true;
	}

	function toggleTimer() {
		if (running) pauseTimer();
		else startTimer();
	}

	function onDocumentPointer(event: PointerEvent) {
		if (!open) return;
		const target = event.target as Node;
		if (popover?.contains(target) || trigger?.contains(target)) return;
		open = false;
	}

	function onKeydown(event: KeyboardEvent) {
		if (open && event.key === 'Escape') {
			event.preventDefault();
			close();
		}
	}

	onMount(() => {
		document.addEventListener('pointerdown', onDocumentPointer);
		document.addEventListener('keydown', onKeydown);
		return () => {
			document.removeEventListener('pointerdown', onDocumentPointer);
			document.removeEventListener('keydown', onKeydown);
		};
	});
</script>

<div class="lv-focus-menu">
	<button
		bind:this={trigger}
		class="lv-focus-menu__trigger"
		class:is-running={running}
		class:is-paused={paused}
		type="button"
		aria-expanded={open}
		aria-controls="focus-menu-popover"
		aria-label={`Pomodoro: ${status.toLowerCase()}, ${modeLabel($pomodoro.mode)}, ${clock}`}
		onclick={toggle}
	>
		<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="13" r="7" fill="none" stroke="currentColor" stroke-width="1.7"/><path d="M12 9.5V13l2.4 1.4M9.5 2.5h5M12 2.5v3" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"/></svg>
		<span>{label}</span>
	</button>

	{#if open}
		<div class="lv-focus-menu__popover" id="focus-menu-popover" bind:this={popover} role="dialog" aria-label="Pomodoro">
			<div class="lv-focus-menu__head">
				<div>
					<strong>Foco</strong>
					<span>{status} · Ciclo {$pomodoro.cycle + 1}</span>
				</div>
				<button type="button" class="lv-focus-menu__close" aria-label="Fechar Pomodoro" onclick={close}>×</button>
			</div>
			<div class="lv-focus-menu__modes" role="group" aria-label="Modo do timer">
				{#each POMODORO_MODES as mode (mode)}
					<button class="lvui-btn lvui-btn-sm" type="button" aria-pressed={$pomodoro.mode === mode} class:is-active={$pomodoro.mode === mode} onclick={() => setMode(mode)}>{modeLabel(mode)}</button>
				{/each}
			</div>
			<div class="lv-focus-menu__clock" aria-hidden="true">{clock}</div>
			<p class="visually-hidden" role="status" aria-live="polite">{announcement}</p>
			<div class="lv-focus-menu__actions">
				<button class="lvui-btn lvui-btn-primary" type="button" onclick={toggleTimer}>{running ? 'Pausar' : paused ? 'Retomar' : 'Iniciar foco'}</button>
				<button class="lvui-btn" type="button" onclick={resetTimer}>Reiniciar</button>
			</div>
			{#if $focusSync.error}
				<p class="lv-focus-menu__note" role="status">{$focusSync.error} <button class="lv-text-link" type="button" disabled={$focusSync.saving} onclick={() => void retryFocusSessions()}>Tentar de novo</button></p>
			{:else if $focusSync.pending.length > 0}
				<p class="lv-focus-menu__note" role="status">{$focusSync.pending.length} sessão(ões) aguardando sincronização.</p>
			{/if}
			<a class="lv-text-link lv-focus-menu__settings" href={href('settings')} onclick={() => (open = false)}>Ajustar tempos</a>
		</div>
	{/if}
</div>
