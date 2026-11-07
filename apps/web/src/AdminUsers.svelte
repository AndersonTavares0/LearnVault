<script lang="ts">
	import { onMount } from 'svelte';
	import PasswordField from './PasswordField.svelte';
	import { courseIdOf, type CatalogNode, type LocalUser, type Transport } from './transport';

	let {
		transport,
		courses = []
	}: {
		transport: Transport;
		courses?: CatalogNode[];
	} = $props();

	let users = $state<LocalUser[]>([]);
	let loading = $state(true);
	let error = $state('');
	let statusMessage = $state('');

	let newUsername = $state('');
	let newPassword = $state('');
	let newUnrestricted = $state(true);
	let newSelectedCourses = $state<string[]>([]);
	let creating = $state(false);

	let busyUserId = $state<string | null>(null);
	let confirmDeleteUserId = $state<string | null>(null);
	let draftUnrestricted = $state<Record<string, boolean>>({});
	let draftCourses = $state<Record<string, string[]>>({});

	function syncDrafts(list: LocalUser[]) {
		const nextUnrestricted: Record<string, boolean> = {};
		const nextCourses: Record<string, string[]> = {};
		for (const u of list) {
			nextUnrestricted[u.userId] = u.allowedCourseIds === null;
			nextCourses[u.userId] = u.allowedCourseIds ? [...u.allowedCourseIds] : courses.map((c) => courseIdOf(c));
		}
		draftUnrestricted = nextUnrestricted;
		draftCourses = nextCourses;
	}

	async function loadUsers() {
		if (!transport.listUsers) {
			loading = false;
			return;
		}
		loading = true;
		error = '';
		try {
			const list = await transport.listUsers();
			users = list;
			syncDrafts(list);
		} catch (e) {
			error = e instanceof Error ? e.message : 'Não foi possível carregar os usuários locais.';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		void loadUsers();
	});

	function toggleNewCourse(courseId: string, checked: boolean) {
		if (checked) {
			if (!newSelectedCourses.includes(courseId)) {
				newSelectedCourses = [...newSelectedCourses, courseId];
			}
		} else {
			newSelectedCourses = newSelectedCourses.filter((id) => id !== courseId);
		}
	}

	function toggleDraftCourse(userId: string, courseId: string, checked: boolean) {
		const current = draftCourses[userId] ?? [];
		const next = checked
			? current.includes(courseId)
				? current
				: [...current, courseId]
			: current.filter((id) => id !== courseId);
		draftCourses = { ...draftCourses, [userId]: next };
	}

	async function handleCreateUser(event: SubmitEvent) {
		event.preventDefault();
		if (!transport.createUser || creating) return;
		error = '';
		statusMessage = '';
		creating = true;
		try {
			const allowed = newUnrestricted ? null : newSelectedCourses;
			const created = await transport.createUser(newUsername.trim(), newPassword, allowed);
			const next = [...users, created];
			users = next;
			syncDrafts(next);
			newUsername = '';
			newPassword = '';
			newUnrestricted = true;
			newSelectedCourses = [];
			statusMessage = `Conta "${created.username}" criada com sucesso.`;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Não foi possível criar o usuário.';
		} finally {
			creating = false;
		}
	}

	async function handleSavePermissions(user: LocalUser) {
		if (!transport.setUserCourseAccess || busyUserId) return;
		error = '';
		statusMessage = '';
		busyUserId = user.userId;
		try {
			const unrestricted = draftUnrestricted[user.userId] ?? true;
			const allowed = unrestricted ? null : (draftCourses[user.userId] ?? []);
			const saved = await transport.setUserCourseAccess(user.userId, allowed);
			users = users.map((item) =>
				item.userId === user.userId ? { ...item, allowedCourseIds: saved } : item
			);
			statusMessage = `Permissões de "${user.username}" atualizadas.`;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Não foi possível salvar as permissões.';
		} finally {
			busyUserId = null;
		}
	}

	async function handleDeleteUser(user: LocalUser) {
		if (!transport.deleteUser || busyUserId) return;
		error = '';
		statusMessage = '';
		busyUserId = user.userId;
		try {
			await transport.deleteUser(user.userId);
			users = users.filter((item) => item.userId !== user.userId);
			confirmDeleteUserId = null;
			statusMessage = `Conta "${user.username}" removida.`;
		} catch (e) {
			error = e instanceof Error ? e.message : 'Não foi possível remover o usuário.';
		} finally {
			busyUserId = null;
		}
	}
