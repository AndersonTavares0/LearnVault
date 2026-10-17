<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	type Props = {
		id: string;
		label: string;
		autocomplete: HTMLInputAttributes['autocomplete'];
		value: string;
		minLength?: number;
		hint?: string;
	};

	let { id, label, autocomplete, value = $bindable(''), minLength, hint }: Props = $props();
	let visible = $state(false);
</script>

<div class="password-field">
	<label for={id}>{label}</label>
	<div class="password-input">
		<input
			{id}
			type={visible ? 'text' : 'password'}
			{autocomplete}
			bind:value
			required
			maxlength="1024"
			minlength={minLength}
			aria-describedby={hint ? `${id}-hint` : undefined}
		/>
		<button
			type="button"
			class="password-toggle"
			aria-pressed={visible}
			aria-label={visible ? `Ocultar ${label}` : `Mostrar ${label}`}
			aria-controls={id}
			onclick={() => (visible = !visible)}
		>
			<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false">
				{#if visible}
					<path d="m3 3 18 18M10.6 10.6a2 2 0 0 0 2.8 2.8M9.9 5.2A11 11 0 0 1 12 5c5 0 9 7 9 7a18 18 0 0 1-3.1 3.9M6.3 6.3A21 21 0 0 0 3 12s4 7 9 7a10 10 0 0 0 4.3-1" />
				{:else}
					<path d="M3 12s4-7 9-7 9 7 9 7-4 7-9 7-9-7-9-7Z" />
					<circle cx="12" cy="12" r="3" />
				{/if}
			</svg>
		</button>
	</div>
	{#if hint}<p id={`${id}-hint`} class="password-hint">{hint}</p>{/if}
</div>
