<script lang="ts">
	import { onMount } from 'svelte';
	import {
		buildForest,
		calculateForestStats,
		filterForest,
		type ForestFilter,
		type ForestTree
	} from './forest';
	import ForestEcosystemBand from './ForestEcosystemBand.svelte';
	import ForestGrowthStage from './ForestGrowthStage.svelte';
	import ForestGuide from './ForestGuide.svelte';
	import ForestInspector from './ForestInspector.svelte';
	import ForestIsland2D5 from './ForestIsland2D5.svelte';
	import ForestNursery from './ForestNursery.svelte';
	import ForestTreeSvg from './ForestTreeSvg.svelte';
	import { readSessionSeeds } from './forest-catalog';
	import { nursery } from './nursery-store';
	import { formatClock, readFocusSessions, type FocusSession } from './pomodoro';
	import { focusSync, pomodoro, setMode, startTimer } from './pomodoro-store';
	import { transport } from './transport';

	let sessionsLoaded = $state<ForestTree[]>([]);
	let loading = $state(true);
	let filter = $state<ForestFilter>('week');
	let viewMode = $state<'island' | 'grid'>('island');
	let selectedTree = $state<ForestTree | null>(null);
	let activeTab = $state<'forest' | 'nursery'>('forest');

	const visibleTrees = $derived(filterForest(sessionsLoaded, filter));
	const stats = $derived(calculateForestStats(visibleTrees));

	const isRunning = $derived($pomodoro.status === 'running');
	const isPaused = $derived($pomodoro.status === 'paused');
	const hasActiveSession = $derived(isRunning || isPaused);
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

		const seeds = readSessionSeeds(storage());
		sessionsLoaded = buildForest(Array.from(sessionMap.values()), seeds);
		loading = false;
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

	let lastLoadedCompletedAt = $state<number | null>(null);

	$effect(() => {
		const completed = $pomodoro.lastCompleted;
		if (completed?.mode === 'focus' && completed.at !== lastLoadedCompletedAt) {
			lastLoadedCompletedAt = completed.at;
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
			<div class="lv-forest__nav-tabs" role="group" aria-label="Visualização da floresta">
				<button
					type="button"
					class="lv-forest__nav-tab"
					class:is-active={activeTab === 'forest'}
					aria-pressed={activeTab === 'forest'}
					onclick={() => (activeTab = 'forest')}
				>
					Meu Bosque
					<span class="lv-count">{visibleTrees.length}</span>
				</button>
				<button
					type="button"
					class="lv-forest__nav-tab"
					class:is-active={activeTab === 'nursery'}
					aria-pressed={activeTab === 'nursery'}
					onclick={() => (activeTab = 'nursery')}
				>
					Viveiro
					<span class="lv-forest__drops-pill">{$nursery.dropsBalance} gotas</span>
				</button>
			</div>
		</div>

		{#if activeTab === 'forest'}
			<div class="lv-forest__toolbar">
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

				<!-- Toggle de Visualização: Ilha 2.5D vs Grade -->
				<div class="lv-forest__view-toggle" role="group" aria-label="Modo de visualização">
					<button
						type="button"
						class="lv-forest__filter-btn"
						class:is-active={viewMode === 'island'}
						aria-pressed={viewMode === 'island'}
						onclick={() => (viewMode = 'island')}
						title="Vista isométrica 2.5D em ilha flutuante"
					>
						<svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
							<path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" />
						</svg>
						Ilha 2.5D
					</button>
					<button
						type="button"
						class="lv-forest__filter-btn"
						class:is-active={viewMode === 'grid'}
						aria-pressed={viewMode === 'grid'}
						onclick={() => (viewMode = 'grid')}
						title="Vista em grade de catálogo"
					>
						<svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
							<rect x="3" y="3" width="7" height="7" rx="1" />
							<rect x="14" y="3" width="7" height="7" rx="1" />
							<rect x="14" y="14" width="7" height="7" rx="1" />
							<rect x="3" y="14" width="7" height="7" rx="1" />
						</svg>
						Grade
					</button>
				</div>
			</div>
		{/if}
	</div>

	{#if activeTab === 'nursery'}
		<ForestNursery onclose={() => (activeTab = 'forest')} />
	{:else}
		{#if viewMode === 'grid'}
			<!-- Métricas editoriais da Floresta (Atelier design para modo Grade) -->
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
		{/if}

		<!-- Bosque / Visualizador de Árvores -->
		<div
			class="lv-forest__canvas lv-forest__canvas--biome-{stats.ecosystem.level}"
			class:is-island-mode={viewMode === 'island'}
			role="region"
			aria-label="Visualização do bosque"
		>
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
			{#if hasActiveSession && $pomodoro.mode === 'focus'}
				<ForestGrowthStage
					speciesId={$nursery.activeSeed}
					remainingSeconds={$pomodoro.remainingSeconds}
					totalSeconds={$pomodoro.totalSeconds}
					{clock}
				/>
			{/if}

			{#if viewMode === 'island'}
				<!-- Ilha Isométrica 2.5D Estilo Forest App -->
				<ForestIsland2D5
					trees={visibleTrees}
					{stats}
					{selectedTree}
					onselect={(tree) => (selectedTree = selectedTree?.sessionId === tree.sessionId ? null : tree)}
				/>

				<!-- Faixa de Evolução do Ecossistema da Floresta -->
				<ForestEcosystemBand ecosystem={stats.ecosystem} healthyTrees={stats.healthyTrees} />
			{:else}
				<!-- Faixa de Evolução do Ecossistema da Floresta -->
				<ForestEcosystemBand ecosystem={stats.ecosystem} healthyTrees={stats.healthyTrees} />

				<!-- Grade de árvores (ESTÁVEL: NUNCA SE MOVE NO HOVER) -->
				<ul class="lv-forest__grid" role="list" aria-label="Árvores cultivadas">
					{#each visibleTrees as tree (tree.sessionId)}
						<li class="lv-forest__tree-wrapper">
							<button
								type="button"
								class="lv-forest__tree-item"
								class:is-withered={!tree.completed}
								class:is-selected={selectedTree?.sessionId === tree.sessionId}
								class:is-stage-sapling={tree.stage === 'sapling'}
								class:is-stage-oak={tree.stage === 'oak'}
								class:is-stage-pine={tree.stage === 'pine'}
								class:is-stage-sakura={tree.stage === 'sakura'}
								class:is-stage-willow={tree.stage === 'willow'}
								class:is-stage-ginkgo={tree.stage === 'ginkgo'}
								class:is-stage-ancient={tree.stage === 'ancient'}
								class:is-stage-withered={tree.stage === 'withered'}
								aria-pressed={selectedTree?.sessionId === tree.sessionId}
								aria-label={tree.label}
								title="{tree.stageName} · {tree.minutes} min · {tree.completed ? 'Sadia' : 'Seca'}"
								onclick={() => (selectedTree = selectedTree?.sessionId === tree.sessionId ? null : tree)}
							>
								<div class="lv-forest__tree-visual" aria-hidden="true">
									<ForestTreeSvg
										stage={tree.stage}
										size={tree.stage === 'sapling' ? 44 : tree.stage === 'ancient' ? 62 : tree.stage === 'withered' ? 46 : 52}
									/>
								</div>
								<span class="lv-forest__tree-name">{tree.stageName}</span>
								<span class="lv-forest__tree-time">{tree.minutes} min</span>
							</button>
						</li>
					{/each}
				</ul>
			{/if}

			<!-- Painel de Inspeção Detalhada (Abaixo da Grade, SEM deslocamento das árvores) -->
			{#if selectedTree}
				<ForestInspector tree={selectedTree} onclose={() => (selectedTree = null)} />
			{/if}
		{/if}
	</div>

	<!-- Guia de Espécies da Floresta -->
	<ForestGuide speciesCounts={stats.speciesCounts} />
{/if}
</section>

<style>
	.lv-forest {
		display: flex;
		flex-direction: column;
		gap: 0.875rem;
		padding-bottom: 0.875rem;
	}

	.lv-forest__head-title {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		flex-wrap: wrap;
	}

	.lv-forest__nav-tabs {
		display: flex;
		align-items: center;
		gap: 0.375rem;
	}

	.lv-forest__nav-tab {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 0.3rem 0.65rem;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--muted);
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.lv-forest__nav-tab.is-active {
		background: var(--surface);
		color: var(--ink);
		border-color: var(--accent);
	}

	.lv-forest__drops-pill {
		display: inline-flex;
		align-items: center;
		background: color-mix(in srgb, #0284c7 14%, transparent);
		color: #0284c7;
		font-size: 0.75rem;
		padding: 0.1rem 0.4rem;
		border-radius: 9999px;
		font-weight: 700;
	}

	.lv-forest__toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		flex-wrap: wrap;
	}

	/* Segmented Control para Filtros e Modos de Vista */
	.lv-forest__filters,
	.lv-forest__view-toggle {
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

	/* Bosque Canvas & Ambiência dos Biomas */
	.lv-forest__canvas {
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
		min-height: 180px;
		transition: background 0.3s ease, border-color 0.3s ease, box-shadow 0.3s ease;
	}

	.lv-forest__canvas.is-island-mode {
		background: transparent;
		border: none;
		padding: 0;
		box-shadow: none;
		gap: 0.85rem;
	}

	.lv-forest__canvas--biome-0 {
		background: radial-gradient(ellipse at 50% 100%, color-mix(in srgb, #92400e 6%, var(--surface-2)) 0%, var(--surface-2) 80%);
		border-color: color-mix(in srgb, #92400e 20%, var(--line));
	}

	.lv-forest__canvas--biome-1 {
		background: radial-gradient(ellipse at 50% 100%, color-mix(in srgb, #16a34a 9%, var(--surface-2)) 0%, var(--surface-2) 80%);
		border-color: color-mix(in srgb, #16a34a 24%, var(--line));
	}

	.lv-forest__canvas--biome-2 {
		background: radial-gradient(ellipse at 50% 100%, color-mix(in srgb, #15803d 13%, var(--surface-2)) 0%, color-mix(in srgb, #059669 5%, var(--surface-2)) 60%, var(--surface-2) 100%);
		border-color: color-mix(in srgb, #15803d 30%, var(--line));
	}

	.lv-forest__canvas--biome-3 {
		background: radial-gradient(ellipse at 50% 100%, color-mix(in srgb, #047857 16%, var(--surface-2)) 0%, color-mix(in srgb, #0f766e 8%, var(--surface-2)) 65%, var(--surface-2) 100%);
		border-color: color-mix(in srgb, #047857 36%, var(--line));
		box-shadow: inset 0 0 40px color-mix(in srgb, #047857 8%, transparent);
	}

	.lv-forest__canvas--biome-4 {
		background: radial-gradient(ellipse at 50% 90%, color-mix(in srgb, #065f46 20%, var(--surface-2)) 0%, color-mix(in srgb, #1e3a8a 6%, var(--surface-2)) 65%, var(--surface-2) 100%);
		border-color: color-mix(in srgb, #059669 40%, var(--line));
		box-shadow: inset 0 0 50px color-mix(in srgb, #059669 10%, transparent);
	}

	.lv-forest__canvas--biome-5 {
		background: radial-gradient(ellipse at 50% 85%, color-mix(in srgb, #eab308 14%, color-mix(in srgb, #047857 24%, var(--surface-2))) 0%, color-mix(in srgb, #047857 14%, var(--surface-2)) 60%, var(--surface-2) 100%);
		border-color: color-mix(in srgb, #eab308 45%, var(--line));
		box-shadow: inset 0 0 60px color-mix(in srgb, #eab308 10%, transparent), 0 0 20px color-mix(in srgb, #eab308 8%, transparent);
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

	.lv-forest__tree-item.is-stage-ancient {
		border-color: color-mix(in srgb, #eab308 35%, var(--line));
		background: color-mix(in srgb, #eab308 4%, var(--surface));
	}

	.lv-forest__tree-item.is-stage-pine {
		border-color: color-mix(in srgb, #059669 22%, var(--line));
	}

	.lv-forest__tree-item.is-stage-sakura {
		border-color: color-mix(in srgb, #ec4899 30%, var(--line));
		background: color-mix(in srgb, #ec4899 4%, var(--surface));
	}

	.lv-forest__tree-item.is-stage-willow {
		border-color: color-mix(in srgb, #059669 28%, var(--line));
		background: color-mix(in srgb, #059669 4%, var(--surface));
	}

	.lv-forest__tree-item.is-stage-ginkgo {
		border-color: color-mix(in srgb, #eab308 32%, var(--line));
		background: color-mix(in srgb, #eab308 4%, var(--surface));
	}

	.lv-forest__tree-item.is-stage-sapling {
		border-color: color-mix(in srgb, #65a30d 22%, var(--line));
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
