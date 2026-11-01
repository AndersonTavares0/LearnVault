<script lang="ts">
	import {
		BOTANICAL_SPECIES,
		type BotanicalSpecies,
		type BotanicalSpeciesId
	} from './forest-catalog';
	import {
		nursery,
		setActiveSeed,
		unlockBotanicalSpecies
	} from './nursery-store';
	import ForestTreeSvg from './ForestTreeSvg.svelte';

	let { onclose }: { onclose?: () => void } = $props();

	let notification = $state<string | null>(null);

	function handleSelect(speciesId: BotanicalSpeciesId) {
		const ok = setActiveSeed(speciesId);
		if (ok) {
			const s = BOTANICAL_SPECIES.find((item) => item.id === speciesId);
			notification = `Semente de ${s?.name ?? speciesId} selecionada para os próximos ciclos de foco!`;
		}
	}

	function handleUnlock(species: BotanicalSpecies) {
		const ok = unlockBotanicalSpecies(species.id);
		if (ok) {
			notification = `Parabéns! Você desbloqueou o ${species.name} com sucesso.`;
		}
	}
</script>

<div class="lv-nursery" role="region" aria-label="Viveiro Botânico de Espécies">
	<!-- Cabeçalho do Viveiro -->
	<div class="lv-nursery__head">
		<div class="lv-nursery__titles">
			<div class="lv-nursery__badge-strip">
				<span class="lv-nursery__chip">Viveiro Botânico</span>
				<div class="lv-nursery__balance" aria-label="Saldo atual: {$nursery.dropsBalance} gotas de foco">
					<svg class="lv-nursery__drop-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
						<path d="M12 2.69l5.66 5.66a8 8 0 1 1-11.31 0z" />
					</svg>
					<strong>{$nursery.dropsBalance}</strong>
					<span>Gotas de Foco</span>
				</div>
			</div>
			<h2 class="lv-nursery__title">Cultive Espécies Únicas</h2>
			<p class="lv-nursery__desc">
				Cada ciclo de foco concluído sem interrupção rende gotas de orvalho. Use suas gotas para
				desbloquear espécies raras e enriquecer seu bosque pessoal.
			</p>
		</div>
		{#if onclose}
			<button type="button" class="lvui-btn lvui-btn-outline" onclick={onclose}>
				Voltar ao Bosque
			</button>
		{/if}
	</div>

	{#if notification}
		<div class="lv-nursery__alert" role="status" aria-live="polite">
			<span>{notification}</span>
			<button type="button" class="lv-nursery__alert-close" aria-label="Fechar aviso" onclick={() => (notification = null)}>×</button>
		</div>
	{/if}

	<!-- Grade de Espécies Botânicas -->
	<div class="lv-nursery__grid" role="list">
		{#each BOTANICAL_SPECIES as species (species.id)}
			{@const isUnlocked = $nursery.unlockedSpecies.includes(species.id)}
			{@const isActive = $nursery.activeSeed === species.id}
			{@const canAfford = $nursery.dropsBalance >= species.cost}
			{@const deficit = species.cost - $nursery.dropsBalance}

			<div
				class="lv-nursery__card"
				class:is-unlocked={isUnlocked}
				class:is-active={isActive}
				role="listitem"
				style="--species-color: {species.accentColor}"
			>
				<div class="lv-nursery__card-visual">
					<ForestTreeSvg stage={species.id} size={64} />
					{#if isActive}
						<span class="lv-nursery__active-tag">Ativa no Pomodoro</span>
					{/if}
				</div>

				<div class="lv-nursery__card-body">
					<div class="lv-nursery__card-header">
						<h3 class="lv-nursery__species-name">{species.name}</h3>
						<span class="lv-nursery__species-minutes">{species.minMinutes} min recomendados</span>
					</div>

					<p class="lv-nursery__species-lore">{species.lore}</p>

					<div class="lv-nursery__card-footer">
						{#if isUnlocked}
							{#if isActive}
								<button class="lvui-btn lvui-btn-sm" type="button" disabled aria-pressed="true">
									✓ Semente Selecionada
								</button>
							{:else}
								<button class="lvui-btn lvui-btn-sm lvui-btn-outline" type="button" onclick={() => handleSelect(species.id)}>
									Plantar no próximo foco
								</button>
							{/if}
						{:else}
							<div class="lv-nursery__cost-info">
								<span class="lv-nursery__cost-value">
									<svg class="lv-nursery__cost-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 2.69l5.66 5.66a8 8 0 1 1-11.31 0z"/></svg>
									{species.cost} gotas
								</span>
								{#if canAfford}
									<button class="lvui-btn lvui-btn-sm lvui-btn-primary" type="button" onclick={() => handleUnlock(species)}>
										Desbloquear
									</button>
								{:else}
									<button class="lvui-btn lvui-btn-sm" type="button" disabled title="Faltam {deficit} gotas">
										Faltam {deficit} gotas
									</button>
								{/if}
							</div>
						{/if}
					</div>
				</div>
			</div>
		{/each}
	</div>
</div>

<style>
	.lv-nursery {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
		padding: 1.5rem;
		background: var(--surface);
		border: 1px solid var(--line);
		border-radius: 12px;
	}

	.lv-nursery__head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 1.5rem;
		flex-wrap: wrap;
	}

	.lv-nursery__titles {
		flex: 1;
		min-width: 280px;
	}

	.lv-nursery__badge-strip {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		margin-bottom: 0.5rem;
		flex-wrap: wrap;
	}

	.lv-nursery__chip {
		font-size: 0.75rem;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		background: color-mix(in srgb, #16a34a 12%, transparent);
		color: #16a34a;
		padding: 0.2rem 0.6rem;
		border-radius: 9999px;
		border: 1px solid color-mix(in srgb, #16a34a 25%, transparent);
	}

	.lv-nursery__balance {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.8125rem;
		background: color-mix(in srgb, #0284c7 12%, transparent);
		color: #0284c7;
		padding: 0.2rem 0.6rem;
		border-radius: 9999px;
		border: 1px solid color-mix(in srgb, #0284c7 25%, transparent);
	}

	.lv-nursery__drop-icon,
	.lv-nursery__cost-icon {
		width: 14px;
		height: 14px;
	}

	.lv-nursery__title {
		font-size: 1.25rem;
		font-weight: 700;
		color: var(--text);
		margin: 0 0 0.35rem;
	}

	.lv-nursery__desc {
		margin: 0;
		font-size: 0.875rem;
		color: var(--muted);
		max-width: 600px;
		line-height: 1.45;
	}

	.lv-nursery__alert {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.75rem 1rem;
		background: color-mix(in srgb, #16a34a 12%, var(--surface));
		border: 1px solid color-mix(in srgb, #16a34a 30%, var(--line));
		border-radius: 8px;
		font-size: 0.875rem;
		color: var(--text);
	}

	.lv-nursery__alert-close {
		background: transparent;
		border: none;
		color: var(--muted);
		font-size: 1.125rem;
		cursor: pointer;
		padding: 0 0.25rem;
	}

	.lv-nursery__grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: 1.25rem;
	}

	.lv-nursery__card {
		background: var(--surface-2);
		border: 1px solid var(--line);
		border-radius: 12px;
		padding: 1.25rem;
		display: flex;
		flex-direction: column;
		gap: 1rem;
		transition: border-color 0.2s ease, transform 0.2s ease;
	}

	.lv-nursery__card.is-active {
		border-color: var(--species-color);
		box-shadow: 0 0 0 2px color-mix(in srgb, var(--species-color) 25%, transparent);
	}

	.lv-nursery__card-visual {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 1rem;
		background: color-mix(in srgb, var(--surface) 60%, transparent);
		border: 1px solid var(--line);
		border-radius: 8px;
		min-height: 100px;
		position: relative;
	}

	.lv-nursery__active-tag {
		position: absolute;
		top: 0.5rem;
		right: 0.5rem;
		font-size: 0.6875rem;
		font-weight: 700;
		color: var(--species-color);
		background: color-mix(in srgb, var(--species-color) 12%, var(--surface));
		border: 1px solid color-mix(in srgb, var(--species-color) 30%, transparent);
		padding: 0.15rem 0.45rem;
		border-radius: 9999px;
	}

	.lv-nursery__card-body {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		flex: 1;
	}

	.lv-nursery__card-header {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}

	.lv-nursery__species-name {
		font-size: 1rem;
		font-weight: 700;
		margin: 0;
		color: var(--text);
	}

	.lv-nursery__species-minutes {
		font-size: 0.75rem;
		color: var(--muted);
	}

	.lv-nursery__species-lore {
		font-size: 0.8125rem;
		color: var(--muted);
		margin: 0;
		line-height: 1.4;
		flex: 1;
	}

	.lv-nursery__card-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding-top: 0.5rem;
		border-top: 1px solid var(--line);
	}

	.lv-nursery__cost-info {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		gap: 0.5rem;
	}

	.lv-nursery__cost-value {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		font-size: 0.8125rem;
		font-weight: 600;
		color: #0284c7;
	}
</style>
