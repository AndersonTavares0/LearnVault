<script lang="ts">
	let {
		itemTitle,
		index,
		count,
		busy = false,
		onMove,
		onReset
	}: {
		itemTitle: string;
		index: number;
		count: number;
		busy?: boolean;
		onMove: (offset: -1 | 1) => void;
		onReset: () => void;
	} = $props();
</script>

<div class="catalog-order-controls" aria-label={`Ordem de ${itemTitle}`}>
	<button type="button" class="text-button" aria-label={`Mover ${itemTitle} para cima`} onclick={() => onMove(-1)} disabled={busy || index === 0}>↑</button>
	<button type="button" class="text-button" aria-label={`Mover ${itemTitle} para baixo`} onclick={() => onMove(1)} disabled={busy || index === count - 1}>↓</button>
	{#if index === 0}<button type="button" class="text-button" onclick={onReset} disabled={busy}>Restaurar ordem automática</button>{/if}
</div>
