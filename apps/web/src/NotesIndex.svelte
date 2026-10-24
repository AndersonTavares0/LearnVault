<script lang="ts">
	import { formatDuration } from './player';
	import { findLessonLocation, href } from './router';
	import { transport, type Catalog, type StudyNote } from './transport';
	let { catalog }: { catalog: Catalog | null } = $props();
	let notes = $state<StudyNote[]>([]);
	let loading = $state(true);
	let error = $state('');
	let truncated = $state(false);
	let query = $state('');
	let selectedNoteId = $state<number | null>(null);
	let activeTab = $state<'all' | 'course' | 'recent'>('all');
	let groups = $derived.by(() => {
		const groups = new Map<string, { title: string; courseId: string | null; entries: { note: StudyNote; location: ReturnType<typeof findLessonLocation> }[] }>();
		for (const note of notes) {
			const location = findLessonLocation(catalog, note.mediaId);
			const key = location?.courseId ?? `media:${note.mediaId}`;
			let group = groups.get(key);
			if (!group) groups.set(key, group = { title: location?.courseTitle ?? 'Mídia fora do catálogo', courseId: location?.courseId ?? null, entries: [] });
			group.entries.push({ note, location });
		}
		return [...groups.entries()].map(([key, value]) => ({ key, ...value }));
	});
	let visibleNotes = $derived.by(() => {
		const result = notes.filter((note) => note.text.toLocaleLowerCase().includes(query.toLocaleLowerCase()));
		if (activeTab === 'recent') return [...result].sort((a, b) => b.updatedAt - a.updatedAt);
		if (activeTab === 'course') return [...result].sort((a, b) => (findLessonLocation(catalog, a.mediaId)?.courseTitle ?? '').localeCompare(findLessonLocation(catalog, b.mediaId)?.courseTitle ?? '', 'pt-BR'));
		return result;
	});
	let selectedNote = $derived(visibleNotes.find((note) => note.noteId === selectedNoteId) ?? visibleNotes[0] ?? null);
	$effect(() => {
		let active = true;
		loading = true; error = '';
		transport.listAllNotes().then((result) => { if (active) { notes = result.items; truncated = result.truncated; } })
			.catch((cause) => { if (active) error = cause instanceof Error ? cause.message : 'Não foi possível carregar as anotações.'; })
		.finally(() => { if (active) loading = false; });
		return () => { active = false; };
	});
	async function remove(note: StudyNote) {
		if (!window.confirm('Excluir esta anotação? Esta ação não pode ser desfeita.')) return;
		try { await transport.deleteNote(note.noteId); notes = notes.filter((item) => item.noteId !== note.noteId); }
		catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível excluir a anotação.'; }
	}
</script>