</script>

<section class="lv-panel lv-settings__panel" aria-labelledby="users-heading">
	<div class="lv-panel__head">
		<h2 id="users-heading">Usuários locais e permissões de cursos</h2>
		<span class="lv-count">{users.length} {users.length === 1 ? 'conta' : 'contas'}</span>
	</div>
	<div class="lv-settings__body lv-admin-users">
		<p class="lv-notice">
			Crie contas locais para estudantes e defina quais cursos cada pessoa pode acessar. O progresso, anotações, flashcards e favoritos ficam isolados por conta.
		</p>

		{#if error}<p class="error" role="alert">{error}</p>{/if}
		{#if statusMessage}<p class="lv-notice" role="status">{statusMessage}</p>{/if}

		{#if loading}
			<p class="lv-notice">Carregando usuários…</p>
		{:else}
			<ul class="lv-admin-users__list" aria-label="Contas locais cadastradas">
				{#each users as item (item.userId)}
					<li class="lv-admin-users__item">
						<div class="lv-admin-users__row">
							<div class="lv-admin-users__identity">
								<strong>{item.username}</strong>
								<span class="lv-admin-users__badge" class:is-admin={item.isAdmin}>
									{item.isAdmin ? 'Administrador' : 'Estudante'}
								</span>
								{#if !item.isAdmin}
									<span class="lv-admin-users__access">
										{item.allowedCourseIds === null
											? 'Todos os cursos'
											: `${item.allowedCourseIds.length} ${item.allowedCourseIds.length === 1 ? 'curso liberado' : 'cursos liberados'}`}
									</span>
								{/if}
							</div>
							{#if !item.isAdmin}
								<div class="lv-admin-users__actions">
									{#if confirmDeleteUserId === item.userId}
										<button
											class="text-button danger"
											type="button"
											disabled={busyUserId === item.userId}
											onclick={() => handleDeleteUser(item)}
										>
											Confirmar remoção
										</button>
										<button
											class="text-button"
											type="button"
											disabled={busyUserId === item.userId}
											onclick={() => (confirmDeleteUserId = null)}
										>
											Cancelar
										</button>
									{:else}
										<button
											class="text-button danger"
											type="button"
											disabled={busyUserId === item.userId}
											onclick={() => (confirmDeleteUserId = item.userId)}
										>
											Remover conta
										</button>
									{/if}
								</div>
							{/if}
						</div>

						{#if !item.isAdmin}
							<details class="lv-admin-users__permissions">
								<summary>Configurar acesso a cursos</summary>
								<div class="lv-admin-users__perm-body">
									<label class="lv-admin-users__check">
										<input
											type="checkbox"
											checked={draftUnrestricted[item.userId] ?? true}
											onchange={(e) => {
												draftUnrestricted = {
													...draftUnrestricted,
													[item.userId]: (e.currentTarget as HTMLInputElement).checked
												};
											}}
										/>
										<span>Liberar acesso a todos os cursos</span>
									</label>

									{#if !(draftUnrestricted[item.userId] ?? true)}
										{#if courses.length === 0}
											<p class="lv-notice">Nenhum curso importado na biblioteca no momento.</p>
										{:else}
											<div class="lv-admin-users__courses" role="group" aria-label={`Cursos permitidos para ${item.username}`}>
												{#each courses as c (courseIdOf(c))}
													{@const cid = courseIdOf(c)}
													<label class="lv-admin-users__check">
														<input
															type="checkbox"
															checked={(draftCourses[item.userId] ?? []).includes(cid)}
															onchange={(e) =>
																toggleDraftCourse(
																	item.userId,
																	cid,
																	(e.currentTarget as HTMLInputElement).checked
																)}
														/>
														<span>{c.title}</span>
													</label>
												{/each}
											</div>
										{/if}
									{/if}

									<div>
										<button
											class="lv-btn lv-btn--primary"
											type="button"
											disabled={busyUserId === item.userId}
											onclick={() => handleSavePermissions(item)}
										>
											{busyUserId === item.userId ? 'Salvando…' : 'Salvar permissões'}
										</button>
									</div>
								</div>
							</details>
						{/if}
					</li>
				{/each}
			</ul>
		{/if}

		<form class="lv-form lv-admin-users__create" onsubmit={handleCreateUser}>
			<fieldset class="lv-fieldset" disabled={creating}>
				<legend class="lv-admin-users__legend">Nova conta de estudante</legend>
				<label for="new-student-username">
					<span>Nome de usuário</span>
					<input
						id="new-student-username"
						name="new-student-username"
						autocomplete="off"
						bind:value={newUsername}
						minlength="2"
						maxlength="64"
						required
					/>
				</label>
				<PasswordField
					id="new-student-password"
					label="Senha inicial"
					autocomplete="new-password"
					bind:value={newPassword}
					minLength={12}
					hint="Mínimo de 12 caracteres."
				/>
				<label class="lv-admin-users__check">
					<input type="checkbox" bind:checked={newUnrestricted} />
					<span>Liberar acesso a todos os cursos</span>
				</label>
				{#if !newUnrestricted}
					{#if courses.length === 0}
						<p class="lv-notice">Nenhum curso importado ainda.</p>
					{:else}
						<div class="lv-admin-users__courses" role="group" aria-label="Cursos liberados para a nova conta">
							{#each courses as c (courseIdOf(c))}
								{@const cid = courseIdOf(c)}
								<label class="lv-admin-users__check">
									<input
										type="checkbox"
										checked={newSelectedCourses.includes(cid)}
										onchange={(e) =>
											toggleNewCourse(cid, (e.currentTarget as HTMLInputElement).checked)}
									/>
									<span>{c.title}</span>
								</label>
							{/each}
						</div>
					{/if}
				{/if}
				<button class="lv-btn lv-btn--primary" type="submit" disabled={creating} aria-busy={creating}>
					{creating ? 'Criando conta…' : 'Criar conta de estudante'}
				</button>
			</fieldset>
		</form>
	</div>
</section>

<style>
	.lv-admin-users {
		display: flex;
		flex-direction: column;
		gap: 1rem;
	}
	.lv-admin-users__list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.65rem;
	}
	.lv-admin-users__item {
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		padding: 0.75rem 0.9rem;
		background: var(--surface-subtle, transparent);
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}
	.lv-admin-users__row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		flex-wrap: wrap;
	}
	.lv-admin-users__identity {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
	}
	.lv-admin-users__badge {
		font-size: 0.75rem;
		padding: 0.15rem 0.5rem;
		border-radius: 999px;
		background: var(--surface-elevated, rgba(128, 128, 128, 0.15));
		color: var(--muted);
	}
	.lv-admin-users__badge.is-admin {
		color: var(--accent);
		font-weight: 600;
	}
	.lv-admin-users__access {
		font-size: 0.82rem;
		color: var(--muted);
	}
	.lv-admin-users__actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}
	.lv-admin-users__permissions summary {
		cursor: pointer;
		font-size: 0.85rem;
		color: var(--muted);
		user-select: none;
	}
	.lv-admin-users__perm-body {
		margin-top: 0.65rem;
		display: flex;
		flex-direction: column;
		gap: 0.65rem;
	}
	.lv-admin-users__courses {
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		padding-left: 0.5rem;
		border-left: 2px solid var(--border);
	}
	.lv-admin-users__check {
		display: inline-flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.9rem;
		cursor: pointer;
	}
	.lv-admin-users__create {
		border-top: 1px solid var(--border);
		padding-top: 1rem;
	}
	.lv-admin-users__legend {
		font-weight: 600;
		margin-bottom: 0.5rem;
	}
</style>
