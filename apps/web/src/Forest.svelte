<script lang="ts">
	import { onMount } from 'svelte';
	import {
		buildForest,
		calculateForestStats,
		filterForest,
		type ForestFilter,
		type ForestTree
	} from './forest';
	import ForestTreeSvg from './ForestTreeSvg.svelte';
	import { readFocusSessions } from './pomodoro';
	import { transport } from './transport';

	let sessionsLoaded = $state<ForestTree[]>([]);
	let loading = $state(true);
	let filter = $state<ForestFilter>('week');
	let hoveredTree = $state<ForestTree | null>(null);

	const visibleTrees = $derived(filterForest(sessionsLoaded, filter));
	const stats = $derived(calculateForestStats(visibleTrees));

	function storage(): Storage | null {
		try {
			return window.localStorage;
		} catch {
			return null;
		}
	}

	async function loadForestData() {
		loading = true;
		try {
			// Tenta buscar do backend primeiro
			const remote = await transport.listFocusSessions(500);
			if (remote && remote.length > 0) {
				sessionsLoaded = buildForest(remote);
				loading = false;
				return;
			}
		} catch {
			// Fallback para cache local / offline outbox
		}

		// Fallback para localStorage
		const local = readFocusSessions(storage());
		sessionsLoaded = buildForest(local);
		loading = false;
	}

	onMount(() => {
		void loadForestData();
	});
</script>

