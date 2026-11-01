<script lang="ts">
	import { onMount } from 'svelte';
	import { formatClock, modeLabel, POMODORO_MODES } from './pomodoro';
	import {
		abandonTimer,
		focusSync,
		pauseTimer,
		pomodoro,
		resetTimer,
		retryFocusSessions,
		setMode,
		startTimer
	} from './pomodoro-store';
	import { href } from './router';
	import { getSpeciesById, type BotanicalSpeciesId } from './forest-catalog';
	import { nursery, setActiveSeed } from './nursery-store';
	import ForestGrowthStage from './ForestGrowthStage.svelte';

	let open = $state(false);
	let confirmAbandon = $state(false);
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
		confirmAbandon = false;
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

	function handleAbandon() {
		abandonTimer();
		confirmAbandon = false;
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

			{#if $pomodoro.mode === 'focus'}
				{#if running || paused}
					<ForestGrowthStage
						compact={true}
						speciesId={$nursery.activeSeed}
						remainingSeconds={$pomodoro.remainingSeconds}
						totalSeconds={$pomodoro.totalSeconds}
						{clock}
					/>
				{:else}
					<div class="lv-focus-menu__seed-strip">
						<span class="lv-focus-menu__seed-label">Plantar:</span>
						<select
							class="lv-focus-menu__seed-select"
							value={$nursery.activeSeed}
							onchange={(e) => setActiveSeed(e.currentTarget.value as BotanicalSpeciesId)}
							aria-label="Espécie botânica a cultivar"
						>
							{#each $nursery.unlockedSpecies as id (id)}
								{@const sp = getSpeciesById(id)}
								<option value={id}>{sp.name}</option>
							{/each}
						</select>
					</div>
				{/if}
			{/if}

			<div class="lv-focus-menu__actions">
				<button class="lvui-btn lvui-btn-primary" type="button" onclick={toggleTimer}>{running ? 'Pausar' : paused ? 'Retomar' : 'Iniciar foco'}</button>
				<button class="lvui-btn" type="button" onclick={resetTimer}>Reiniciar</button>
				{#if $pomodoro.mode === 'focus' && (running || paused) && !confirmAbandon}
					<button class="lvui-btn lvui-btn-outline lvui-btn-danger" type="button" onclick={() => (confirmAbandon = true)}>Abandonar</button>
				{/if}
			</div>

			{#if confirmAbandon}
				<div class="lv-focus-menu__confirm-abandon" role="alert">
					<p>Deseja desistir desta sessão de foco? A árvore cultivada nela secará.</p>
					<div class="lv-focus-menu__confirm-actions">
						<button class="lvui-btn lvui-btn-sm lvui-btn-danger" type="button" onclick={handleAbandon}>Desistir e secar árvore</button>
						<button class="lvui-btn lvui-btn-sm" type="button" onclick={() => (confirmAbandon = false)}>Continuar focando</button>
					</div>
				</div>
			{/if}
			{#if $focusSync.error}
				<p class="lv-focus-menu__note" role="status">{$focusSync.error} <button class="lv-text-link" type="button" disabled={$focusSync.saving} onclick={() => void retryFocusSessions()}>Tentar de novo</button></p>
			{:else if $focusSync.pending.length > 0}
				<p class="lv-focus-menu__note" role="status">{$focusSync.pending.length} sessão(ões) aguardando sincronização.</p>
			{/if}
			<div class="lv-focus-menu__footer-links">
				<a class="lv-text-link" href={href('statistics')} onclick={() => (open = false)}>Ver Bosque e Viveiro</a>
				<a class="lv-text-link lv-focus-menu__settings" href={href('settings')} onclick={() => (open = false)}>Ajustar tempos</a>
			</div>
		</div>
	{/if}
</div>

<style>
	.lv-focus-menu__seed-strip {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		background: color-mix(in srgb, var(--surface) 90%, transparent);
		border: 1px solid var(--line);
		border-radius: 8px;
		padding: 0.35rem 0.65rem;
		font-size: 0.8125rem;
	}

	.lv-focus-menu__seed-label {
		color: var(--muted);
		font-weight: 600;
	}

	.lv-focus-menu__seed-select {
		background: transparent;
		border: none;
		color: var(--text);
		font-weight: 600;
		font-size: 0.8125rem;
		cursor: pointer;
		flex: 1;
		outline: none;
	}

	.lv-focus-menu__footer-links {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding-top: 0.5rem;
		border-top: 1px solid var(--line);
		font-size: 0.75rem;
	}
</style>
