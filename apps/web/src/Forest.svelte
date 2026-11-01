<script lang="ts">
	import { onMount } from 'svelte';
	import {
		buildForest,
		calculateForestStats,
		filterForest,
		type ForestFilter,
		type ForestTree,
		type TreeStage
	} from './forest';
	import ForestGuide from './ForestGuide.svelte';
	import ForestTreeSvg from './ForestTreeSvg.svelte';
	import { formatClock, readFocusSessions, type FocusSession } from './pomodoro';
	import { focusSync, pomodoro, setMode, startTimer } from './pomodoro-store';
	import { transport } from './transport';

	let sessionsLoaded = $state<ForestTree[]>([]);
	let loading = $state(true);
	let filter = $state<ForestFilter>('week');
	let selectedTree = $state<ForestTree | null>(null);

	const visibleTrees = $derived(filterForest(sessionsLoaded, filter));
	const stats = $derived(calculateForestStats(visibleTrees));

	const isRunning = $derived($pomodoro.status === 'running');
	const clock = $derived(formatClock($pomodoro.remainingSeconds));

	function storage(): Storage | null {
		try {
			return window.localStorage;
		} catch {
			return null;
		}
	}

	async function loadForestData() {
		loading = true;
		let remote: FocusSession[] = [];
		try {
			const fetched = await transport.listFocusSessions(500);
			if (fetched && fetched.length > 0) {
				remote = fetched;
			}
		} catch {
			// Fallback para cache local / offline
		}

		const local = readFocusSessions(storage());
		const pending = $focusSync.pending;
		const sessionMap = new Map<string, FocusSession>();

		for (const s of local) sessionMap.set(s.id, s);
		for (const s of remote) sessionMap.set(s.id, s);
		for (const s of pending) {
			sessionMap.set(s.id, {
				id: s.id,
				kind: s.kind,
				plannedSeconds: s.plannedSeconds,
				actualSeconds: s.actualSeconds,
				completed: s.completed,
				startedAt: s.startedAt,
				endedAt: s.endedAt
			});
		}

		sessionsLoaded = buildForest(Array.from(sessionMap.values()));
		loading = false;
	}

	function stageDescription(stage: TreeStage): string {
		switch (stage) {
			case 'sapling':
				return 'Ciclo rápido de foco concluído (até 20 minutos).';
			case 'oak':
				return 'Ciclo padrão de foco concluído (21 a 40 minutos).';
			case 'pine':
				return 'Ciclo estendido de foco concluído (41 a 60 minutos).';
			case 'ancient':
				return 'Ciclo de foco profundo e imersão (+60 minutos).';
			case 'withered':
				return 'Ciclo interrompido ou abandonado antes do término.';
		}
	}

	function formatDateTime(timestamp: number): string {
		try {
			return new Date(timestamp * 1000).toLocaleString('pt-BR', {
				day: '2-digit',
				month: 'short',
				year: 'numeric',
				hour: '2-digit',
				minute: '2-digit'
			});
		} catch {
			return '';
		}
	}

	function setFilter(next: ForestFilter) {
		filter = next;
		if (selectedTree && !filterForest(sessionsLoaded, next).some((t) => t.sessionId === selectedTree?.sessionId)) {
			selectedTree = null;
		}
	}

	function handleStartFocus() {
		if ($pomodoro.mode !== 'focus') {
			setMode('focus');
		}
		startTimer();
	}

	function handleWindowKeydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && selectedTree) {
			selectedTree = null;
		}
	}

	$effect(() => {
		if ($pomodoro.lastCompleted?.mode === 'focus') {
			void loadForestData();
		}
	});

	onMount(() => {
		void loadForestData();
	});
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<section class="lv-panel lv-forest" aria-labelledby="forest-heading">
	<div class="lv-panel__head">
		<div class="lv-forest__head-title">
			<h2 id="forest-heading">Floresta de Estudos</h2>
			<span class="lv-count" aria-label="{visibleTrees.length} árvores no período">
				{visibleTrees.length} {visibleTrees.length === 1 ? 'árvore' : 'árvores'}
			</span>
		</div>

		<!-- Segmented Control de Período -->
		<div class="lv-forest__filters" role="group" aria-label="Período da floresta">
			<button
				type="button"
				class="lv-forest__filter-btn"
				class:is-active={filter === 'today'}
				aria-pressed={filter === 'today'}
				onclick={() => setFilter('today')}
			>
				Hoje
			</button>
			<button
				type="button"
				class="lv-forest__filter-btn"
				class:is-active={filter === 'week'}
				aria-pressed={filter === 'week'}
				onclick={() => setFilter('week')}
			>
				Esta semana
			</button>
			<button
				type="button"
				class="lv-forest__filter-btn"
				class:is-active={filter === 'month'}
				aria-pressed={filter === 'month'}
				onclick={() => setFilter('month')}
			>
				Este mês
			</button>
			<button
				type="button"
				class="lv-forest__filter-btn"
				class:is-active={filter === 'all'}
				aria-pressed={filter === 'all'}
				onclick={() => setFilter('all')}
			>
				Tudo
			</button>
		</div>
	</div>

	<!-- Métricas editoriais da Floresta (Atelier design) -->
	<div class="lv-forest__stats">
		<div>
			<strong class="is-healthy">{stats.healthyTrees}</strong>
			<span>Árvores sadias</span>
		</div>
		<div>
			<strong class="is-withered">{stats.witheredTrees}</strong>
			<span>Árvores secas</span>
		</div>
		<div>
			<strong>{stats.successRate}%</strong>
			<span>Taxa de florescimento</span>
		</div>
		<div>
			<strong class="is-time">{stats.totalMinutes} min</strong>
			<span>Tempo dedicado</span>
		</div>
	</div>

	<!-- Bosque / Visualizador de Árvores -->
	<div class="lv-forest__canvas" role="region" aria-label="Visualização do bosque">
		{#if loading}
			<p class="lv-forest__loading" aria-live="polite">Cultivando sua floresta…</p>
		{:else if visibleTrees.length === 0}
			<div class="lv-forest__empty">
				<div class="lv-forest__empty-icon" aria-hidden="true">
					<svg viewBox="0 0 48 48" fill="none" width="48" height="48" stroke="currentColor">
						<circle cx="24" cy="24" r="20" stroke-width="2" opacity="0.15" />
						<path
							d="M24 38V22M24 22c0-8 6-12 12-12-2 6-4 12-12 12zM24 26c0-6-5-9-10-9 1.5 4.5 3 9 10 9z"
							stroke-width="2.5"
							stroke-linecap="round"
							stroke-linejoin="round"
						/>
					</svg>
				</div>
				<p class="lv-forest__empty-title">Nenhuma árvore cultivada neste período</p>
				<span class="lv-forest__empty-desc">
					Cada ciclo de foco concluído floresce uma nova árvore. Inicie uma sessão de foco no Pomodoro para plantar sua primeira muda!
				</span>
				{#if !isRunning}
					<button class="lv-btn lv-btn--primary lv-forest__cta-btn" type="button" onclick={handleStartFocus}>
						<svg viewBox="0 0 24 24" aria-hidden="true" width="16" height="16">
							<path d="M8 5v14l11-7z" fill="currentColor" />
						</svg>
						Iniciar foco no Pomodoro
					</button>
				{:else}
					<p class="lv-forest__active-cue">
						<span class="lv-forest__active-pulse" aria-hidden="true"></span>
						Ciclo em andamento no Pomodoro ({clock}) — sua muda está crescendo!
					</p>
				{/if}
			</div>
		{:else}
			{#if isRunning}
				<div class="lv-forest__active-banner" role="status" aria-live="polite">
					<span class="lv-forest__active-pulse" aria-hidden="true"></span>
					<span>Ciclo de foco em andamento no Pomodoro: <strong>{clock} restantes</strong>. Mantenha o foco para florescer uma árvore sadia!</span>
				</div>
			{/if}

			<!-- Grade de árvores (ESTÁVEL: NUNCA SE MOVE NO HOVER) -->
			<ul class="lv-forest__grid" role="list" aria-label="Árvores cultivadas">
				{#each visibleTrees as tree (tree.sessionId)}
					<li class="lv-forest__tree-wrapper">
						<button
							type="button"
							class="lv-forest__tree-item"
							class:is-withered={!tree.completed}
							class:is-selected={selectedTree?.sessionId === tree.sessionId}
							aria-pressed={selectedTree?.sessionId === tree.sessionId}
							aria-label={tree.label}
							title="{tree.stageName} · {tree.minutes} min · {tree.completed ? 'Sadia' : 'Seca'}"
							onclick={() => (selectedTree = selectedTree?.sessionId === tree.sessionId ? null : tree)}
						>
							<div class="lv-forest__tree-visual" aria-hidden="true">
								<ForestTreeSvg stage={tree.stage} size={50} />
							</div>
							<span class="lv-forest__tree-name">{tree.stageName}</span>
							<span class="lv-forest__tree-time">{tree.minutes} min</span>
						</button>
					</li>
				{/each}
			</ul>

			<!-- Painel de Inspeção Detalhada (Abaixo da Grade, SEM deslocamento das árvores) -->
			{#if selectedTree}
				<div class="lv-forest__inspector" role="status" aria-live="polite">
					<div class="lv-forest__inspector-body">
						<div class="lv-forest__inspector-visual" aria-hidden="true">
							<ForestTreeSvg stage={selectedTree.stage} size={48} />
						</div>
						<div class="lv-forest__inspector-details">
							<div class="lv-forest__inspector-head">
								<strong class="lv-forest__inspector-title">{selectedTree.stageName}</strong>
								<span
									class="lv-forest__status-pill"
									class:is-healthy={selectedTree.completed}
									class:is-withered={!selectedTree.completed}
								>
									{selectedTree.completed ? 'Sadia' : 'Seca'}
								</span>
								<span class="lv-forest__inspector-time">{selectedTree.minutes} min de foco</span>
							</div>
							<p class="lv-forest__inspector-desc">
								{stageDescription(selectedTree.stage)}
								{#if selectedTree.timestamp}
									<span class="lv-forest__inspector-date">· Cultivada em {formatDateTime(selectedTree.timestamp)}</span>
								{/if}
							</p>
						</div>
					</div>
					<button
						type="button"
						class="lv-forest__inspector-close"
						aria-label="Fechar detalhes da árvore"
						onclick={() => (selectedTree = null)}
					>
						✕
					</button>
				</div>
			{/if}
		{/if}
	</div>

	<!-- Guia de Espécies da Floresta -->
	<ForestGuide />
</section>

<style>
	.lv-forest {
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
		padding-bottom: 1.25rem;
	}

	.lv-forest__head-title {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	/* Segmented Control para Filtros */
	.lv-forest__filters {
		display: inline-flex;
		align-items: center;
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 3px;
		gap: 3px;
	}

	.lv-forest__filter-btn {
		min-height: 28px;
		padding: 0 0.75rem;
		font-size: 0.75rem;
		font-weight: 600;
		border-radius: calc(var(--radius) - 2px);
		border: 1px solid transparent;
		background: transparent;
		color: var(--muted);
		cursor: pointer;
		transition: background-color 0.15s ease, border-color 0.15s ease, color 0.15s ease;
		white-space: nowrap;
	}

	.lv-forest__filter-btn:hover {
		color: var(--ink);
	}

	.lv-forest__filter-btn.is-active {
		background: var(--surface);
		color: var(--ink);
		border-color: var(--line-strong);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
	}

	/* Métricas da Floresta (estilo Atelier .lv-focus__stats) */
	.lv-forest__stats {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: 1rem;
		padding: 0.25rem 0;
	}

	.lv-forest__stats div {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.lv-forest__stats strong {
		font-family: var(--display);
		font-size: 1.5rem;
		font-weight: 700;
		line-height: 1.1;
		letter-spacing: -0.02em;
		font-variant-numeric: tabular-nums;
		color: var(--ink);
	}

	.lv-forest__stats strong.is-healthy {
		color: var(--ok);
	}

	.lv-forest__stats strong.is-withered {
		color: var(--muted);
	}

	.lv-forest__stats strong.is-time {
		color: var(--accent);
	}

	.lv-forest__stats span {
		font-size: 0.8125rem;
		color: var(--muted);
	}

	/* Bosque Canvas */
	.lv-forest__canvas {
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
		min-height: 180px;
	}

	.lv-forest__loading {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 140px;
		color: var(--muted);
		font-size: 0.875rem;
		margin: auto;
	}

	/* Estado Vazio */
	.lv-forest__empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 180px;
		text-align: center;
		padding: 1.5rem 1rem;
		margin: auto;
		max-width: 440px;
		gap: 0.5rem;
	}

	.lv-forest__empty-icon {
		color: var(--muted);
		margin-bottom: 0.25rem;
	}

	.lv-forest__empty-title {
		font-size: 0.9375rem;
		font-weight: 600;
		color: var(--ink);
		margin: 0;
	}

	.lv-forest__empty-desc {
		font-size: 0.8125rem;
		color: var(--muted);
		line-height: 1.45;
	}

	.lv-forest__cta-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.5rem;
		cursor: pointer;
	}

	.lv-forest__active-cue {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.5rem;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--ok);
	}

	.lv-forest__active-banner {
		display: flex;
		align-items: center;
		gap: 0.625rem;
		padding: 0.625rem 0.875rem;
		background: color-mix(in srgb, var(--ok) 8%, var(--surface));
		border: 1px solid color-mix(in srgb, var(--ok) 25%, transparent);
		border-radius: var(--radius);
		font-size: 0.8125rem;
		color: var(--ink);
	}

	.lv-forest__active-pulse {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--ok);
		box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7);
		animation: lv-pulse 1.8s infinite;
		flex-shrink: 0;
	}

	@keyframes lv-pulse {
		0% {
			transform: scale(0.95);
			box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7);
		}
		70% {
			transform: scale(1);
			box-shadow: 0 0 0 8px rgba(16, 185, 129, 0);
		}
		100% {
			transform: scale(0.95);
			box-shadow: 0 0 0 0 rgba(16, 185, 129, 0);
		}
	}

	/* Grade de Árvores (layout estável, sem salto) */
	.lv-forest__grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
		gap: 0.875rem;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.lv-forest__tree-wrapper {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
	}

	.lv-forest__tree-item {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		width: 100%;
		padding: 0.75rem 0.375rem;
		border-radius: var(--radius);
		background: var(--surface);
		border: 1px solid var(--line);
		color: inherit;
		font: inherit;
		cursor: pointer;
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
		transition: transform 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
	}

	.lv-forest__tree-item:hover {
		transform: translateY(-2px);
		border-color: var(--line-strong);
		box-shadow: 0 4px 10px rgba(0, 0, 0, 0.06);
	}

	.lv-forest__tree-item.is-selected {
		transform: translateY(-2px);
		border-color: var(--accent);
		outline: 2px solid var(--accent);
		outline-offset: 1px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
	}

	.lv-forest__tree-item:focus-visible {
		transform: translateY(-2px);
		border-color: var(--accent);
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}

	.lv-forest__tree-item.is-withered {
		opacity: 0.85;
		border-style: dashed;
	}

	.lv-forest__tree-visual {
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.lv-forest__tree-name {
		font-size: 0.75rem;
		font-weight: 600;
		text-align: center;
		margin-top: 0.375rem;
		color: var(--ink);
		line-height: 1.25;
	}

	.lv-forest__tree-time {
		font-size: 0.6875rem;
		font-family: var(--mono);
		color: var(--muted);
		margin-top: 0.15rem;
	}

	/* Painel de Inspeção (abaixo da grade) */
	.lv-forest__inspector {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.75rem 1rem;
		background: var(--surface);
		border: 1px solid var(--line-strong);
		border-radius: var(--radius);
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
		animation: lv-fade-in 0.15s ease;
	}

	@keyframes lv-fade-in {
		from {
			opacity: 0;
			transform: translateY(4px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.lv-forest__inspector-body {
		display: flex;
		align-items: center;
		gap: 0.875rem;
		min-width: 0;
	}

	.lv-forest__inspector-visual {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.lv-forest__inspector-details {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		min-width: 0;
	}

	.lv-forest__inspector-head {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.lv-forest__inspector-title {
		font-size: 0.875rem;
		color: var(--ink);
	}

	.lv-forest__status-pill {
		font-size: 0.6875rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.04em;
		padding: 0.1rem 0.45rem;
		border-radius: 9999px;
	}

	.lv-forest__status-pill.is-healthy {
		background: color-mix(in srgb, var(--ok) 12%, transparent);
		color: var(--ok);
		border: 1px solid color-mix(in srgb, var(--ok) 28%, transparent);
	}

	.lv-forest__status-pill.is-withered {
		background: color-mix(in srgb, var(--muted) 12%, transparent);
		color: var(--muted);
		border: 1px solid color-mix(in srgb, var(--muted) 28%, transparent);
	}

	.lv-forest__inspector-time {
		font-size: 0.75rem;
		font-family: var(--mono);
		color: var(--muted);
	}

	.lv-forest__inspector-desc {
		font-size: 0.8125rem;
		color: var(--muted);
		margin: 0;
		line-height: 1.4;
	}

	.lv-forest__inspector-date {
		color: var(--muted);
		margin-left: 0.25rem;
	}

	.lv-forest__inspector-close {
		background: transparent;
		border: none;
		color: var(--muted);
		font-size: 1rem;
		padding: 0.25rem 0.5rem;
		border-radius: var(--radius);
		cursor: pointer;
		transition: color 0.15s ease, background 0.15s ease;
	}

	.lv-forest__inspector-close:hover {
		color: var(--ink);
		background: var(--surface-2);
	}

	@media (max-width: 640px) {
		.lv-forest__stats {
			grid-template-columns: repeat(2, 1fr);
		}

		.lv-forest__filters {
			width: 100%;
			overflow-x: auto;
		}

		.lv-forest__filter-btn {
			flex: 1;
			text-align: center;
		}

		.lv-forest__grid {
			grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
			gap: 0.625rem;
		}
	}
</style>
