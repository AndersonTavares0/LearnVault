<script lang="ts">
	import type { ForestTree, TreeStage } from './forest';
	import ForestTreeSvg from './ForestTreeSvg.svelte';

	let {
		tree,
		onclose
	}: {
		tree: ForestTree;
		onclose: () => void;
	} = $props();

	function stageDescription(stage: TreeStage): string {
		switch (stage) {
			case 'sapling':
				return 'Ciclo rápido de foco concluído (até 20 minutos).';
			case 'oak':
				return 'Ciclo padrão de foco concluído (21 a 40 minutos).';
			case 'pine':
				return 'Ciclo estendido de foco concluído (41 a 60 minutos).';
			case 'sakura':
				return 'Cerejeira cultivada com dedicação e harmonia visual.';
			case 'willow':
				return 'Salgueiro cultivado com resiliência e paciência nos estudos.';
			case 'ginkgo':
				return 'Ginkgo dourado cultivado para memorização profunda.';
			case 'ancient':
				return 'Ciclo de foco profundo e imersão (+60 minutos).';
			case 'withered':
				return 'Ciclo interrompido ou abandonado antes do término.';
			default:
				return 'Árvore cultivada na floresta de estudos.';
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
</script>

<div class="lv-forest__inspector" role="status" aria-live="polite">
	<div class="lv-forest__inspector-body">
		<div class="lv-forest__inspector-visual" aria-hidden="true">
			<ForestTreeSvg stage={tree.stage} size={48} />
		</div>
		<div class="lv-forest__inspector-details">
			<div class="lv-forest__inspector-head">
				<strong class="lv-forest__inspector-title">{tree.stageName}</strong>
				<span
					class="lv-forest__status-pill"
					class:is-healthy={tree.completed}
					class:is-withered={!tree.completed}
				>
					{tree.completed ? 'Sadia' : 'Seca'}
				</span>
				<span class="lv-forest__inspector-time">{tree.minutes} min de foco</span>
			</div>
			<p class="lv-forest__inspector-desc">
				{stageDescription(tree.stage)}
				{#if tree.timestamp}
					<span class="lv-forest__inspector-date">· Cultivada em {formatDateTime(tree.timestamp)}</span>
				{/if}
			</p>
		</div>
	</div>
	<button
		type="button"
		class="lv-forest__inspector-close"
		aria-label="Fechar detalhes da árvore"
		onclick={onclose}
	>
		✕
	</button>
</div>

<style>
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
</style>