<section class="notes-page" aria-labelledby="notes-index-heading">
	<header class="notes-heading"><div><p class="page-eyebrow">Estudo pessoal</p><h1 id="notes-index-heading">Minhas anotações</h1><p class="intro">Guarde ideias e retome cada momento importante das suas aulas.</p></div><a class="primary" href={href('home')}>Explorar meus cursos</a></header>
	{#if loading}<p role="status">Carregando anotações…</p>
	{:else if error}<p class="error" role="alert">{error}</p>
	{:else if notes.length === 0}<div class="notes-empty" role="status"><div class="notes-empty-art" aria-hidden="true">✎</div><h2>Suas ideias começam aqui</h2><p>Durante uma aula, salve uma anotação para encontrá-la aqui depois.</p><a class="primary" href={href('home')}>Explorar meus cursos</a></div>
	{:else}
		<nav class="notes-tabs" aria-label="Filtro de anotações"><button class:active={activeTab === 'all'} onclick={() => (activeTab = 'all')}>Todas as notas</button><button class:active={activeTab === 'course'} onclick={() => (activeTab = 'course')}>Por curso</button><button class:active={activeTab === 'recent'} onclick={() => (activeTab = 'recent')}>Recentes</button><label><span class="visually-hidden">Filtrar anotações</span><input type="search" placeholder="Filtrar" bind:value={query} /></label></nav>
		{#if truncated}<p role="status">Exibindo as notas mais recentes; a lista foi limitada.</p>{/if}
		<div class="notes-workspace">
			<aside class="notes-list" aria-label="Lista de anotações"><h2>Recentes</h2><ul>
				{#each visibleNotes as note (note.noteId)}{@const location = findLessonLocation(catalog, note.mediaId)}<li><button class:active={selectedNote?.noteId === note.noteId} onclick={() => (selectedNoteId = note.noteId)}><strong>{note.text.slice(0, 72) || 'Anotação sem título'}</strong><span>{location?.lesson.title ?? 'Mídia fora do catálogo'}</span><small>{new Date(note.updatedAt * 1000).toLocaleDateString('pt-BR')}</small></button></li>{/each}
			</ul></aside>
			<article class="notes-detail" aria-live="polite">
				{#if selectedNote}{@const location = findLessonLocation(catalog, selectedNote.mediaId)}<header><p class="page-eyebrow">{location?.courseTitle ?? 'Mídia fora do catálogo'}</p><h2>{location?.lesson.title ?? 'Anotação'}</h2>{#if location && selectedNote.positionSeconds !== null}<a href={href('player', location.courseId, location.lesson.path)}>Voltar para {formatDuration(selectedNote.positionSeconds)} <span aria-hidden="true">→</span></a>{/if}</header><p class="notes-detail-text">{selectedNote.text}</p><button type="button" class="text-button" onclick={() => remove(selectedNote)}>Excluir anotação</button>{:else}<p>Selecione uma anotação.</p>{/if}
			</article>
		</div>
	{/if}
</section>
<style>
	.notes-page { max-width: 1120px; margin: 0 auto; }.notes-heading { display:flex; align-items:end; justify-content:space-between; gap:20px; margin:10px 0 28px; }.notes-heading h1 { margin:0; }.notes-heading .intro { margin:8px 0 0; }.page-eyebrow { margin:0 0 5px; color:var(--library-accent); font-size:11px; font-weight:700; letter-spacing:.08em; text-transform:uppercase; }.notes-tabs { display:flex; align-items:center; gap:4px; margin-bottom:18px; border-bottom:1px solid var(--library-line); }.notes-tabs button { min-height:38px; padding:0 12px; border:0; border-bottom:2px solid transparent; background:transparent; color:var(--library-muted); cursor:pointer; }.notes-tabs button.active { border-color:var(--library-accent); color:var(--library-ink); font-weight:650; }.notes-tabs label { margin-left:auto; }.notes-tabs input { width:150px; min-height:32px; padding:0 9px; border:1px solid var(--library-line); border-radius:7px; background:var(--library-surface); color:var(--library-ink); }.notes-workspace { display:grid; grid-template-columns:minmax(220px,.8fr) minmax(0,1.5fr); min-height:320px; border:1px solid var(--library-line); border-radius:12px; overflow:hidden; background:var(--library-surface); }.notes-list { border-right:1px solid var(--library-line); }.notes-list h2 { margin:0; padding:16px; color:var(--library-ink); font-size:14px; }.notes-list ul { list-style:none; margin:0; padding:0; }.notes-list li { border-top:1px solid var(--library-line); }.notes-list button { display:grid; width:100%; gap:4px; padding:13px 16px; border:0; background:transparent; color:var(--library-ink); text-align:left; cursor:pointer; }.notes-list button.active,.notes-list button:hover { background:var(--library-accent-soft); }.notes-list strong { overflow:hidden; font-size:13px; text-overflow:ellipsis; white-space:nowrap; }.notes-list span,.notes-list small { color:var(--library-muted); font-size:11px; }.notes-detail { padding:24px; }.notes-detail h2 { margin:0; color:var(--library-ink); font-size:20px; }.notes-detail header a { display:inline-block; margin-top:10px; color:var(--library-accent); font-size:12px; }.notes-detail-text { max-width:65ch; margin:28px 0; color:var(--library-ink); line-height:1.7; white-space:pre-wrap; overflow-wrap:anywhere; }.notes-empty { display:grid; justify-items:center; max-width:500px; margin:60px auto; text-align:center; }.notes-empty-art { display:grid; place-items:center; width:68px; height:68px; border-radius:18px; background:var(--library-accent-soft); color:var(--library-accent); font-size:34px; }.notes-empty h2 { margin:20px 0 8px; color:var(--library-ink); font-size:24px; }.notes-empty p { color:var(--library-muted); }.error { color:var(--error); }
	@media(max-width:700px){.notes-heading{align-items:start; flex-direction:column;}.notes-workspace{grid-template-columns:1fr;}.notes-list{border-right:0; border-bottom:1px solid var(--library-line); max-height:260px; overflow:auto;}.notes-tabs{flex-wrap:wrap;}.notes-tabs label{width:100%; margin:6px 0 12px;}.notes-tabs input{width:100%;}.notes-detail{padding:18px;}}
</style>
