<script lang="ts">
	import {
		calculateGrowthPhase,
		getGuardianPhrase,
		getSpeciesById,
		type BotanicalSpeciesId
	} from './forest-catalog';
	import ForestTreeSvg from './ForestTreeSvg.svelte';

	let {
		speciesId = 'oak',
		remainingSeconds,
		totalSeconds,
		clock,
		compact = false
	}: {
		speciesId?: BotanicalSpeciesId;
		remainingSeconds: number;
		totalSeconds: number;
		clock: string;
		compact?: boolean;
	} = $props();

	const species = $derived(getSpeciesById(speciesId));
	const phase = $derived(calculateGrowthPhase(remainingSeconds, totalSeconds));
	const elapsed = $derived(Math.max(0, totalSeconds - remainingSeconds));
	const progressRatio = $derived(totalSeconds > 0 ? Math.min(1, Math.max(0, elapsed / totalSeconds)) : 1);
	const progressPercent = $derived(Math.round(progressRatio * 100));
	const guardianPhrase = $derived(getGuardianPhrase(speciesId, phase));

	const phaseLabels = [
		'Semente no solo',
		'Broto rompendo a terra',
		'Copa jovem se ramificando',
		'Árvore madura florescendo'
	];
</script>

<div
	class="lv-growth-stage"
	class:is-compact={compact}
	role="region"
	aria-label="Crescimento em tempo real: {species.name}"
