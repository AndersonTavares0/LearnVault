<script lang="ts">
	import { onMount } from 'svelte';
	import { formatClock } from './pomodoro';
	import { findLessonLocation, href } from './router';
	import { transport, type Catalog, type StudyNote } from './transport';

	let { catalog }: { catalog: Catalog | null } = $props();

	let notes = $state<StudyNote[]>([]);
	let loading = $state(true);
	let error = $state('');
	let query = $state('');
	let editingId = $state<string | null>(null);
	let editContent = $state('');
	let saving = $state(false);

	const enrichedNotes = $derived(
		notes.map((note) => {
			const location = findLessonLocation(catalog, note.mediaId);
			const courseTitle = location?.courseTitle ?? 'Curso';
			const lessonTitle = location?.lesson.title ?? note.relativePath ?? 'Aula';
			const courseId = location?.courseId ?? null;
			const lessonPath = location?.lesson.path ?? null;
			const targetHref = courseId && lessonPath ? href('course', courseId, lessonPath, note.positionSeconds) : null;
			return {
				...note,
				courseTitle,
				lessonTitle,
				courseId,
				lessonPath,
				targetHref
			};
		})
	);

	const filteredNotes = $derived(
		enrichedNotes.filter((note) => {
			if (!query.trim()) return true;
			const q = query.toLowerCase();
			return (
				note.content.toLowerCase().includes(q) ||
				note.courseTitle.toLowerCase().includes(q) ||
				note.lessonTitle.toLowerCase().includes(q)
			);
		})
	);

	async function loadNotes() {
		loading = true;
		error = '';
		try {
			notes = await transport.listNotes();
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível carregar as anotações.';
		} finally {
			loading = false;
		}
	}

	function startEdit(note: StudyNote) {
		editingId = note.noteId;
		editContent = note.content;
	}

	function cancelEdit() {
		editingId = null;
		editContent = '';
	}

	async function saveEdit(noteId: string) {
		const trimmed = editContent.trim();
		if (!trimmed || saving) return;
		saving = true;
		try {
			const updated = await transport.updateNote(noteId, trimmed);
			notes = notes.map((n) => (n.noteId === noteId ? { ...n, content: updated.content, updatedAt: updated.updatedAt } : n));
			editingId = null;
			editContent = '';
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível atualizar a anotação.';
		} finally {
			saving = false;
		}
	}

	async function deleteNote(noteId: string) {
		if (saving) return;
		saving = true;
		try {
			await transport.deleteNote(noteId);
			notes = notes.filter((n) => n.noteId !== noteId);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível excluir a anotação.';
		} finally {
			saving = false;
		}
	}

	function formatDate(timestampSeconds: number): string {
		if (!timestampSeconds) return '';
		return new Date(timestampSeconds * 1000).toLocaleDateString('pt-BR', {
			day: '2-digit',
			month: 'short',
			year: 'numeric'
		});
	}

	onMount(() => {
		void loadNotes();
	});
</script>

