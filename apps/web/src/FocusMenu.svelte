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
	let seedMenuOpen = $state(false);
	let trigger = $state<HTMLButtonElement | null>(null);
	let popover = $state<HTMLDivElement | null>(null);
	let seedMenuRef = $state<HTMLDivElement | null>(null);
	let seedBtnRef = $state<HTMLButtonElement | null>(null);

	const running = $derived($pomodoro.status === 'running');
	const paused = $derived($pomodoro.status === 'paused');
	const clock = $derived(formatClock($pomodoro.remainingSeconds));
	const status = $derived(running ? 'Em andamento' : paused ? 'Pausado' : 'Pronto');
	const label = $derived(running || paused ? clock : 'Foco');
	const announcement = $derived(`${status}. ${modeLabel($pomodoro.mode)}, ${clock}.`);
	const currentSpecies = $derived(getSpeciesById($nursery.activeSeed));

	function close() {
		open = false;
		seedMenuOpen = false;
		confirmAbandon = false;
		trigger?.focus();
	}

	function closeSeedMenu() {
		seedMenuOpen = false;
		seedBtnRef?.focus();
	}

	function selectSeed(id: BotanicalSpeciesId) {
		setActiveSeed(id);
		closeSeedMenu();
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
		const target = event.target as Node;
		if (seedMenuOpen && !seedMenuRef?.contains(target)) {
			seedMenuOpen = false;
		}
		if (!open) return;
		if (popover?.contains(target) || trigger?.contains(target)) return;
		open = false;
		seedMenuOpen = false;
	}

	function onKeydown(event: KeyboardEvent) {
		if (open && event.key === 'Escape') {
			event.preventDefault();
			if (seedMenuOpen) {
				closeSeedMenu();
			} else {
				close();
			}
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
						<div class="lv-focus-menu__seed-dropdown" bind:this={seedMenuRef}>
							<button
								bind:this={seedBtnRef}
								type="button"
								class="lv-focus-menu__seed-btn"
								aria-haspopup="listbox"
								aria-expanded={seedMenuOpen}
								aria-controls="lv-seed-listbox"
								aria-label="Espécie botânica a cultivar, selecionado: {currentSpecies.name}"
								onclick={() => (seedMenuOpen = !seedMenuOpen)}
							>
								<span class="lv-focus-menu__seed-name">{currentSpecies.name}</span>
								<svg class="lv-focus-menu__seed-arrow" class:is-open={seedMenuOpen} viewBox="0 0 20 20" fill="currentColor" width="14" height="14" aria-hidden="true">
									<path fill-rule="evenodd" d="M5.23 7.21a.75.75 0 011.06.02L10 11.168l3.71-3.938a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z" clip-rule="evenodd" />
								</svg>
							</button>
							{#if seedMenuOpen}
								<ul
									id="lv-seed-listbox"
									class="lv-focus-menu__seed-list"
									role="listbox"
									tabindex="-1"
									aria-label="Espécies botânicas desbloqueadas"
								>
									{#each $nursery.unlockedSpecies as id (id)}
										{@const sp = getSpeciesById(id)}
										{@const isSelected = id === $nursery.activeSeed}
										<li
											role="option"
											aria-selected={isSelected}
											tabindex="0"
											class="lv-focus-menu__seed-item"
											class:is-active={isSelected}
											onclick={() => selectSeed(id)}
											onkeydown={(e) => {
												if (e.key === 'Enter' || e.key === ' ') {
													e.preventDefault();
													selectSeed(id);
												} else if (e.key === 'ArrowDown') {
													e.preventDefault();
													const next = (e.currentTarget as HTMLElement).nextElementSibling as HTMLElement | null;
													next?.focus();
												} else if (e.key === 'ArrowUp') {
													e.preventDefault();
													const prev = (e.currentTarget as HTMLElement).previousElementSibling as HTMLElement | null;
													prev?.focus();
												}
											}}
										>
											<span class="lv-focus-menu__seed-item-name">{sp.name}</span>
											{#if isSelected}
												<svg class="lv-focus-menu__seed-item-check" viewBox="0 0 20 20" fill="currentColor" width="14" height="14" aria-hidden="true">
													<path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" />
												</svg>
											{/if}
										</li>
									{/each}
								</ul>
							{/if}
						</div>
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
		position: relative;
	}

	.lv-focus-menu__seed-label {
		color: var(--muted);
		font-weight: 600;
		flex-shrink: 0;
	}

	.lv-focus-menu__seed-dropdown {
		position: relative;
		flex: 1;
		min-width: 0;
	}

	.lv-focus-menu__seed-btn {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.35rem;
		width: 100%;
		background: transparent;
		border: none;
		color: var(--ink);
		font-weight: 600;
		font-size: 0.8125rem;
		cursor: pointer;
		padding: 0.15rem 0.25rem;
		border-radius: 4px;
		text-align: left;
	}

	.lv-focus-menu__seed-btn:focus-visible {
		outline: 2px solid var(--focus);
		outline-offset: 1px;
	}

	.lv-focus-menu__seed-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.lv-focus-menu__seed-arrow {
		flex-shrink: 0;
		color: var(--muted);
		transition: transform 0.15s ease;
	}

	.lv-focus-menu__seed-arrow.is-open {
		transform: rotate(180deg);
	}

	.lv-focus-menu__seed-list {
		position: absolute;
		top: calc(100% + 6px);
		left: 0;
		right: 0;
		min-width: 170px;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 8px;
		box-shadow: 0 10px 25px -4px rgba(0, 0, 0, 0.45);
		padding: 0.25rem;
		margin: 0;
		list-style: none;
		z-index: 60;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.lv-focus-menu__seed-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.45rem 0.6rem;
		border-radius: 6px;
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--ink);
		cursor: pointer;
		user-select: none;
		outline: none;
		transition: background 0.15s ease, color 0.15s ease;
	}

	.lv-focus-menu__seed-item:hover,
	.lv-focus-menu__seed-item:focus-visible {
		background: var(--hover);
		color: var(--ink);
	}

	.lv-focus-menu__seed-item.is-active {
		background: color-mix(in srgb, var(--accent) 15%, var(--surface));
		color: var(--accent);
		font-weight: 700;
	}

	.lv-focus-menu__seed-item-check {
		flex-shrink: 0;
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
