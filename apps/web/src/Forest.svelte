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
	let hoveredTree = $state<ForestTree | null>(null);
	let selectedTree = $state<ForestTree | null>(null);

	const visibleTrees = $derived(filterForest(sessionsLoaded, filter));
	const stats = $derived(calculateForestStats(visibleTrees));
	const activeTree = $derived(hoveredTree ?? selectedTree);

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

	// Reatividade quando uma sessão de foco é completada pelo Pomodoro
	$effect(() => {
		if ($pomodoro.lastCompleted?.mode === 'focus') {
			void loadForestData();
		}
	});

	onMount(() => {
		void loadForestData();
	});
</script>

<div class="lv-forest">
	<!-- Cabeçalho -->
	<div class="lv-forest__header">
		<div class="lv-forest__intro">
			<div class="lv-forest__heading-row">
				<h2 class="lv-forest__title">Floresta de Estudos</h2>
				<span class="lv-forest__badge" aria-label="{visibleTrees.length} árvores no período">
					{visibleTrees.length} {visibleTrees.length === 1 ? 'árvore' : 'árvores'}
				</span>
			</div>
			<p class="lv-forest__subtitle">Cada ciclo de foco concluído floresce uma árvore na sua floresta pessoal.</p>
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

	<!-- Métricas da Floresta -->
	<div class="lv-forest__stats">
		<div class="lv-forest__stat-card">
			<span class="lv-forest__stat-value is-healthy">{stats.healthyTrees}</span>
			<span class="lv-forest__stat-label">Árvores sadias</span>
		</div>
		<div class="lv-forest__stat-card">
			<span class="lv-forest__stat-value is-withered">{stats.witheredTrees}</span>
			<span class="lv-forest__stat-label">Árvores secas</span>
		</div>
		<div class="lv-forest__stat-card">
			<span class="lv-forest__stat-value is-rate">{stats.successRate}%</span>
			<span class="lv-forest__stat-label">Taxa de florescimento</span>
		</div>
		<div class="lv-forest__stat-card">
			<span class="lv-forest__stat-value is-time">{stats.totalMinutes} min</span>
			<span class="lv-forest__stat-label">Tempo dedicado</span>
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
						<path d="M24 38V22M24 22c0-8 6-12 12-12-2 6-4 12-12 12zM24 26c0-6-5-9-10-9 1.5 4.5 3 9 10 9z" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
					</svg>
				</div>
				<p class="lv-forest__empty-title">Nenhuma árvore cultivada neste período</p>
				<span class="lv-forest__empty-desc">
					Inicie um ciclo de foco no Pomodoro para plantar sua primeira muda e ver sua floresta florescer!
				</span>
				{#if !isRunning}
					<button class="lv-forest__cta-btn" type="button" onclick={handleStartFocus}>
						<svg viewBox="0 0 24 24" aria-hidden="true" width="16" height="16"><path d="M8 5v14l11-7z" fill="currentColor"/></svg>
						Iniciar ciclo de foco agora
					</button>
				{:else}
					<p class="lv-forest__active-cue">
						<span class="lv-forest__active-pulse" aria-hidden="true"></span>
						Ciclo em andamento no Pomodoro ({clock})
					</p>
				{/if}
			</div>
		{:else}
			<!-- Banner de detalhe da árvore selecionada ou sob o cursor -->
			{#if activeTree}
				<div class="lv-forest__detail-banner" role="status" aria-live="polite">
					<div class="lv-forest__detail-left">
						<div class="lv-forest__detail-icon" aria-hidden="true">
							<ForestTreeSvg stage={activeTree.stage} size={38} />
						</div>
						<div class="lv-forest__detail-text">
							<div class="lv-forest__detail-title-row">
								<strong class="lv-forest__detail-name">{activeTree.stageName}</strong>
								<span
									class="lv-forest__status-pill"
									class:is-healthy={activeTree.completed}
									class:is-withered={!activeTree.completed}
								>
									{activeTree.completed ? 'Sadia' : 'Seca'}
								</span>
								<span class="lv-forest__detail-time">{activeTree.minutes} min</span>
							</div>
							<p class="lv-forest__detail-desc">
								{stageDescription(activeTree.stage)}
								{#if activeTree.timestamp}
									<span class="lv-forest__detail-date">· Cultivada em {formatDateTime(activeTree.timestamp)}</span>
								{/if}
							</p>
						</div>
					</div>
					{#if selectedTree}
						<button
							type="button"
							class="lv-forest__detail-close"
							aria-label="Fechar detalhes da árvore"
							onclick={() => (selectedTree = null)}
						>
							✕
						</button>
					{/if}
				</div>
			{/if}

			<!-- Grade de árvores -->
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
							onmouseenter={() => (hoveredTree = tree)}
							onmouseleave={() => (hoveredTree = null)}
							onfocus={() => (hoveredTree = tree)}
							onblur={() => (hoveredTree = null)}
							onclick={() => (selectedTree = selectedTree?.sessionId === tree.sessionId ? null : tree)}
						>
							<div class="lv-forest__tree-visual" aria-hidden="true">
								<ForestTreeSvg stage={tree.stage} size={54} />
							</div>
							<span class="lv-forest__tree-name">{tree.stageName}</span>
							<span class="lv-forest__tree-time">{tree.minutes} min</span>
						</button>
					</li>
				{/each}
			</ul>
		{/if}
	</div>

	<!-- Guia de Espécies da Floresta -->
	<ForestGuide />
</div>

<style>
	.lv-forest {
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
		padding: 1.5rem;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		box-shadow: var(--shadow);
		color: var(--ink);
	}

	.lv-forest__header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
	}

	.lv-forest__intro {
		min-width: 0;
	}

	.lv-forest__heading-row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
	}

	.lv-forest__title {
		font-family: var(--display);
		font-size: 1.25rem;
		font-weight: 700;
		letter-spacing: -0.02em;
		margin: 0;
		color: var(--ink);
	}

	.lv-forest__badge {
		font-size: 0.75rem;
		font-family: var(--mono);
		color: var(--muted);
		background: var(--surface-2);
		border: 1px solid var(--line);
		padding: 0.15rem 0.5rem;
		border-radius: 9999px;
		white-space: nowrap;
	}

	.lv-forest__subtitle {
		font-size: 0.875rem;
		margin: 0.35rem 0 0;
		color: var(--muted);
		line-height: 1.45;
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
		min-height: 32px;
		padding: 0 0.875rem;
		font-size: 0.8125rem;
		font-weight: 600;
		border-radius: calc(var(--radius) - 2px);
		border: 1px solid transparent;
		background: transparent;
		color: var(--muted);
		cursor: pointer;
		transition: background-color 0.15s ease, border-color 0.15s ease, color 0.15s ease, box-shadow 0.15s ease;
		white-space: nowrap;
	}

	.lv-forest__filter-btn:hover {
		color: var(--ink);
	}

	.lv-forest__filter-btn.is-active {
		background: var(--surface);
		color: var(--ink);
		border-color: var(--line-strong);
		box-shadow: 0 1px 3px rgba(0, 0, 0, 0.08);
	}

	/* Cartões de Métricas */
	.lv-forest__stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
		gap: 0.75rem;
	}

	.lv-forest__stat-card {
		display: flex;
		flex-direction: column;
		padding: 0.875rem 1rem;
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		transition: border-color 0.15s ease;
	}

	.lv-forest__stat-card:hover {
		border-color: var(--line-strong);
	}

	.lv-forest__stat-value {
		font-family: var(--display);
		font-size: 1.5rem;
		font-weight: 700;
		line-height: 1.1;
		letter-spacing: -0.02em;
		font-variant-numeric: tabular-nums;
		color: var(--ink);
	}

	.lv-forest__stat-value.is-healthy {
		color: var(--ok);
	}

	.lv-forest__stat-value.is-withered {
		color: var(--muted);
	}

	.lv-forest__stat-value.is-rate {
		color: var(--ink);
	}

	.lv-forest__stat-value.is-time {
		color: var(--accent);
	}

	.lv-forest__stat-label {
		font-size: 0.75rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--muted);
		margin-top: 0.35rem;
	}

	/* Bosque Canvas */
	.lv-forest__canvas {
		position: relative;
		min-height: 220px;
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 1.25rem;
		display: flex;
		flex-direction: column;
	}

	.lv-forest__loading {
		display: flex;
		align-items: center;
		justify-content: center;
		min-height: 160px;
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
		min-height: 190px;
		text-align: center;
		padding: 2rem 1rem;
		margin: auto;
		max-width: 460px;
		gap: 0.5rem;
	}

	.lv-forest__empty-icon {
		color: var(--muted);
		margin-bottom: 0.25rem;
	}

	.lv-forest__empty-title {
		font-size: 1rem;
		font-weight: 600;
		color: var(--ink);
		margin: 0;
	}

	.lv-forest__empty-desc {
		font-size: 0.875rem;
		color: var(--muted);
		line-height: 1.5;
	}

	.lv-forest__cta-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		margin-top: 0.75rem;
		padding: 0.5rem 1.125rem;
		background: var(--accent);
		color: var(--accent-ink);
		font-weight: 600;
		font-size: 0.875rem;
		border-radius: var(--radius);
		border: 1px solid var(--accent);
		cursor: pointer;
		transition: opacity 0.15s ease, transform 0.15s ease;
	}

	.lv-forest__cta-btn:hover {
		opacity: 0.92;
		transform: translateY(-1px);
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

	.lv-forest__active-pulse {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--ok);
		box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7);
		animation: lv-pulse 1.8s infinite;
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

	/* Banner de Detalhes da Árvore Ativa */
	.lv-forest__detail-banner {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		padding: 0.875rem 1.125rem;
		background: var(--surface);
		border: 1px solid var(--line-strong);
		border-radius: var(--radius);
		margin-bottom: 1.125rem;
		box-shadow: 0 2px 8px rgba(0, 0, 0, 0.04);
		animation: lv-fade-in 0.15s ease;
	}

	@keyframes lv-fade-in {
		from {
			opacity: 0;
			transform: translateY(-4px);
		}
		to {
			opacity: 1;
			transform: translateY(0);
		}
	}

	.lv-forest__detail-left {
		display: flex;
		align-items: center;
		gap: 0.875rem;
		min-width: 0;
	}

	.lv-forest__detail-icon {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.lv-forest__detail-text {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		min-width: 0;
	}

	.lv-forest__detail-title-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}

	.lv-forest__detail-name {
		font-size: 0.9375rem;
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
		background: color-mix(in srgb, var(--ok) 14%, transparent);
		color: var(--ok);
		border: 1px solid color-mix(in srgb, var(--ok) 30%, transparent);
	}

	.lv-forest__status-pill.is-withered {
		background: color-mix(in srgb, var(--muted) 14%, transparent);
		color: var(--muted);
		border: 1px solid color-mix(in srgb, var(--muted) 30%, transparent);
	}

	.lv-forest__detail-time {
		font-size: 0.75rem;
		font-family: var(--mono);
		color: var(--muted);
	}

	.lv-forest__detail-desc {
		font-size: 0.8125rem;
		color: var(--muted);
		margin: 0;
		line-height: 1.4;
	}

	.lv-forest__detail-date {
		color: var(--muted);
		margin-left: 0.25rem;
	}

	.lv-forest__detail-close {
		background: transparent;
		border: none;
		color: var(--muted);
		font-size: 1rem;
		padding: 0.25rem 0.5rem;
		border-radius: var(--radius);
		cursor: pointer;
		transition: color 0.15s ease, background 0.15s ease;
	}

	.lv-forest__detail-close:hover {
		color: var(--ink);
		background: var(--surface-2);
	}

	/* Grade de Árvores */
	.lv-forest__grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
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
		padding: 0.875rem 0.5rem;
		border-radius: var(--radius);
		background: var(--surface);
		border: 1px solid var(--line);
		color: inherit;
		font: inherit;
		cursor: pointer;
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
		transition: transform 0.15s ease, border-color 0.15s ease, box-shadow 0.15s ease;
	}

	.lv-forest__tree-item:hover,
	.lv-forest__tree-item.is-selected {
		transform: translateY(-2px);
		border-color: var(--accent);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
	}

	.lv-forest__tree-item:focus-visible {
		transform: translateY(-2px);
		border-color: var(--accent);
		outline: 2px solid var(--focus);
		outline-offset: 2px;
	}

	.lv-forest__tree-item.is-selected {
		border-color: var(--accent);
		border-width: 2px;
		padding: calc(0.875rem - 1px) calc(0.5rem - 1px);
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
		margin-top: 0.5rem;
		color: var(--ink);
		line-height: 1.25;
	}

	.lv-forest__tree-time {
		font-size: 0.75rem;
		font-family: var(--mono);
		color: var(--muted);
		margin-top: 0.2rem;
	}

	@media (max-width: 640px) {
		.lv-forest {
			padding: 1rem;
		}

		.lv-forest__header {
			flex-direction: column;
			align-items: flex-start;
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
			grid-template-columns: repeat(auto-fill, minmax(85px, 1fr));
			gap: 0.625rem;
		}
	}
</style>