>
	<!-- Visual Botânico Animado -->
	<div class="lv-growth-stage__visual-wrap">
		<div class="lv-growth-stage__canvas phase-{phase}">
			{#if phase === 0}
				<!-- Fase 0: Semente fincada no solo fértil -->
				<svg class="lv-growth-stage__sprout" viewBox="0 0 64 64" fill="none" aria-hidden="true">
					<ellipse cx="32" cy="52" rx="18" ry="5" fill="currentColor" opacity="0.2" />
					<!-- Solo -->
					<path d="M12 52Q32 48 52 52" stroke="#5d4037" stroke-width="2.5" stroke-linecap="round" />
					<!-- Semente -->
					<ellipse cx="32" cy="46" rx="6" ry="8" fill="#795548" stroke="#4e342e" stroke-width="1.5" />
					<path d="M32 40Q35 34 32 30" stroke="#84cc16" stroke-width="2" stroke-linecap="round" />
					<!-- Raiz -->
					<path d="M32 54Q30 58 32 60" stroke="#8d6e63" stroke-width="1.5" stroke-linecap="round" />
				</svg>
			{:else if phase === 1}
				<!-- Fase 1: Pequena muda com folhas jovens -->
				<svg class="lv-growth-stage__sprout" viewBox="0 0 64 64" fill="none" aria-hidden="true">
					<ellipse cx="32" cy="54" rx="16" ry="4" fill="currentColor" opacity="0.18" />
					<path d="M32 54V32" stroke="#5d4037" stroke-width="3" stroke-linecap="round" />
					<path d="M32 42C24 42 22 34 32 36" fill="#84cc16" stroke="#4d7c0f" stroke-width="1.5" />
					<path d="M32 38C40 38 42 30 32 32" fill="#a3e635" stroke="#4d7c0f" stroke-width="1.5" />
					<circle cx="32" cy="30" r="3" fill="#65a30d" />
				</svg>
			{:else if phase === 2}
				<!-- Fase 2: Caule espesso com copa em formação -->
				<svg class="lv-growth-stage__sprout" viewBox="0 0 64 64" fill="none" aria-hidden="true">
					<ellipse cx="32" cy="56" rx="18" ry="4" fill="currentColor" opacity="0.18" />
					<path d="M30 54V34M34 54V36M32 34L26 26M32 34L38 26" stroke="#5d4037" stroke-width="3" stroke-linecap="round" />
					<circle cx="26" cy="24" r="8" fill="#16a34a" opacity="0.9" />
					<circle cx="38" cy="24" r="8" fill="#16a34a" opacity="0.9" />
					<circle cx="32" cy="18" r="9" fill="#22c55e" />
				</svg>
			{:else}
				<!-- Fase 3: Árvore completa da espécie selecionada -->
				<ForestTreeSvg stage={species.id} size={compact ? 56 : 72} />
			{/if}
		</div>

		<div class="lv-growth-stage__info">
			<div class="lv-growth-stage__header">
				<span class="lv-growth-stage__species-tag" style="--species-color: {species.accentColor}">
					{species.name}
				</span>
				<span class="lv-growth-stage__clock">{clock}</span>
			</div>

			<!-- Barra de Progresso Botânica -->
			<div
				class="lv-growth-stage__progress"
				role="progressbar"
				aria-valuenow={progressPercent}
				aria-valuemin={0}
				aria-valuemax={100}
				aria-valuetext="{progressPercent}% concluído · {phaseLabels[phase]}"
			>
				<div class="lv-growth-stage__fill" style="width: {progressPercent}%; --species-color: {species.accentColor}"></div>
			</div>

			<div class="lv-growth-stage__meta">
				<span class="lv-growth-stage__phase-name">{phaseLabels[phase]} ({progressPercent}%)</span>
				{#if !compact}
					<p class="lv-growth-stage__phrase" aria-live="polite">{guardianPhrase}</p>
				{/if}
			</div>
		</div>
	</div>
</div>

<style>
	.lv-growth-stage {
		background: color-mix(in srgb, var(--surface) 95%, transparent);
		border: 1px solid var(--line);
		border-radius: 12px;
		padding: 1rem 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		transition: border-color 0.2s ease, box-shadow 0.2s ease;
	}

	.lv-growth-stage.is-compact {
		padding: 0.75rem 1rem;
		gap: 0.5rem;
	}

	.lv-growth-stage__visual-wrap {
		display: flex;
		align-items: center;
		gap: 1.25rem;
	}

	.lv-growth-stage__canvas {
		width: 72px;
		height: 72px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--surface-2) 80%, transparent);
		border: 1px solid var(--line);
		border-radius: 12px;
		flex-shrink: 0;
		overflow: hidden;
		transition: transform 0.3s ease;
	}

	.lv-growth-stage.is-compact .lv-growth-stage__canvas {
		width: 56px;
		height: 56px;
	}

	.lv-growth-stage__sprout {
		width: 48px;
		height: 48px;
	}

	.lv-growth-stage__info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.375rem;
	}

	.lv-growth-stage__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
	}

	.lv-growth-stage__species-tag {
		font-size: 0.8125rem;
		font-weight: 600;
		color: var(--species-color);
		background: color-mix(in srgb, var(--species-color) 12%, transparent);
		padding: 0.125rem 0.5rem;
		border-radius: 9999px;
		border: 1px solid color-mix(in srgb, var(--species-color) 25%, transparent);
	}

	.lv-growth-stage__clock {
		font-family: var(--font-mono, monospace);
		font-size: 0.875rem;
		font-weight: 700;
		color: var(--text);
	}

	.lv-growth-stage__progress {
		height: 6px;
		background: var(--surface-2);
		border-radius: 9999px;
		overflow: hidden;
		border: 1px solid var(--line);
	}

	.lv-growth-stage__fill {
		height: 100%;
		background: linear-gradient(90deg, #16a34a, var(--species-color, #16a34a));
		border-radius: 9999px;
		transition: width 0.3s ease;
	}

	.lv-growth-stage__meta {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}

	.lv-growth-stage__phase-name {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--text-2);
	}

	.lv-growth-stage__phrase {
		margin: 0;
		font-size: 0.8125rem;
		color: var(--muted);
		font-style: italic;
		line-height: 1.35;
	}

	@media (prefers-reduced-motion: reduce) {
		.lv-growth-stage__canvas,
		.lv-growth-stage__fill {
			transition: none;
		}
	}
</style>
