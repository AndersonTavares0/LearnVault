<script lang="ts">
	import { formatDuration } from './player';
	import { MAX_NOTE_TEXT_CHARS, noteTextChars, noteTextTooLarge } from './studyNotes';
	import { transport, type StudyNote } from './transport';

	let { mediaId, currentTime, onSeek, onViewAll = undefined }: { mediaId: string; currentTime: number; onSeek: (seconds: number) => void; onViewAll?: () => void } = $props();
	let notes: StudyNote[] = $state([]);
	let loading = $state(true);
	let saving = $state(false);
	let error = $state('');
	let text = $state('');
	let composerOpen = $state(false);
	let capturePosition = $state(false);
	let editingId = $state<number | null>(null);
	let editText = $state('');
	let editPosition = $state(false);
	let editRemovePosition = $state(false);
	let confirmingId = $state<number | null>(null);
	let expandedId = $state<number | null>(null);
	// O servidor valida o texto em caracteres; a interface mede o mesmo limite.
	let overLimit = $derived(noteTextTooLarge(text));
	let editOverLimit = $derived(noteTextTooLarge(editText));

	$effect(() => {
		let active = true;
		loading = true;
		error = '';
		composerOpen = false;
		confirmingId = null;
		expandedId = null;
		transport.listNotes(mediaId).then((result) => { if (active) notes = result.items; })
			.catch((cause) => { if (active) error = cause instanceof Error ? cause.message : 'Não foi possível carregar as notas.'; })
			.finally(() => { if (active) loading = false; });
		return () => { active = false; };
	});

	function openComposer() {
		composerOpen = true;
		error = '';
	}

	function cancelComposer() {
		composerOpen = false;
		text = '';
		capturePosition = false;
	}

	async function createNote(event: SubmitEvent) {
		event.preventDefault();
		if (saving || !text.trim() || overLimit) return;
		saving = true; error = '';
		try {
			const position = capturePosition ? Math.floor(currentTime) : null;
			const created = await transport.createNote(mediaId, text, position);
			notes = [created, ...notes];
			text = '';
			capturePosition = false;
			composerOpen = false;
		} catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível salvar a nota.'; }
		finally { saving = false; }
	}

	function startEdit(note: StudyNote) {
		editingId = note.noteId; editText = note.text; editPosition = false; editRemovePosition = false;
		confirmingId = null;
	}

	async function saveEdit(note: StudyNote) {
		if (saving || !editText.trim() || editOverLimit) return;
		saving = true; error = '';
		try {
			const position = editRemovePosition ? null : editPosition ? Math.floor(currentTime) : note.positionSeconds;
			const updated = await transport.updateNote(note.noteId, editText, position);
			notes = notes.map((item) => item.noteId === updated.noteId ? updated : item); editingId = null;
		} catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível atualizar a nota.'; }
		finally { saving = false; }
	}

	async function remove(note: StudyNote) {
		if (saving) return;
		saving = true; error = '';
		try {
			await transport.deleteNote(note.noteId);
			notes = notes.filter((item) => item.noteId !== note.noteId);
			confirmingId = null;
		}
		catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível excluir a nota.'; }
		finally { saving = false; }
	}

	function formatNoteDate(seconds: number): string {
		return new Date(seconds * 1000).toLocaleString('pt-BR', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });
	}
</script>

