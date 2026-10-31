<script lang="ts">
	import { formatDuration } from './player';
	import { transport, type StudyNote } from './transport';

	let {
		mediaId,
		currentTime = 0,
		onSeek = () => undefined
	}: {
		mediaId: string | null;
		currentTime: number;
		onSeek: (seconds: number) => void;
	} = $props();

	let notes: StudyNote[] = $state([]);
	let loading = $state(false);
	let error = $state('');
	let isAdding = $state(false);
	let noteContent = $state('');
	let notePosition = $state(0);
	let saving = $state(false);

	$effect(() => {
		const id = mediaId;
		if (!id) {
			notes = [];
			error = '';
			return;
		}
		let active = true;
		loading = true;
		error = '';
		transport
			.listNotes(id)
			.then((items) => {
				if (active) notes = items;
			})
			.catch((cause) => {
				if (active) error = cause instanceof Error ? cause.message : 'Erro ao carregar anotações.';
			})
			.finally(() => {
				if (active) loading = false;
			});

		return () => {
			active = false;
		};
	});

	function startAdding() {
		notePosition = Math.max(0, Math.floor(currentTime));
		noteContent = '';
		isAdding = true;
	}

	function cancelAdding() {
		isAdding = false;
		noteContent = '';
	}

	async function saveNote() {
		if (!mediaId || saving) return;
		const trimmed = noteContent.trim();
		if (!trimmed) return;

		saving = true;
		error = '';
		try {
			const created = await transport.createNote(mediaId, notePosition, trimmed);
			notes = [...notes, created].sort((a, b) => a.positionSeconds - b.positionSeconds);
			isAdding = false;
			noteContent = '';
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Falha ao salvar anotação.';
		} finally {
			saving = false;
		}
	}

	async function deleteNote(noteId: string) {
		if (saving) return;
		try {
			await transport.deleteNote(noteId);
			notes = notes.filter((n) => n.noteId !== noteId);
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Falha ao remover anotação.';
		}
	}

	function handleKeydown(event: KeyboardEvent) {
		if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
			event.preventDefault();
			void saveNote();
		}
	}
</script>

