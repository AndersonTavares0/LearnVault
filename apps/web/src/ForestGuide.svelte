<script lang="ts">
	import ForestTreeSvg from './ForestTreeSvg.svelte';
</script>

<details class="lv-forest__guide">
	<summary class="lv-forest__guide-summary">
		<svg viewBox="0 0 24 24" fill="none" width="16" height="16" stroke="currentColor" aria-hidden="true">
			<circle cx="12" cy="12" r="10" stroke-width="1.8" />
			<path d="M12 16v-4M12 8h.01" stroke-width="2" stroke-linecap="round" />
		</svg>
		<span>Como florescer espécies na floresta de estudos</span>
	</summary>
	<div class="lv-forest__guide-content">
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="sapling" size={34} />
			<div class="lv-forest__species-info">
				<strong>Brotinho</strong>
				<span>Sessão de foco de 1 a 20 min</span>
			</div>
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="oak" size={34} />
			<div class="lv-forest__species-info">
				<strong>Carvalho Frondoso</strong>
				<span>Sessão de foco de 21 a 40 min</span>
			</div>
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="pine" size={34} />
			<div class="lv-forest__species-info">
				<strong>Pinheiro Nobre</strong>
				<span>Sessão de foco de 41 a 60 min</span>
			</div>
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="ancient" size={34} />
			<div class="lv-forest__species-info">
				<strong>Árvore Ancestral</strong>
				<span>Sessão de foco profundo (+60 min)</span>
			</div>
		</div>
		<div class="lv-forest__species-card is-withered">
			<ForestTreeSvg stage="withered" size={34} />
			<div class="lv-forest__species-info">
				<strong>Árvore Seca</strong>
				<span>Sessão abandonada antes do fim</span>
			</div>
		</div>
	</div>
</details>

<style>
	.lv-forest__guide {
		border-top: 1px solid var(--line);
		padding-top: 0.875rem;
	}

	.lv-forest__guide-summary {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--muted);
		cursor: pointer;
		user-select: none;
		transition: color 0.15s ease;
	}

	.lv-forest__guide-summary:hover {
		color: var(--ink);
	}

	.lv-forest__guide-content {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
		gap: 0.625rem;
		margin-top: 0.75rem;
	}

	.lv-forest__species-card {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0.75rem;
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: var(--radius);
	}

	.lv-forest__species-card.is-withered {
		border-style: dashed;
	}

	.lv-forest__species-info {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		min-width: 0;
	}

	.lv-forest__species-info strong {
		font-size: 0.75rem;
		color: var(--ink);
	}

	.lv-forest__species-info span {
		font-size: 0.6875rem;
		color: var(--muted);
	}
</style>
