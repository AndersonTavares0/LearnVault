<script lang="ts">
	import type { ForestEcosystem } from './forest';
	import ForestBiomeCrest from './ForestBiomeCrest.svelte';

	let { ecosystem, healthyTrees }: { ecosystem: ForestEcosystem; healthyTrees: number } = $props();
</script>

<div class="lv-ecosystem-band" role="status" aria-label="Ecossistema: {ecosystem.name}">
	<div class="lv-ecosystem-band__left">
		<ForestBiomeCrest level={ecosystem.level} size={42} />
		<div class="lv-ecosystem-band__info">
			<div class="lv-ecosystem-band__badge">
				<span class="lv-ecosystem-band__tag">Nível {ecosystem.level}</span>
				<strong class="lv-ecosystem-band__title">{ecosystem.name}</strong>
			</div>
			<p class="lv-ecosystem-band__desc">{ecosystem.description}</p>
		</div>
	</div>

	{#if ecosystem.nextThreshold !== null}
		<div class="lv-ecosystem-band__progress">
			<div class="lv-ecosystem-band__labels">
				<span>Próximo estágio: <strong>{ecosystem.nextName}</strong></span>
				<span class="lv-ecosystem-band__count">{healthyTrees} de {ecosystem.nextThreshold} sadias</span>
			</div>
			<div
				class="lv-ecosystem-band__track"
				role="progressbar"
				aria-label="Progresso para {ecosystem.nextName}"
				aria-valuemin="0"
				aria-valuemax="100"
				aria-valuenow={Math.round(ecosystem.progress * 100)}
			>
				<span
					class="lv-ecosystem-band__fill"
					style:width={`${Math.round(ecosystem.progress * 100)}%`}
				></span>
			</div>
		</div>
	{:else}
		<div class="lv-ecosystem-band__max">
			<span aria-hidden="true">★</span>
			<span>Ápice florestal alcançado</span>
		</div>
	{/if}
</div>

<style>
	.lv-ecosystem-band {
		background: color-mix(in srgb, var(--surface) 90%, transparent);
		border: 1px solid var(--line);
		border-radius: var(--radius);
		padding: 0.875rem 1.125rem;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1.25rem;
		flex-wrap: wrap;
	}

	.lv-ecosystem-band__left {
		display: flex;
		align-items: center;
		gap: 0.875rem;
		min-width: 240px;
		flex: 1;
	}

	.lv-ecosystem-band__info {
		display: flex;
		flex-direction: column;
		gap: 0.15rem;
	}

	.lv-ecosystem-band__badge {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.lv-ecosystem-band__tag {
		font-size: 0.6875rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		background: color-mix(in srgb, var(--accent) 15%, transparent);
		color: var(--accent);
		padding: 0.15rem 0.45rem;
		border-radius: 9999px;
	}

	.lv-ecosystem-band__title {
		font-size: 0.9375rem;
		font-weight: 700;
		color: var(--text);
	}

	.lv-ecosystem-band__desc {
		margin: 0;
		font-size: 0.75rem;
		color: var(--muted);
		max-width: 480px;
		line-height: 1.35;
	}

	.lv-ecosystem-band__progress {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
		min-width: 200px;
		max-width: 320px;
		flex: 1;
	}

	.lv-ecosystem-band__labels {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 0.5rem;
		font-size: 0.75rem;
		color: var(--text-2);
	}

	.lv-ecosystem-band__count {
		font-family: var(--mono);
		font-size: 0.6875rem;
		color: var(--muted);
	}

	.lv-ecosystem-band__track {
		height: 6px;
		background: var(--surface-2);
		border-radius: 9999px;
		overflow: hidden;
		border: 1px solid var(--line);
	}

	.lv-ecosystem-band__fill {
		display: block;
		height: 100%;
		background: linear-gradient(90deg, #16a34a, #22c55e);
		border-radius: 9999px;
		transition: width 0.4s ease;
	}

	.lv-ecosystem-band__max {
		display: flex;
		align-items: center;
		gap: 0.375rem;
		font-size: 0.8125rem;
		font-weight: 600;
		color: #eab308;
		background: color-mix(in srgb, #eab308 10%, transparent);
		padding: 0.35rem 0.75rem;
		border-radius: 9999px;
		border: 1px solid color-mix(in srgb, #eab308 25%, transparent);
	}

	@media (max-width: 640px) {
		.lv-ecosystem-band {
			flex-direction: column;
			align-items: flex-start;
		}

		.lv-ecosystem-band__progress {
			width: 100%;
			max-width: none;
		}
	}
</style>
