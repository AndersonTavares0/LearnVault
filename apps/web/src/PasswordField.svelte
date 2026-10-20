<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';

	type Props = {
		id: string;
		label: string;
		autocomplete: HTMLInputAttributes['autocomplete'];
		value: string;
		placeholder?: string;
		minLength?: number;
		maxLength?: number;
		hint?: string;
		error?: string;
		/** name do input; por padrão o próprio id, preservando formulários existentes. */
		name?: string;
	};

	let { id, label, autocomplete, value = $bindable(''), placeholder, minLength, maxLength, hint, error, name = id }: Props = $props();
	let visible = $state(false);
	let input = $state<HTMLInputElement | null>(null);
	let touched = $state(false);

	const length = $derived.by(() => {
		let count = 0;
		if ((minLength !== undefined || maxLength !== undefined) && value) {
			for (const _ of value) count += 1;
		}
		return count;
	});
	const localError = $derived.by(() => {
		if (!value) return '';
		if (minLength !== undefined && length < minLength) return `Use ao menos ${minLength} caracteres.`;
		if (maxLength !== undefined && length > maxLength) return `Use no máximo ${maxLength} caracteres.`;
		return '';
	});
	const exposedError = $derived(error ?? localError);

	$effect(() => {
		// A validade conta caracteres Unicode por iteração; minlength/maxlength do HTML
		// contam unidades UTF-16 e não representam esses limites.
		input?.setCustomValidity(touched && exposedError ? exposedError : '');
	});
</script>

<div class="password-field">
	<label for={id}>{label}</label>
	<div class="password-input">
		<input
			{id}
			{name}
			type={visible ? 'text' : 'password'}
			{autocomplete}
			{placeholder}
			bind:value
			bind:this={input}
			required
			aria-invalid={(touched && exposedError) || error ? 'true' : undefined}
			aria-describedby={[hint ? `${id}-hint` : null, exposedError ? `${id}-error` : null].filter(Boolean).join(' ') || undefined}
			onblur={() => (touched = true)}
			oninput={() => { if (touched) input?.setCustomValidity(''); }}
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
	{#if exposedError}<p id={`${id}-error`} class="password-error" role="alert">{exposedError}</p>{/if}
</div>

<style>
	.password-error { margin: 4px 0 0; color: var(--danger); font-size: 14px; line-height: 20px; }
</style>
