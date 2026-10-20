<script lang="ts">
	let { title, description, level = 2 as 2 | 3, tone = 'neutral' as 'neutral' | 'error', action = null as null | { label: string; href?: string; onClick?: () => void } }: {
		title: string;
		description: string;
		level?: 2 | 3;
		tone?: 'neutral' | 'error';
		action?: null | { label: string; href?: string; onClick?: () => void };
	} = $props();
</script>

<section class="empty-state" class:error={tone === 'error'} role={tone === 'error' ? 'alert' : 'status'}>
	{#if level === 2}
		<h2>{title}</h2>
	{:else}
		<h3>{title}</h3>
	{/if}
	<p>{description}</p>
	{#if action}
		{#if action.href}
			<a class="btn primary" href={action.href}>{action.label}</a>
		{:else}
			<button class="btn primary" type="button" onclick={action.onClick}>{action.label}</button>
		{/if}
	{/if}
</section>

<style>
	.empty-state { display: grid; justify-items: start; gap: 8px; max-width: 60ch; padding: 0; background: transparent; border: 0; }
	.empty-state h2, .empty-state h3 { margin: 0; font-size: 20px; line-height: 28px; font-weight: 650; color: var(--ink); }
	.empty-state p { margin: 0; color: var(--muted); font-size: 16px; line-height: 24px; overflow-wrap: anywhere; }
	.empty-state.error { padding-left: 16px; border-left: 3px solid var(--danger); }
	.empty-state .btn { justify-self: start; margin-top: 8px; }
	:global(.analytics .empty-state) { margin: 0 0 24px; }
	:global(main.home .empty-state) { margin: 8px 0 24px; }
</style>
