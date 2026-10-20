<script lang="ts">
	import PasswordField from './PasswordField.svelte';
	import { transport, type User } from './transport';

	let { user, onUpdated }: { user: User; onUpdated: (user: User) => void } = $props();

	let newUsername = $state('');
	let currentPassword = $state('');
	let newPassword = $state('');
	let confirmPassword = $state('');
	let busy = $state(false);
	let apiError = $state('');
	let success = $state(0);
	$effect(() => {
		if (!newUsername) newUsername = user.username;
	});

	// Ordena o foco pelo formulário; valida igualdade exata, sem trim.
	const mismatch = $derived(confirmPassword !== '' && confirmPassword !== newPassword ? 'As senhas não coincidem.' : '');
	const invalidFields = $derived.by(() => {
		const invalid: string[] = [];
		const name = newUsername.trim();
		if (Array.from(name).length < 2 || Array.from(name).length > 64 || /\p{Cc}/u.test(name) || name !== name.trim()) invalid.push('settings-new-username');
		if (currentPassword.length < 1 || currentPassword.length > 1024) invalid.push('settings-current-password');
		const newLength = Array.from(newPassword).length;
		if (newLength < 8 || newLength > 128) invalid.push('settings-new-password');
		const confirmLength = Array.from(confirmPassword).length;
		if (confirmLength < 8 || confirmLength > 128 || (confirmPassword !== '' && confirmPassword !== newPassword)) invalid.push('settings-confirm-password');
		return invalid;
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		apiError = '';
		const form = event.currentTarget as HTMLFormElement;
		if (!form.checkValidity() || invalidFields.length > 0) {
			form.reportValidity();
			const first = document.getElementById(invalidFields[0] ?? '') as HTMLElement | null;
			first?.focus();
			return;
		}
		busy = true;
		try {
			const updated = await transport.updateCredentials(newUsername.trim(), newPassword, currentPassword);
			onUpdated(updated);
			currentPassword = '';
			newPassword = '';
			confirmPassword = '';
			success += 1;
		} catch (cause) {
			apiError = cause instanceof Error ? cause.message : 'Não foi possível atualizar as credenciais.';
		} finally {
			busy = false;
		}
	}
</script>

{#if success > 0}
	<p class="inline-status" role="status">Credenciais atualizadas. Outras sessões foram encerradas.</p>
{/if}
{#if apiError}
	<p class="inline-status error" role="alert" tabindex="-1">{apiError}</p>
{/if}
<form class="credentials-form" onsubmit={submit} aria-busy={busy}>
	<fieldset class="auth-fields" disabled={busy}>
		<legend>Credenciais de acesso</legend>
		<label class="form-field" for="settings-new-username">Nome de usuário
			<input id="settings-new-username" name="username" autocomplete="username" autocapitalize="none" spellcheck={false} bind:value={newUsername} required />
		</label>
		<PasswordField id="settings-current-password" name="current-password" label="Senha atual" autocomplete="current-password" bind:value={currentPassword} maxLength={1024} hint="Necessária para confirmar a alteração." />
		<PasswordField id="settings-new-password" name="new-password" label="Nova senha" autocomplete="new-password" bind:value={newPassword} minLength={8} maxLength={128} hint="Use de 8 a 128 caracteres. Você pode usar uma frase." />
		<PasswordField id="settings-confirm-password" name="confirm-password" label="Confirmar nova senha" autocomplete="new-password" bind:value={confirmPassword} minLength={8} maxLength={128} error={mismatch || undefined} />
		<p class="small credentials-note">A alteração encerra as outras sessões. Você continuará conectado neste navegador.</p>
		<button class="btn primary" type="submit" disabled={busy} aria-busy={busy}>{busy ? 'Salvando…' : 'Atualizar credenciais'}</button>
	</fieldset>
</form>
