<script lang="ts">
	import { formatDuration } from './player';
	import { MAX_NOTE_TEXT_CHARS, noteTextChars, noteTextTooLarge } from './studyNotes';
	import { transport, type StudyNote } from './transport';

	let { mediaId, currentTime, onSeek }: { mediaId: string; currentTime: number; onSeek: (seconds: number) => void } = $props();
	let notes: StudyNote[] = $state([]);
	let loading = $state(true);
	let saving = $state(false);
	let error = $state('');
	let text = $state('');
	let capturePosition = $state(false);
	let editingId = $state<number | null>(null);
	let editText = $state('');
	let editPosition = $state(false);
	let editRemovePosition = $state(false);
	// O servidor valida o texto em caracteres; a interface mede o mesmo limite.
	let overLimit = $derived(noteTextTooLarge(text));
	let editOverLimit = $derived(noteTextTooLarge(editText));

	$effect(() => {
		let active = true;
		loading = true;
		error = '';
		transport.listNotes(mediaId).then((result) => { if (active) notes = result.items; })
			.catch((cause) => { if (active) error = cause instanceof Error ? cause.message : 'Não foi possível carregar as notas.'; })
			.finally(() => { if (active) loading = false; });
		return () => { active = false; };
	});

	async function createNote(event: SubmitEvent) {
		event.preventDefault();
		if (saving || !text.trim() || overLimit) return;
		saving = true; error = '';
		try {
			const position = capturePosition ? Math.floor(currentTime) : null;
			const created = await transport.createNote(mediaId, text, position);
			notes = [...notes, created]; text = ''; capturePosition = false;
		} catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível salvar a nota.'; }
		finally { saving = false; }
	}

	function startEdit(note: StudyNote) {
		editingId = note.noteId; editText = note.text; editPosition = false; editRemovePosition = false;
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
		if (saving || !window.confirm('Excluir esta nota? Esta ação não pode ser desfeita.')) return;
		saving = true; error = '';
		try { await transport.deleteNote(note.noteId); notes = notes.filter((item) => item.noteId !== note.noteId); }
		catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível excluir a nota.'; }
		finally { saving = false; }
	}
</script>