<div class="lv-forest">
	<div class="lv-forest__header">
		<div>
			<h2 class="lv-forest__title">Floresta de Estudos</h2>
			<p class="lv-forest__subtitle">Cada ciclo de foco concluído floresce uma árvore na sua floresta pessoal.</p>
		</div>
		<div class="lv-forest__filters" role="group" aria-label="Período da floresta">
			<button
				type="button"
				class="lvui-btn lvui-btn-sm"
				class:is-active={filter === 'today'}
				aria-pressed={filter === 'today'}
				onclick={() => (filter = 'today')}
			>
				Hoje
			</button>
			<button
				type="button"
				class="lvui-btn lvui-btn-sm"
				class:is-active={filter === 'week'}
				aria-pressed={filter === 'week'}
				onclick={() => (filter = 'week')}
			>
				Esta semana
			</button>
			<button
				type="button"
				class="lvui-btn lvui-btn-sm"
				class:is-active={filter === 'month'}
				aria-pressed={filter === 'month'}
				onclick={() => (filter = 'month')}
			>
				Este mês
			</button>
			<button
				type="button"
				class="lvui-btn lvui-btn-sm"
				class:is-active={filter === 'all'}
				aria-pressed={filter === 'all'}
				onclick={() => (filter = 'all')}
			>
				Tudo
			</button>
		</div>
	</div>

	<!-- Métricas da floresta -->
	<div class="lv-forest__stats">
		<div class="lv-forest__stat-card">
			<span class="lv-forest__stat-value">{stats.healthyTrees}</span>
			<span class="lv-forest__stat-label">Árvores sadias</span>
		</div>
		<div class="lv-forest__stat-card">
			<span class="lv-forest__stat-value is-withered">{stats.witheredTrees}</span>
			<span class="lv-forest__stat-label">Árvores secas</span>
		</div>
		<div class="lv-forest__stat-card">
			<span class="lv-forest__stat-value">{stats.successRate}%</span>
			<span class="lv-forest__stat-label">Taxa de florescimento</span>
		</div>
		<div class="lv-forest__stat-card">
			<span class="lv-forest__stat-value">{stats.totalMinutes} min</span>
			<span class="lv-forest__stat-label">Tempo dedicado</span>
		</div>
	</div>

	<!-- Bosque / Visualizador de árvores -->
	<div class="lv-forest__canvas" role="region" aria-label="Visualização do bosque">
		{#if loading}
			<p class="lv-forest__empty" aria-live="polite">Cultivando sua floresta…</p>
		{:else if visibleTrees.length === 0}
			<div class="lv-forest__empty">
				<p>Nenhuma árvore cultivada neste período.</p>
				<span>Inicie um ciclo de foco no Pomodoro para plantar sua primeira árvore!</span>
			</div>
		{:else}
			<ul class="lv-forest__grid" role="list" aria-label="Árvores cultivadas">
				{#each visibleTrees as tree (tree.sessionId)}
					<li class="lv-forest__tree-wrapper">
						<button
							type="button"
							class="lv-forest__tree-item"
							class:is-withered={!tree.completed}
							aria-label={tree.label}
							onmouseenter={() => (hoveredTree = tree)}
							onmouseleave={() => (hoveredTree = null)}
							onfocus={() => (hoveredTree = tree)}
							onblur={() => (hoveredTree = null)}
						>
							<ForestTreeSvg stage={tree.stage} size={52} />
							<span class="lv-forest__tree-name">{tree.stageName}</span>
							<span class="lv-forest__tree-time">{tree.minutes} min</span>
						</button>
					</li>
				{/each}
			</ul>
		{/if}

		{#if hoveredTree}
			<div class="lv-forest__tooltip" role="tooltip" aria-live="polite">
				<strong>{hoveredTree.stageName}</strong>
				<span>{hoveredTree.label}</span>
			</div>
		{/if}
	</div>
</div>

<style>
	.lv-forest {
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
		padding: 1.5rem;
		background: var(--bg-surface, rgba(255, 255, 255, 0.03));
		border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
		border-radius: var(--radius-lg, 0.75rem);
	}

	.lv-forest__header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
	}

	.lv-forest__title {
		font-size: 1.25rem;
		font-weight: 700;
		margin: 0;
		color: var(--text-primary, currentColor);
	}

	.lv-forest__subtitle {
		font-size: 0.875rem;
		margin: 0.25rem 0 0;
		color: var(--text-muted, rgba(255, 255, 255, 0.6));
	}

	.lv-forest__filters {
		display: flex;
		gap: 0.375rem;
		background: var(--bg-subtle, rgba(255, 255, 255, 0.05));
		padding: 0.25rem;
		border-radius: var(--radius-md, 0.5rem);
	}

	.lv-forest__filters button.is-active {
		background: var(--accent, #2563eb);
		color: #ffffff;
	}

	.lv-forest__stats {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
		gap: 0.75rem;
	}

	.lv-forest__stat-card {
		display: flex;
		flex-direction: column;
		padding: 0.75rem 1rem;
		background: var(--bg-card, rgba(255, 255, 255, 0.04));
		border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
		border-radius: var(--radius-md, 0.5rem);
	}

	.lv-forest__stat-value {
		font-size: 1.375rem;
		font-weight: 700;
		color: var(--color-success, #4caf50);
	}

	.lv-forest__stat-value.is-withered {
		color: var(--text-muted, #78909c);
	}

	.lv-forest__stat-label {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--text-muted, rgba(255, 255, 255, 0.6));
		margin-top: 0.25rem;
	}

	.lv-forest__canvas {
		position: relative;
		min-height: 180px;
		background: var(--bg-inset, rgba(0, 0, 0, 0.15));
		border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.06));
		border-radius: var(--radius-md, 0.5rem);
		padding: 1.25rem;
	}

	.lv-forest__grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(90px, 1fr));
		gap: 1rem;
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
		padding: 0.75rem 0.5rem;
		border-radius: var(--radius-md, 0.5rem);
		background: var(--bg-tile, rgba(255, 255, 255, 0.02));
		border: 1px solid transparent;
		color: inherit;
		font: inherit;
		cursor: default;
		transition: transform 0.15s ease, border-color 0.15s ease, background-color 0.15s ease;
	}

	.lv-forest__tree-item:hover,
	.lv-forest__tree-item:focus-visible {
		transform: translateY(-2px);
		border-color: var(--accent, #2563eb);
		background: var(--bg-tile-hover, rgba(255, 255, 255, 0.06));
		outline: none;
	}

	.lv-forest__tree-item.is-withered {
		opacity: 0.75;
	}

	.lv-forest__tree-name {
		font-size: 0.75rem;
		font-weight: 600;
		text-align: center;
		margin-top: 0.5rem;
		color: var(--text-primary, currentColor);
	}

	.lv-forest__tree-time {
		font-size: 0.7rem;
		color: var(--text-muted, rgba(255, 255, 255, 0.5));
	}

	.lv-forest__empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 150px;
		text-align: center;
		color: var(--text-muted, rgba(255, 255, 255, 0.6));
		font-size: 0.875rem;
		gap: 0.25rem;
	}

	.lv-forest__tooltip {
		position: absolute;
		bottom: 1rem;
		left: 50%;
		transform: translateX(-50%);
		background: var(--bg-popover, #1e293b);
		color: var(--text-contrast, #ffffff);
		padding: 0.5rem 0.875rem;
		border-radius: var(--radius-sm, 0.375rem);
		font-size: 0.8125rem;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
		border: 1px solid var(--border, rgba(255, 255, 255, 0.15));
		pointer-events: none;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.125rem;
		white-space: nowrap;
		z-index: 10;
	}
</style>
