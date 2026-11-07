<script lang="ts">
	import { treeStageName, type TreeStage } from './forest';
	import ForestTreeSvg from './ForestTreeSvg.svelte';
	import { tr } from './i18n.svelte';

	let { speciesCounts }: { speciesCounts?: Record<TreeStage, number> } = $props();
</script>

<details class="lv-forest__guide">
	<summary class="lv-forest__guide-summary">
		<svg viewBox="0 0 24 24" fill="none" width="16" height="16" stroke="currentColor" aria-hidden="true">
			<circle cx="12" cy="12" r="10" stroke-width="1.8" />
			<path d="M12 16v-4M12 8h.01" stroke-width="2" stroke-linecap="round" />
		</svg>
		<span>{tr('How to blossom species in the study forest', 'Como florescer espécies na floresta de estudos')}</span>
	</summary>
	<div class="lv-forest__guide-content">
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="sapling" size={34} />
			<div class="lv-forest__species-info">
				<strong>{treeStageName('sapling')}</strong>
				<span>{tr('Focus session of 1 to 20 min', 'Sessão de foco de 1 a 20 min')}</span>
			</div>
			{#if speciesCounts && speciesCounts.sapling > 0}
				<span class="lv-forest__species-count" title={tr(`${speciesCounts.sapling} grown in period`, `${speciesCounts.sapling} cultivados no período`)}>
					{speciesCounts.sapling}
				</span>
			{/if}
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="oak" size={34} />
			<div class="lv-forest__species-info">
				<strong>{treeStageName('oak')}</strong>
				<span>{tr('Focus session of 21 to 40 min', 'Sessão de foco de 21 a 40 min')}</span>
			</div>
			{#if speciesCounts && speciesCounts.oak > 0}
				<span class="lv-forest__species-count" title={tr(`${speciesCounts.oak} grown in period`, `${speciesCounts.oak} cultivados no período`)}>
					{speciesCounts.oak}
				</span>
			{/if}
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="pine" size={34} />
			<div class="lv-forest__species-info">
				<strong>{treeStageName('pine')}</strong>
				<span>{tr('Focus session of 41 to 60 min', 'Sessão de foco de 41 a 60 min')}</span>
			</div>
			{#if speciesCounts && speciesCounts.pine > 0}
				<span class="lv-forest__species-count" title={tr(`${speciesCounts.pine} grown in period`, `${speciesCounts.pine} cultivados no período`)}>
					{speciesCounts.pine}
				</span>
			{/if}
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="sakura" size={34} />
			<div class="lv-forest__species-info">
				<strong>{treeStageName('sakura')}</strong>
				<span>{tr('Unlockable in Nursery (80 drops)', 'Desbloqueável no Viveiro (80 gotas)')}</span>
			</div>
			{#if speciesCounts && speciesCounts.sakura > 0}
				<span class="lv-forest__species-count" title={tr(`${speciesCounts.sakura} grown in period`, `${speciesCounts.sakura} cultivadas no período`)}>
					{speciesCounts.sakura}
				</span>
			{/if}
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="willow" size={34} />
			<div class="lv-forest__species-info">
				<strong>{treeStageName('willow')}</strong>
				<span>{tr('Unlockable in Nursery (120 drops)', 'Desbloqueável no Viveiro (120 gotas)')}</span>
			</div>
			{#if speciesCounts && speciesCounts.willow > 0}
				<span class="lv-forest__species-count" title={tr(`${speciesCounts.willow} grown in period`, `${speciesCounts.willow} cultivados no período`)}>
					{speciesCounts.willow}
				</span>
			{/if}
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="ginkgo" size={34} />
			<div class="lv-forest__species-info">
				<strong>{treeStageName('ginkgo')}</strong>
				<span>{tr('Unlockable in Nursery (200 drops)', 'Desbloqueável no Viveiro (200 gotas)')}</span>
			</div>
			{#if speciesCounts && speciesCounts.ginkgo > 0}
				<span class="lv-forest__species-count" title={tr(`${speciesCounts.ginkgo} grown in period`, `${speciesCounts.ginkgo} cultivados no período`)}>
					{speciesCounts.ginkgo}
				</span>
			{/if}
		</div>
		<div class="lv-forest__species-card">
			<ForestTreeSvg stage="ancient" size={34} />
			<div class="lv-forest__species-info">
				<strong>{treeStageName('ancient')}</strong>
				<span>{tr('Deep focus session (+60 min)', 'Sessão de foco profundo (+60 min)')}</span>
			</div>
			{#if speciesCounts && speciesCounts.ancient > 0}
				<span class="lv-forest__species-count" title={tr(`${speciesCounts.ancient} grown in period`, `${speciesCounts.ancient} cultivados no período`)}>
					{speciesCounts.ancient}
				</span>
			{/if}
		</div>
		<div class="lv-forest__species-card is-withered">
			<ForestTreeSvg stage="withered" size={34} />
			<div class="lv-forest__species-info">
				<strong>{treeStageName('withered')}</strong>
				<span>{tr('Session abandoned before completion', 'Sessão abandonada antes do fim')}</span>
			</div>
			{#if speciesCounts && speciesCounts.withered > 0}
				<span class="lv-forest__species-count is-withered" title={tr(`${speciesCounts.withered} interrupted in period`, `${speciesCounts.withered} interrompidas no período`)}>
					{speciesCounts.withered}
				</span>
			{/if}
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
		position: relative;
	}

	.lv-forest__species-card.is-withered {
		border-style: dashed;
	}

	.lv-forest__species-info {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
		min-width: 0;
		flex: 1;
	}

	.lv-forest__species-info strong {
		font-size: 0.75rem;
		color: var(--ink);
	}

	.lv-forest__species-info span {
		font-size: 0.6875rem;
		color: var(--muted);
	}

	.lv-forest__species-count {
		font-size: 0.6875rem;
		font-family: var(--mono);
		font-weight: 700;
		color: var(--ok);
		background: color-mix(in srgb, var(--ok) 12%, transparent);
		border: 1px solid color-mix(in srgb, var(--ok) 25%, transparent);
		padding: 0.1rem 0.4rem;
		border-radius: 9999px;
		white-space: nowrap;
	}

	.lv-forest__species-count.is-withered {
		color: var(--muted);
		background: color-mix(in srgb, var(--muted) 12%, transparent);
		border-color: color-mix(in srgb, var(--muted) 25%, transparent);
	}
</style>
