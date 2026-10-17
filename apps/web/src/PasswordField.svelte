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
			onclick={() => (visible = !visible)}
		>
			{visible ? 'Ocultar' : 'Mostrar'}
		</button>
	</div>
	{#if hint}<p id={`${id}-hint`} class="password-hint">{hint}</p>{/if}
</div>