<div class="lv-all-notes" role="region" aria-label="Todas as anotações">
	<header class="lv-all-notes__header">
		<div>
			<h1 class="lv-all-notes__title">Minhas Anotações</h1>
			<p class="lv-all-notes__subtitle">Anotações salvas durante suas aulas em todos os cursos.</p>
		</div>
		<div class="lv-all-notes__search">
			<input
				class="lvui-input"
				type="search"
				placeholder="Buscar em anotações, cursos ou aulas…"
				aria-label="Buscar anotações"
				bind:value={query}
			/>
		</div>
	</header>

	{#if error}
		<p class="lv-form-error" role="alert">{error}</p>
	{/if}

	{#if loading}
		<p class="lv-all-notes__loading" aria-live="polite">Carregando suas anotações…</p>
	{:else if filteredNotes.length === 0}
		<div class="lv-all-notes__empty">
			{#if query.trim()}
				<p>Nenhuma anotação encontrada para "{query}".</p>
			{:else}
				<p>Você ainda não possui anotações registradas.</p>
				<span>Assista a uma aula no player e adicione anotações vinculadas aos momentos importantes do vídeo.</span>
			{/if}
		</div>
	{:else}
		<ul class="lv-all-notes__list" role="list">
			{#each filteredNotes as note (note.noteId)}
				<li class="lv-all-notes__card">
					<div class="lv-all-notes__card-head">
						<div class="lv-all-notes__card-context">
							<span class="lv-all-notes__course">{note.courseTitle}</span>
							<span class="lv-all-notes__divider" aria-hidden="true">•</span>
							<span class="lv-all-notes__lesson">{note.lessonTitle}</span>
						</div>
						<div class="lv-all-notes__card-meta">
							<span class="lv-all-notes__timestamp" title="Posição no vídeo">{formatClock(note.positionSeconds)}</span>
							<span class="lv-all-notes__date">{formatDate(note.updatedAt || note.createdAt)}</span>
						</div>
					</div>

					{#if editingId === note.noteId}
						<div class="lv-all-notes__edit-box">
							<textarea
								class="lvui-input lv-all-notes__textarea"
								rows="3"
								aria-label="Editar anotação"
								bind:value={editContent}
								disabled={saving}
							></textarea>
							<div class="lv-all-notes__edit-actions">
								<button
									class="lvui-btn lvui-btn-sm lvui-btn-primary"
									type="button"
									disabled={saving || !editContent.trim()}
									onclick={() => saveEdit(note.noteId)}
								>
									Salvar
								</button>
								<button
									class="lvui-btn lvui-btn-sm"
									type="button"
									disabled={saving}
									onclick={cancelEdit}
								>
									Cancelar
								</button>
							</div>
						</div>
					{:else}
						<p class="lv-all-notes__content">{note.content}</p>
					{/if}

					<div class="lv-all-notes__card-foot">
						{#if note.targetHref}
							<a
								class="lvui-btn lvui-btn-sm lvui-btn-primary"
								href={note.targetHref}
								title="Abrir aula e posicionar player nesta anotação"
							>
								Ir para a aula ({formatClock(note.positionSeconds)})
							</a>
						{/if}
						<div class="lv-all-notes__card-buttons">
							{#if editingId !== note.noteId}
								<button
									class="lvui-btn lvui-btn-sm"
									type="button"
									onclick={() => startEdit(note)}
									aria-label="Editar anotação"
								>
									Editar
								</button>
								<button
									class="lvui-btn lvui-btn-sm lvui-btn-danger"
									type="button"
									onclick={() => deleteNote(note.noteId)}
									aria-label="Excluir anotação"
								>
									Excluir
								</button>
							{/if}
						</div>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.lv-all-notes {
		display: flex;
		flex-direction: column;
		gap: 1.5rem;
		max-width: 900px;
		margin: 0 auto;
		padding: 1.5rem 1rem 3rem;
	}

	.lv-all-notes__header {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
	}

	.lv-all-notes__title {
		font-size: 1.75rem;
		font-weight: 700;
		margin: 0;
		color: var(--text-primary, currentColor);
	}

	.lv-all-notes__subtitle {
		font-size: 0.9375rem;
		margin: 0.25rem 0 0;
		color: var(--text-muted, rgba(255, 255, 255, 0.6));
	}

	.lv-all-notes__search {
		min-width: 280px;
		flex: 1;
		max-width: 400px;
	}

	.lv-all-notes__loading,
	.lv-all-notes__empty {
		text-align: center;
		padding: 3rem 1rem;
		background: var(--bg-surface, rgba(255, 255, 255, 0.03));
		border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
		border-radius: var(--radius-lg, 0.75rem);
		color: var(--text-muted, rgba(255, 255, 255, 0.6));
	}

	.lv-all-notes__empty p {
		font-size: 1.0625rem;
		font-weight: 600;
		margin: 0 0 0.5rem;
		color: var(--text-primary, currentColor);
	}

	.lv-all-notes__empty span {
		font-size: 0.875rem;
	}

	.lv-all-notes__list {
		display: flex;
		flex-direction: column;
		gap: 1rem;
		list-style: none;
		margin: 0;
		padding: 0;
	}

	.lv-all-notes__card {
		display: flex;
		flex-direction: column;
		gap: 0.875rem;
		padding: 1.25rem;
		background: var(--bg-surface, rgba(255, 255, 255, 0.03));
		border: 1px solid var(--border, rgba(255, 255, 255, 0.1));
		border-radius: var(--radius-lg, 0.75rem);
		transition: border-color 0.15s ease;
	}

	.lv-all-notes__card:hover {
		border-color: var(--accent, #2563eb);
	}

	.lv-all-notes__card-head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
	}

	.lv-all-notes__card-context {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 0.875rem;
	}

	.lv-all-notes__course {
		font-weight: 600;
		color: var(--accent, #2563eb);
	}

	.lv-all-notes__divider {
		color: var(--text-muted, rgba(255, 255, 255, 0.4));
	}

	.lv-all-notes__lesson {
		color: var(--text-primary, currentColor);
	}

	.lv-all-notes__card-meta {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		font-size: 0.8125rem;
	}

	.lv-all-notes__timestamp {
		background: var(--bg-subtle, rgba(255, 255, 255, 0.08));
		padding: 0.125rem 0.5rem;
		border-radius: var(--radius-sm, 0.25rem);
		font-family: monospace;
		font-weight: 600;
	}

	.lv-all-notes__date {
		color: var(--text-muted, rgba(255, 255, 255, 0.5));
	}

	.lv-all-notes__content {
		font-size: 0.9375rem;
		line-height: 1.5;
		margin: 0;
		white-space: pre-wrap;
		color: var(--text-primary, currentColor);
	}

	.lv-all-notes__edit-box {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.lv-all-notes__textarea {
		width: 100%;
		resize: vertical;
		font-family: inherit;
	}

	.lv-all-notes__edit-actions {
		display: flex;
		gap: 0.5rem;
	}

	.lv-all-notes__card-foot {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		padding-top: 0.75rem;
		border-top: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.06));
	}

	.lv-all-notes__card-buttons {
		display: flex;
		gap: 0.5rem;
	}
</style>
