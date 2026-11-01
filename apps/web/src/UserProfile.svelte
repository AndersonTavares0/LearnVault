<script lang="ts">
	import { transport, type User } from './transport';

	let {
		user,
		avatarVersion = 0,
		onAvatarChange = () => {}
	}: {
		user: User;
		avatarVersion?: number;
		onAvatarChange?: () => void;
	} = $props();

	let fileInput = $state<HTMLInputElement | null>(null);
	let selectedFile = $state<File | null>(null);
	let previewUrl = $state<string | null>(null);
	let avatarFailed = $state(false);
	let busy = $state(false);
	let message = $state('');
	let error = $state('');

	const userInitials = $derived(
		user.username
			.trim()
			.split(/[\s._-]+/)
			.filter(Boolean)
			.slice(0, 2)
			.map((word) => word[0]?.toUpperCase() ?? '')
			.join('') || '?'
	);

	const avatarSrc = $derived(
		previewUrl ?? (transport.userAvatarUrl(avatarVersion) || null)
	);

	const hasCustomAvatar = $derived(!avatarFailed && !!transport.userAvatarUrl(avatarVersion));

	$effect(() => {
		if (avatarVersion && !previewUrl) {
			avatarFailed = false;
		}
	});

	$effect(() => {
		return () => {
			if (previewUrl) URL.revokeObjectURL(previewUrl);
		};
	});

	function handleFileSelected(event: Event) {
		error = '';
		message = '';
		const input = event.currentTarget as HTMLInputElement;
		const file = input.files?.[0] ?? null;
		if (!file) return;

		const validTypes = ['image/png', 'image/jpeg', 'image/webp'];
		if (!validTypes.includes(file.type.toLowerCase())) {
			error = 'Formato de imagem não suportado. Escolha um arquivo PNG, JPEG ou WebP.';
			input.value = '';
			return;
		}

		const MAX_SIZE = 2 * 1024 * 1024;
		if (file.size > MAX_SIZE) {
			error = 'A imagem é maior que 2 MiB. Escolha uma foto menor.';
			input.value = '';
			return;
		}

		selectedFile = file;
		if (previewUrl) URL.revokeObjectURL(previewUrl);
		previewUrl = URL.createObjectURL(file);
		avatarFailed = false;
	}

	function cancelSelection() {
		if (previewUrl) URL.revokeObjectURL(previewUrl);
		previewUrl = null;
		selectedFile = null;
		if (fileInput) fileInput.value = '';
		error = '';
	}

	async function saveAvatar() {
		if (!selectedFile || busy) return;
		busy = true;
		error = '';
		message = '';
		try {
			await transport.uploadUserAvatar(selectedFile);
			if (previewUrl) URL.revokeObjectURL(previewUrl);
			previewUrl = null;
			selectedFile = null;
			if (fileInput) fileInput.value = '';
			avatarFailed = false;
			message = 'Foto de perfil atualizada com sucesso.';
			onAvatarChange();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível salvar o avatar.';
		} finally {
			busy = false;
		}
	}

	async function removeAvatar() {
		if (busy) return;
		busy = true;
		error = '';
		message = '';
		try {
			await transport.deleteUserAvatar();
			cancelSelection();
			avatarFailed = true;
			message = 'Foto de perfil removida com sucesso.';
			onAvatarChange();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível remover o avatar.';
		} finally {
			busy = false;
		}
	}
</script>

<div class="lv-user-profile">
	<div class="lv-user-profile__card">
		<div class="lv-user-profile__avatar-wrap">
			{#if avatarSrc && !avatarFailed}
				<img
					class="lv-user-profile__img"
					src={avatarSrc}
					alt="Avatar de {user.username}"
					onerror={() => (avatarFailed = true)}
				/>
			{:else}
				<span class="lv-user-profile__initials" aria-hidden="true">{userInitials}</span>
			{/if}
		</div>
		<div class="lv-user-profile__details">
			<div class="lv-user-profile__name">{user.username}</div>
			<div class="lv-user-profile__role">{user.isAdmin ? 'Administrador' : 'Estudante'}</div>
			<div class="lv-user-profile__hint">Formatos suportados: PNG, JPEG ou WebP (máx. 2 MiB).</div>
		</div>
	</div>

	<input
		type="file"
		accept="image/png,image/jpeg,image/webp"
		bind:this={fileInput}
		onchange={handleFileSelected}
		class="visually-hidden"
		id="user-avatar-file-input"
		tabindex="-1"
		aria-hidden="true"
	/>

	<div class="lv-user-profile__actions">
		{#if selectedFile}
			<button class="lvui-btn lvui-btn-primary" type="button" onclick={saveAvatar} disabled={busy} aria-busy={busy}>
				{busy ? 'Salvando…' : 'Salvar nova foto'}
			</button>
			<button class="lvui-btn lvui-btn-ghost" type="button" onclick={cancelSelection} disabled={busy}>
				Cancelar
			</button>
		{:else}
			<button class="lvui-btn" type="button" onclick={() => fileInput?.click()} disabled={busy}>
				<svg viewBox="0 0 24 24" aria-hidden="true" width="16" height="16">
					<path d="M4 7h4l2-3h4l2 3h4a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"/>
					<circle cx="12" cy="13" r="3.5" fill="none" stroke="currentColor" stroke-width="1.8"/>
				</svg>
				Escolher foto
			</button>
			{#if hasCustomAvatar}
				<button class="lvui-btn lvui-btn-ghost" type="button" onclick={removeAvatar} disabled={busy} aria-busy={busy}>
					Remover foto
				</button>
			{/if}
		{/if}
	</div>

	{#if error}<p class="lv-form-error" role="alert">{error}</p>{/if}
	{#if message}<p class="lv-form-notice" role="status">{message}</p>{/if}
</div>

<style>
	.lv-user-profile {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.lv-user-profile__card {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 16px;
		border-radius: 12px;
		border: 1px solid var(--line-weak, rgba(0, 0, 0, 0.08));
		background: var(--surface-2, rgba(0, 0, 0, 0.02));
	}

	.lv-user-profile__avatar-wrap {
		width: 64px;
		height: 64px;
		border-radius: 50%;
		overflow: hidden;
		display: grid;
		place-items: center;
		border: 2px solid var(--line-strong, rgba(0, 0, 0, 0.15));
		background: var(--surface-3, rgba(0, 0, 0, 0.04));
		flex-shrink: 0;
	}

	.lv-user-profile__img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}

	.lv-user-profile__initials {
		font-family: var(--display, sans-serif);
		font-size: 22px;
		font-weight: 700;
		color: var(--ink, inherit);
	}

	.lv-user-profile__details {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.lv-user-profile__name {
		font-size: 16px;
		font-weight: 600;
		color: var(--ink, inherit);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.lv-user-profile__role {
		font-size: 13px;
		color: var(--muted, #64748b);
	}

	.lv-user-profile__hint {
		font-size: 11px;
		color: var(--muted, #64748b);
		margin-top: 4px;
	}

	.lv-user-profile__actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		align-items: center;
	}

	.lv-form-error {
		color: var(--danger, #e5484d);
		font-size: 13px;
		margin: 0;
	}

	.lv-form-notice {
		color: var(--accent, #0284c7);
		font-size: 13px;
		margin: 0;
	}
</style>