<section class="notes" aria-labelledby="notes-heading">
	<div class="notes-head">
		<div>
			<h2 id="notes-heading">Anotações</h2>
			{#if !loading}<span class="count">{notes.length === 1 ? '1 anotação nesta aula' : `${notes.length} anotações nesta aula`}</span>{/if}
		</div>
		{#if !composerOpen}<button class="new-note" type="button" onclick={openComposer}>Nova anotação</button>{/if}
	</div>

	{#if composerOpen}
		<form class="composer" onsubmit={createNote}>
			<label class="visually-hidden" for="note-text">Nova anotação</label>
			<!-- O `maxlength` do navegador conta unidades UTF-16 e um caractere fora do plano
			     básico ocupa duas: o teto do atributo é o dobro para nunca barrar texto que o
			     limite em caracteres aceita. A validação fina é a da interface e a do servidor. -->
			<textarea id="note-text" bind:value={text} maxlength={MAX_NOTE_TEXT_CHARS * 2} required rows="4" placeholder="Escreva uma observação sobre esta aula…"></textarea>
			<div class="composer-meta">
				<label class="check"><input type="checkbox" bind:checked={capturePosition} /> Salvar posição atual ({formatDuration(Math.floor(currentTime))})</label>
				<span class="hint" class:over={overLimit}>{noteTextChars(text).toLocaleString('pt-BR')} / {MAX_NOTE_TEXT_CHARS.toLocaleString('pt-BR')}</span>
			</div>
			{#if overLimit}<p class="error" role="alert">O texto passou de {MAX_NOTE_TEXT_CHARS.toLocaleString('pt-BR')} caracteres. Encurte a anotação antes de salvar.</p>{/if}
			<div class="composer-actions">
				<button type="submit" disabled={saving || !text.trim() || overLimit}>{saving ? 'Salvando…' : 'Salvar anotação'}</button>
				<button class="text-button" type="button" onclick={cancelComposer} disabled={saving}>Cancelar</button>
			</div>
		</form>
	{/if}

	{#if error}<p class="error" role="alert">{error}</p>{/if}
	<p class="visually-hidden" aria-live="polite">{saving ? 'Salvando alterações…' : ''}</p>

	{#if loading}
		<p class="state" role="status">Carregando anotações…</p>
	{:else if notes.length === 0}
		<div class="empty" role="status">
			<p>Ainda não há anotações nesta aula.</p>
			{#if !composerOpen}<button class="text-button" type="button" onclick={openComposer}>Escrever a primeira</button>{/if}
		</div>
	{:else}
		<ul class="note-list">
			{#each notes as note (note.noteId)}
				<li class="note">
					{#if editingId === note.noteId}
						<label class="visually-hidden" for="edit-note-{note.noteId}">Editar anotação</label>
						<textarea id="edit-note-{note.noteId}" bind:value={editText} maxlength={MAX_NOTE_TEXT_CHARS * 2} required rows="4"></textarea>
						<div class="composer-meta">
							<span class="hint" class:over={editOverLimit}>{noteTextChars(editText).toLocaleString('pt-BR')} / {MAX_NOTE_TEXT_CHARS.toLocaleString('pt-BR')}</span>
						</div>
						{#if editOverLimit}<p class="error" role="alert">O texto passou de {MAX_NOTE_TEXT_CHARS.toLocaleString('pt-BR')} caracteres. Encurte a anotação antes de salvar.</p>{/if}
						<label class="check"><input type="checkbox" bind:checked={editPosition} /> Atualizar timestamp para a posição atual ({formatDuration(Math.floor(currentTime))})</label>
						{#if note.positionSeconds !== null}<label class="check"><input type="checkbox" bind:checked={editRemovePosition} /> Remover timestamp existente</label>{/if}
						<div class="composer-actions">
							<button type="button" onclick={() => saveEdit(note)} disabled={saving || !editText.trim() || editOverLimit}>Salvar edição</button>
							<button class="text-button" type="button" onclick={() => editingId = null} disabled={saving}>Cancelar</button>
						</div>
					{:else}
						<div class="note-head">
							{#if note.positionSeconds !== null}<button class="timestamp" type="button" onclick={() => onSeek(note.positionSeconds ?? 0)} aria-label="Ir para {formatDuration(note.positionSeconds)}">{formatDuration(note.positionSeconds)}</button>{/if}
							<small class="note-date" title={new Date(note.updatedAt * 1000).toLocaleString('pt-BR')}>{note.updatedAt !== note.createdAt ? 'editada em' : 'criada em'} {formatNoteDate(note.updatedAt)}</small>
						</div>
						<p class="note-text" class:clamped={expandedId !== note.noteId && note.text.length > 420}>{note.text}</p>
						{#if note.text.length > 420}<button class="text-button more" type="button" onclick={() => expandedId = expandedId === note.noteId ? null : note.noteId} aria-expanded={expandedId === note.noteId}>{expandedId === note.noteId ? 'Mostrar menos' : 'Mostrar tudo'}</button>{/if}
						{#if confirmingId === note.noteId}
							<div class="confirm" role="alert">
								<span>Excluir esta anotação?</span>
								<button class="danger" type="button" onclick={() => remove(note)} disabled={saving}>Excluir</button>
								<button class="text-button" type="button" onclick={() => confirmingId = null} disabled={saving}>Cancelar</button>
							</div>
						{:else}
							<div class="note-actions">
								<button class="text-button" type="button" onclick={() => startEdit(note)}>Editar</button>
								<button class="text-button danger" type="button" onclick={() => confirmingId = note.noteId}>Excluir</button>
							</div>
						{/if}
					{/if}
				</li>
			{/each}
		</ul>
	{/if}

	{#if onViewAll}
		<button class="text-button view-all" type="button" onclick={onViewAll}>Ver todas as anotações</button>
	{/if}
</section>

<style>
	.notes { display: grid; gap: 14px; color: inherit; }
	.notes-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
	h2 { margin: 0; font-family: inherit; font-size: 15px; font-weight: 600; line-height: 1.3; letter-spacing: .01em; }
	.count { display: block; margin-top: 2px; color: var(--muted); font-size: 12px; }
	.new-note { flex: none; min-height: 34px; padding: 0 12px; border: 0; border-radius: 8px; background: var(--accent); color: var(--brand-mark-ink); font: 600 13px/1 inherit; cursor: pointer; }
	.new-note:hover { filter: brightness(1.08); }
	.composer { display: grid; gap: 8px; padding: 12px; border: 1px solid var(--line); border-radius: 10px; background: var(--page); }
	textarea { box-sizing: border-box; width: 100%; resize: vertical; padding: 10px; border: 1px solid var(--input-border); border-radius: 8px; background: var(--input); color: var(--text); font: inherit; line-height: 1.5; }
	.composer-meta { display: flex; align-items: center; justify-content: space-between; gap: 10px; flex-wrap: wrap; }
	.check { display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--muted); }
	.composer-actions { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
	button { font: inherit; cursor: pointer; }
	.composer-actions button:not(.text-button) { min-height: 34px; padding: 0 12px; border: 1px solid var(--accent); border-radius: 8px; background: var(--accent); color: var(--brand-mark-ink); font-size: 13px; font-weight: 600; }
	button:disabled { opacity: .55; cursor: wait; }
	button:focus-visible, textarea:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
	.note-list { list-style: none; display: grid; margin: 0; padding: 0; }
	.note { display: grid; gap: 6px; padding: 12px 0; border-top: 1px solid var(--line); }
	.note:first-child { border-top: 0; padding-top: 2px; }
	.note-head { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
	.timestamp { padding: 1px 7px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface); color: var(--accent); font: 600 11px/1.6 ui-monospace, SFMono-Regular, Menlo, monospace; font-variant-numeric: tabular-nums; }
	.timestamp:hover { border-color: var(--accent); }
	.note-date { color: var(--muted); font-size: 11px; }
	.note-text { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.55; font-size: 14px; }
	.note-text.clamped { display: -webkit-box; -webkit-line-clamp: 6; line-clamp: 6; -webkit-box-orient: vertical; overflow: hidden; }
	.more { justify-self: start; }
	.note-actions, .confirm { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; font-size: 12px; }
	.note-actions { opacity: 0; transition: opacity 150ms ease; }
	.note:hover .note-actions, .note:focus-within .note-actions { opacity: 1; }
	@media (hover: none) { .note-actions { opacity: 1; } }
	.danger { color: var(--error); }
	.confirm { color: var(--muted); }
	.confirm .danger { padding: 0; border: 0; background: none; font-weight: 600; text-decoration: underline; }
	.state { margin: 0; color: var(--muted); font-size: 13px; }
	.empty { display: grid; justify-items: start; gap: 4px; padding: 14px 0; color: var(--muted); font-size: 13px; }
	.empty p { margin: 0; }
	.hint { color: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }
	.hint.over { color: var(--error); }
	.error { margin: 0; color: var(--error); font-size: 12px; }
	.view-all { justify-self: start; padding-top: 2px; }
	@media (prefers-reduced-motion: reduce) { .note-actions { transition: none; } }
</style>
