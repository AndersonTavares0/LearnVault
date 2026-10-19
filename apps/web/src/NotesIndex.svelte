<script lang="ts">
	import { formatDuration } from './player';
	import { findLessonLocation, href, type LessonLocation } from './router';
	import { transport, type Catalog, type StudyNote } from './transport';

	let { catalog }: { catalog: Catalog | null } = $props();

	let notes = $state<StudyNote[]>([]);
	let loading = $state(true);
	let error = $state('');
	let truncated = $state(false);

	type Entry = { note: StudyNote; location: LessonLocation | null };
	type Group = { key: string; title: string; courseId: string | null; entries: Entry[] };

	let groups = $derived(groupByCourse(notes, catalog));

	// As anotações são separadas por curso: o catálogo diz de qual curso cada mídia
	// faz parte. A nota de uma mídia que saiu do catálogo não é escondida: ela fica
	// em um grupo próprio, com o nome da mídia.
	function groupByCourse(items: StudyNote[], current: Catalog | null): Group[] {
		const groups: Group[] = [];
		const index = new Map<string, Group>();
		for (const note of items) {
			const location = findLessonLocation(current, note.mediaId);
			const key = location?.courseId ?? `mídia:${note.mediaId}`;
			let group = index.get(key);
			if (!group) {
				group = { key, title: location?.courseTitle ?? 'Mídia fora do catálogo', courseId: location?.courseId ?? null, entries: [] };
				index.set(key, group);
				groups.push(group);
			}
			group.entries.push({ note, location });
		}
		return groups;
	}

	function formatDate(seconds: number): string {
		return new Date(seconds * 1000).toLocaleDateString('pt-BR');
	}

	$effect(() => {
		let active = true;
		loading = true;
		error = '';
		transport.listAllNotes()
			.then((result) => { if (active) { notes = result.items; truncated = result.truncated; } })
			.catch((cause) => { if (active) error = cause instanceof Error ? cause.message : 'Não foi possível carregar as anotações.'; })
			.finally(() => { if (active) loading = false; });
		return () => { active = false; };
	});

	async function remove(note: StudyNote) {
		if (!window.confirm('Excluir esta anotação? Esta ação não pode ser desfeita.')) return;
		error = '';
		try {
			await transport.deleteNote(note.noteId);
			notes = notes.filter((item) => item.noteId !== note.noteId);
		} catch (cause) { error = cause instanceof Error ? cause.message : 'Não foi possível excluir a nota.'; }
	}
</script>

<section class="notes-index" aria-labelledby="notes-index-heading">
	<h1 id="notes-index-heading">Anotações</h1>
	<p class="intro">Suas anotações, separadas por curso. Cada nota fica na mídia em que você a escreveu.</p>
	{#if loading}<p class="state" role="status">Carregando anotações…</p>
	{:else if error}<p class="error" role="alert">{error}</p>
	{:else if notes.length === 0}<p class="state" role="status">Você ainda não escreveu anotações.</p>
	{:else}
		{#if truncated}<p class="state" role="status">Esta é a lista das anotações mais recentes; as mais antigas não cabem na tela.</p>{/if}
		{#each groups as group (group.key)}
			<section class="course-notes" aria-labelledby="notes-group-{group.key}">
				<h2 id="notes-group-{group.key}">{group.title}</h2>
				<ul>
					{#each group.entries as entry (entry.note.noteId)}
						<li>
							<div class="note-head">
								{#if entry.location}
									<a href={href('course', entry.location.courseId, entry.location.lesson.path)}>{entry.location.lesson.title}</a>
								{:else}
									<span>Mídia fora do catálogo</span>
								{/if}
								{#if entry.note.positionSeconds !== null}<span class="timestamp">{formatDuration(entry.note.positionSeconds)}</span>{/if}
								<small>{formatDate(entry.note.createdAt)}</small>
							</div>
							<p class="note-text">{entry.note.text}</p>
							<div class="actions"><button type="button" onclick={() => remove(entry.note)}>Excluir</button></div>
						</li>
					{/each}
				</ul>
			</section>
		{/each}
	{/if}
</section>

<style>
	.notes-index { margin: 24px 0 0; }
	h1 { margin: 0 0 6px; }
	.intro { margin: 0 0 18px; color: #687066; }
	.course-notes { margin: 26px 0 0; }
	h2 { margin: 0 0 10px; font: 400 22px Georgia, serif; }
	ul { list-style: none; margin: 0; padding: 0; display: grid; gap: 12px; }
	li { padding: 14px; border: 1px solid color-mix(in srgb, currentColor 15%, transparent); border-radius: 10px; display: grid; gap: 8px; }
	.note-head { display: flex; flex-wrap: wrap; align-items: baseline; gap: 10px; }
	.note-head a { font-weight: 600; color: inherit; }
	.note-head small { color: #77806e; }
	.timestamp { font-weight: 700; font-size: 13px; }
	.note-text { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
	.actions { display: flex; gap: 8px; }
	button { width: fit-content; padding: 6px 10px; border: 1px solid currentColor; border-radius: 7px; background: transparent; color: inherit; font: inherit; cursor: pointer; }
	button:focus-visible { outline: 2px solid currentColor; outline-offset: 2px; }
	.state { color: inherit; opacity: .75; }
	.error { color: #a23f32; }
</style>