<section class="study-notes-section" aria-label="Anotações e marcadores de estudo">
	<div class="study-notes-header">
		<div class="study-notes-title-group">
			<h3 class="study-notes-title">
				<svg class="study-notes-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
					<path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
					<path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
				</svg>
				Anotações da aula
			</h3>
			<span class="study-notes-count">{notes.length}</span>
		</div>

		{#if !isAdding && mediaId}
			<button class="lv-btn lvui-btn lv-btn--subtle" type="button" onclick={startAdding}>
				<svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
					<line x1="12" y1="5" x2="12" y2="19" />
					<line x1="5" y1="12" x2="19" y2="12" />
				</svg>
				Anotar em {formatDuration(Math.floor(currentTime))}
			</button>
		{/if}
	</div>

	{#if error}
		<p class="study-notes-error" role="alert">{error}</p>
	{/if}

	{#if isAdding}
		<div class="study-notes-form">
			<div class="form-header">
				<span class="timestamp-badge">Marcador: {formatDuration(notePosition)}</span>
				<span class="form-hint">Dica: pressione Ctrl+Enter para salvar</span>
			</div>
			<!-- svelte-ignore a11y_autofocus -->
			<textarea
				class="study-notes-input"
				placeholder="Digite sua reflexão, dúvida ou resumo para este momento do vídeo..."
				rows="3"
				maxlength="10000"
				autofocus
				bind:value={noteContent}
				onkeydown={handleKeydown}
			></textarea>
			<div class="form-actions">
				<button class="lv-btn lvui-btn lv-btn--outline" type="button" onclick={cancelAdding} disabled={saving}>
					Cancelar
				</button>
				<button class="lv-btn lvui-btn" type="button" onclick={saveNote} disabled={saving || !noteContent.trim()}>
					{saving ? 'Salvando…' : 'Salvar anotação'}
				</button>
			</div>
		</div>
	{/if}

	{#if loading}
		<p class="study-notes-empty" role="status">Carregando anotações…</p>
	{:else if notes.length === 0 && !isAdding}
		<div class="study-notes-empty">
			<p>Nenhuma anotação nesta aula.</p>
			<small>Salve anotações vinculadas aos minutos da aula para revisar conceitos com facilidade.</small>
		</div>
	{:else if notes.length > 0}
		<ul class="study-notes-list">
			{#each notes as note (note.noteId)}
				<li class="study-note-item">
					<button
						class="timestamp-pill"
						type="button"
						onclick={() => onSeek(note.positionSeconds)}
						aria-label={`Pular vídeo para ${formatDuration(note.positionSeconds)}`}
					>
						<svg class="pill-play-icon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
							<polygon points="5 3 19 12 5 21 5 3" />
						</svg>
						{formatDuration(note.positionSeconds)}
					</button>

					<div class="note-content-body">
						<p class="note-text">{note.content}</p>
					</div>

					<button
						class="delete-note-btn"
						type="button"
						onclick={() => deleteNote(note.noteId)}
						aria-label="Excluir anotação"
						title="Excluir anotação"
					>
						<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
							<polyline points="3 6 5 6 21 6" />
							<path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
						</svg>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.study-notes-section {
		margin-top: 1.5rem;
		padding: 1.25rem;
		background: var(--lv-surface-1, #1e1e24);
		border: 1px solid var(--lv-border-color, #2a2a35);
		border-radius: 0.75rem;
	}

	.study-notes-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 1rem;
		flex-wrap: wrap;
		margin-bottom: 1rem;
	}

	.study-notes-title-group {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.study-notes-title {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: 1.05rem;
		font-weight: 600;
		color: var(--lv-text-primary, #ffffff);
		margin: 0;
	}

	.study-notes-icon {
		width: 1.15rem;
		height: 1.15rem;
		color: var(--lv-accent, #6366f1);
	}

	.study-notes-count {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		padding: 0.1rem 0.5rem;
		font-size: 0.75rem;
		font-weight: 600;
		background: var(--lv-surface-2, #2d2d38);
		color: var(--lv-text-secondary, #9ca3af);
		border-radius: 9999px;
	}

	.btn-icon {
		width: 1rem;
		height: 1rem;
		margin-right: 0.25rem;
	}

	.study-notes-error {
		color: #ef4444;
		font-size: 0.875rem;
		margin-bottom: 0.75rem;
	}

	.study-notes-form {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 1rem;
		background: var(--lv-surface-2, #252530);
		border: 1px solid var(--lv-border-color, #3a3a48);
		border-radius: 0.5rem;
		margin-bottom: 1rem;
	}

	.form-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		font-size: 0.8rem;
	}

	.timestamp-badge {
		font-weight: 600;
		color: var(--lv-accent, #818cf8);
	}

	.form-hint {
		color: var(--lv-text-muted, #6b7280);
	}

	.study-notes-input {
		width: 100%;
		padding: 0.75rem;
		font-family: inherit;
		font-size: 0.9rem;
		color: var(--lv-text-primary, #ffffff);
		background: var(--lv-surface-1, #18181f);
		border: 1px solid var(--lv-border-color, #333342);
		border-radius: 0.375rem;
		resize: vertical;
		outline: none;
		box-sizing: border-box;
	}

	.study-notes-input:focus {
		border-color: var(--lv-accent, #6366f1);
		box-shadow: 0 0 0 2px rgba(99, 102, 241, 0.2);
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 0.5rem;
	}

	.study-notes-empty {
		text-align: center;
		padding: 1.5rem 1rem;
		color: var(--lv-text-secondary, #9ca3af);
		font-size: 0.9rem;
	}

	.study-notes-empty small {
		display: block;
		color: var(--lv-text-muted, #6b7280);
		margin-top: 0.25rem;
		font-size: 0.8rem;
	}

	.study-notes-list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.study-note-item {
		display: flex;
		align-items: flex-start;
		gap: 0.75rem;
		padding: 0.75rem;
		background: var(--lv-surface-2, #242430);
		border: 1px solid var(--lv-border-color, #2f2f3d);
		border-radius: 0.5rem;
		transition: border-color 0.15s ease;
	}

	.study-note-item:hover {
		border-color: var(--lv-accent, #4f46e5);
	}

	.timestamp-pill {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.25rem 0.6rem;
		font-family: inherit;
		font-size: 0.8rem;
		font-weight: 600;
		color: #ffffff;
		background: var(--lv-accent, #6366f1);
		border: none;
		border-radius: 0.375rem;
		cursor: pointer;
		flex-shrink: 0;
		transition: opacity 0.15s ease, transform 0.1s ease;
	}

	.timestamp-pill:hover {
		opacity: 0.9;
		transform: scale(1.02);
	}

	.pill-play-icon {
		width: 0.65rem;
		height: 0.65rem;
	}

	.note-content-body {
		flex: 1;
		min-width: 0;
	}

	.note-text {
		margin: 0;
		font-size: 0.875rem;
		line-height: 1.45;
		color: var(--lv-text-primary, #f3f4f6);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.delete-note-btn {
		background: transparent;
		border: none;
		color: var(--lv-text-muted, #6b7280);
		padding: 0.25rem;
		border-radius: 0.25rem;
		cursor: pointer;
		flex-shrink: 0;
		transition: color 0.15s ease;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.delete-note-btn:hover {
		color: #ef4444;
	}

	.delete-note-btn svg {
		width: 1rem;
		height: 1rem;
	}
</style>