<section class="notes" aria-labelledby="notes-heading">
	<h2 id="notes-heading">Minhas notas</h2>
	<form onsubmit={createNote}>
		<label for="note-text">Nova anotação</label>
		<!-- O `maxlength` do navegador conta unidades UTF-16 e um caractere fora do plano
		     básico ocupa duas: o teto do atributo é o dobro para nunca barrar texto que o
		     limite em caracteres aceita. A validação fina é a da interface e a do servidor. -->
		<textarea id="note-text" bind:value={text} maxlength={MAX_NOTE_TEXT_CHARS * 2} required rows="3" placeholder="Escreva uma observação sobre esta aula…"></textarea>
		<p class="hint">Texto simples, até {MAX_NOTE_TEXT_CHARS.toLocaleString('pt-BR')} caracteres ({noteTextChars(text)} agora).</p>
		{#if overLimit}<p class="error" role="alert">O texto passou de {MAX_NOTE_TEXT_CHARS.toLocaleString('pt-BR')} caracteres. Encurte a anotação antes de salvar.</p>{/if}
		<label class="check"><input type="checkbox" bind:checked={capturePosition} /> Anexar posição atual ({formatDuration(Math.floor(currentTime))})</label>
		<button type="submit" disabled={saving || !text.trim() || overLimit}>{saving ? 'Salvando…' : 'Adicionar nota'}</button>
	</form>
	{#if error}<p class="error" role="alert">{error}</p>{/if}
	{#if saving}<p class="state" role="status">Salvando alterações…</p>{/if}
	{#if loading}<p class="state" role="status">Carregando notas…</p>
	{:else if !error && notes.length === 0}<p class="state" role="status">Ainda não há notas para esta mídia.</p>
	{:else if notes.length > 0}
		<ul>
			{#each notes as note (note.noteId)}
				<li>
					{#if editingId === note.noteId}
						<label for="edit-note-{note.noteId}">Editar anotação</label>
						<textarea id="edit-note-{note.noteId}" bind:value={editText} maxlength={MAX_NOTE_TEXT_CHARS * 2} required rows="3"></textarea>
						<p class="hint">Até {MAX_NOTE_TEXT_CHARS.toLocaleString('pt-BR')} caracteres ({noteTextChars(editText)} agora).</p>
						{#if editOverLimit}<p class="error" role="alert">O texto passou de {MAX_NOTE_TEXT_CHARS.toLocaleString('pt-BR')} caracteres. Encurte a anotação antes de salvar.</p>{/if}
						<label class="check"><input type="checkbox" bind:checked={editPosition} /> Atualizar timestamp para posição atual ({formatDuration(Math.floor(currentTime))})</label>
						{#if note.positionSeconds !== null}<label class="check"><input type="checkbox" bind:checked={editRemovePosition} /> Remover timestamp existente</label>{/if}
						<button type="button" onclick={() => saveEdit(note)} disabled={saving || !editText.trim() || editOverLimit}>Salvar edição</button>
						<button class="secondary" type="button" onclick={() => editingId = null} disabled={saving}>Cancelar</button>
					{:else}
						{#if note.positionSeconds !== null}<button class="timestamp" type="button" onclick={() => onSeek(note.positionSeconds ?? 0)} aria-label="Ir para {formatDuration(note.positionSeconds)}">{formatDuration(note.positionSeconds)}</button>{/if}
						<p class="note-text">{note.text}</p>
						<div class="actions"><button type="button" onclick={() => startEdit(note)}>Editar</button><button class="secondary" type="button" onclick={() => remove(note)}>Excluir</button></div>
					{/if}
				</li>
			{/each}
		</ul>
	{/if}
</section>

<style>
	.notes {
		margin: 24px 0;
		padding: 20px;
		border: 1px solid var(--line);
		border-radius: var(--radius-md, 12px);
		background: var(--surface);
		color: var(--ink);
	}

	h2 { margin: 0 0 12px; font-size: 16px; line-height: 24px; font-weight: 650; }

	form, li { display: grid; gap: 10px; }

	label { color: var(--ink); font-size: 14px; line-height: 20px; font-weight: 600; }

	textarea {
		box-sizing: border-box;
		width: 100%;
		min-height: 88px;
		padding: 10px 12px;
		resize: vertical;
		border: 1px solid var(--line-strong);
		border-radius: var(--radius-sm, 8px);
		background: var(--input);
		color: var(--ink);
		font: inherit;
		font-size: 16px;
		line-height: 24px;
	}

	.check { display: flex; align-items: center; gap: 8px; font-size: 14px; line-height: 20px; }

	button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: fit-content;
		min-height: 44px;
		padding: 10px 16px;
		border: 1px solid var(--line-strong);
		border-radius: var(--radius-sm, 8px);
		background: var(--surface);
		color: var(--ink);
		font: inherit;
		font-size: 14px;
		font-weight: 600;
		cursor: pointer;
	}

	button[type='submit'] { border-color: var(--accent); background: var(--accent); color: var(--on-accent); }

	button:disabled { opacity: .55; cursor: wait; }

	button:focus-visible, textarea:focus-visible { outline: 3px solid var(--accent); outline-offset: 2px; }

	ul { list-style: none; display: grid; gap: 0; margin: 16px 0 0; padding: 0; }

	li { padding: 14px 0; border-top: 1px solid var(--line); }

	.note-text { white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; font-size: 16px; line-height: 24px; }

	.timestamp { min-height: 32px; padding: 4px 10px; border-radius: 999px; background: var(--accent-soft); color: var(--accent-strong); font-weight: 650; font-variant-numeric: tabular-nums; }

	.actions { display: flex; gap: 8px; }

	.secondary { background: transparent; }

	.state { margin: 0; color: var(--muted); font-size: 14px; line-height: 20px; }

	.hint { margin: 0; color: var(--muted); font-size: 13px; line-height: 20px; }

	.error { margin: 0; color: var(--danger); font-size: 14px; line-height: 20px; }
</style>
