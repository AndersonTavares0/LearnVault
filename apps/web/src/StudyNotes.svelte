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
	.notes { margin: 28px 0; padding: 20px; border: 1px solid color-mix(in srgb, currentColor 18%, transparent); border-radius: 12px; color: inherit; }
	h2 { margin: 0 0 14px; font: 400 22px Georgia, serif; }
	form, li { display: grid; gap: 10px; }
	textarea { box-sizing: border-box; width: 100%; resize: vertical; padding: 10px; border: 1px solid color-mix(in srgb, currentColor 28%, transparent); border-radius: 7px; background: transparent; color: inherit; font: inherit; }
	.check { display: flex; align-items: center; gap: 8px; font-size: 14px; }
	button { width: fit-content; padding: 8px 12px; border: 1px solid currentColor; border-radius: 7px; background: transparent; color: inherit; font: inherit; cursor: pointer; }
	button:disabled { opacity: .6; cursor: wait; }
	button:focus-visible, textarea:focus-visible { outline: 2px solid currentColor; outline-offset: 2px; }
	ul { list-style: none; display: grid; gap: 14px; margin: 18px 0 0; padding: 0; }
	li { padding: 14px 0; border-top: 1px solid color-mix(in srgb, currentColor 15%, transparent); }
	.note-text { white-space: pre-wrap; overflow-wrap: anywhere; margin: 0; }
	.timestamp { font-weight: 700; }
	.actions { display: flex; gap: 8px; }
	.secondary { opacity: .8; }
	.state { color: inherit; opacity: .75; }
	.hint { font-size: 13px; opacity: .7; }
	.error { color: #a23f32; }
</style>
