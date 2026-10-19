<script lang="ts">
	import { onMount } from 'svelte';
	import { formatDuration } from './player';
	import { buildNotesEntries, courseOptions, createDefaultNotesFilters, filterNotes, groupNotes, type NotesSort, type NotesView } from './notes-filter';
	import { href } from './router';
	import { transport, type Catalog, type StudyNote } from './transport';

	let { catalog }: { catalog: Catalog | null } = $props();
	let notes = $state<StudyNote[]>([]);
	let loading = $state(true);
	let error = $state('');
	let truncated = $state(false);
	let filters = $state(createDefaultNotesFilters());
	let sort = $state<NotesSort>('recent');
	let view = $state<NotesView>('course');
	let expanded = $state<Set<number>>(new Set());
	let confirmingId = $state<number | null>(null);
	let saving = $state(false);
	let now = $state(Math.floor(Date.now() / 1000));

	let entries = $derived(buildNotesEntries(notes, catalog));
	let courses = $derived(courseOptions(entries));
	let filtered = $derived(filterNotes(entries, filters, sort, now));
	let groups = $derived(groupNotes(filtered, view, sort, now));
	let activeFilters = $derived(filters.query.trim() !== '' || filters.course !== 'all' || filters.period !== 'all');

	onMount(load);

	async function load() {
		loading = true;
		error = '';
		now = Math.floor(Date.now() / 1000);
		try {
			const result = await transport.listAllNotes();
			notes = result.items;
			truncated = result.truncated;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível carregar as anotações.';
		} finally {
			loading = false;
		}
	}

	function clearFilters() {
		filters = createDefaultNotesFilters();
	}

	function toggleExpanded(noteId: number) {
		const next = new Set(expanded);
		if (next.has(noteId)) next.delete(noteId);
		else next.add(noteId);
		expanded = next;
	}

	async function remove(note: StudyNote) {
		if (saving) return;
		saving = true;
		error = '';
		try {
			await transport.deleteNote(note.noteId);
			notes = notes.filter((item) => item.noteId !== note.noteId);
			confirmingId = null;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível excluir a nota.';
		} finally {
			saving = false;
		}
	}

	function formatTime(seconds: number): string {
		return new Date(seconds * 1000).toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' });
	}

	function formatFullDate(seconds: number): string {
		return new Date(seconds * 1000).toLocaleString('pt-BR');
	}
</script>

<header class="notes-header">
	<h1>Anotações</h1>
	<p class="intro">Tudo o que você escreveu enquanto estudava, reunido por aula. Filtre por curso, período ou pelo texto.</p>
</header>

<div class="notes-toolbar">
	<div class="search-field">
		<svg class="search-glyph" viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="7" fill="none" stroke="currentColor" stroke-width="2"></circle><path d="m16.6 16.6 4.2 4.2" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path></svg>
		<label class="visually-hidden" for="notes-search">Buscar nas anotações</label>
		<input id="notes-search" type="search" placeholder="Buscar no texto, na aula ou no curso" bind:value={filters.query} />
	</div>
	<div class="filter-row">
		<label>Curso
			<select bind:value={filters.course}>
				<option value="all">Todos os cursos</option>
				{#each courses as course (course.id)}<option value={course.id}>{course.title} ({course.count})</option>{/each}
			</select>
		</label>
		<label>Período
			<select bind:value={filters.period}>
				<option value="all">Todo o período</option>
				<option value="today">Hoje</option>
				<option value="7">Últimos 7 dias</option>
				<option value="30">Últimos 30 dias</option>
				<option value="custom">Escolher datas</option>
			</select>
		</label>
		{#if filters.period === 'custom'}
			<label>De<input type="date" bind:value={filters.from} max={filters.to || undefined} /></label>
			<label>Até<input type="date" bind:value={filters.to} min={filters.from || undefined} /></label>
		{/if}
		<label>Ordenar
			<select bind:value={sort}>
				<option value="recent">Mais recentes</option>
				<option value="oldest">Mais antigas</option>
			</select>
		</label>
		<div class="view-switch" role="group" aria-label="Agrupar anotações">
			<button type="button" class:active={view === 'course'} aria-pressed={view === 'course'} onclick={() => (view = 'course')}>Por curso</button>
			<button type="button" class:active={view === 'date'} aria-pressed={view === 'date'} onclick={() => (view = 'date')}>Por data</button>
		</div>
		{#if activeFilters}<button class="text-button clear" type="button" onclick={clearFilters}>Limpar filtros</button>{/if}
	</div>
</div>

<div class="notes-status" aria-live="polite">
	<span>{loading ? 'Carregando anotações…' : `${filtered.length} de ${entries.length} ${entries.length === 1 ? 'anotação' : 'anotações'}`}</span>
	{#if truncated}<span class="truncated">Mostrando as 2.000 mais recentes; refine os filtros ou exclua notas antigas para ver as demais.</span>{/if}
</div>

{#if error}<p class="error" role="alert">{error} <button class="text-button" type="button" onclick={load}>Tentar de novo</button></p>{/if}

{#if loading}
	<p class="state" role="status">Carregando anotações…</p>
{:else if entries.length === 0}
	<div class="empty-panel" role="status">
		<h2>Nenhuma anotação ainda</h2>
		<p>Abra uma aula e escreva a primeira anotação. Ela aparece aqui, ligada ao curso e ao minuto do vídeo.</p>
		<a class="primary-link" href={href('home')}>Ir para os cursos</a>
	</div>
{:else if filtered.length === 0}
	<div class="empty-panel" role="status">
		<h2>Nenhuma anotação corresponde aos filtros</h2>
		<p>Tente outro curso, amplie o período ou apague a busca.</p>
		<button class="text-button" type="button" onclick={clearFilters}>Limpar filtros</button>
	</div>
{:else}
	{#each groups as group (group.key)}
		<section class="note-group" aria-labelledby={`note-group-${group.key}`}>
			<h2 id={`note-group-${group.key}`}>{group.title} <span>{group.entries.length}</span></h2>
			<ul>
				{#each group.entries as entry (entry.note.noteId)}
					{@const note = entry.note}
					<li class="note-row">
						<div class="note-when">
							{#if note.positionSeconds !== null}
								{#if entry.lessonPath}<a class="timestamp" href={href('course', entry.courseId, entry.lessonPath)} aria-label="Abrir a aula no minuto {formatDuration(note.positionSeconds)}">{formatDuration(note.positionSeconds)}</a>{:else}<span class="timestamp">{formatDuration(note.positionSeconds)}</span>{/if}
							{/if}
							<time datetime={new Date(note.updatedAt * 1000).toISOString()} title={formatFullDate(note.updatedAt)}>{formatTime(note.updatedAt)}</time>
						</div>
						<div class="note-content">
							<div class="note-context">
								{#if entry.lessonPath}<a href={href('course', entry.courseId, entry.lessonPath)}>{entry.lessonTitle}</a>{:else}<span>{entry.lessonTitle}</span>{/if}
								{#if view === 'date'}<span class="sep" aria-hidden="true">·</span><span>{entry.courseTitle}</span>{/if}
								{#if entry.moduleTitle}<span class="sep" aria-hidden="true">·</span><span>{entry.moduleTitle}</span>{/if}
								{#if note.updatedAt !== note.createdAt}<span class="edited">editada</span>{/if}
							</div>
							<p class="note-text" class:clamped={!expanded.has(note.noteId) && note.text.length > 420}>{note.text}</p>
							{#if note.text.length > 420}<button class="text-button more" type="button" onclick={() => toggleExpanded(note.noteId)} aria-expanded={expanded.has(note.noteId)}>{expanded.has(note.noteId) ? 'Mostrar menos' : 'Mostrar tudo'}</button>{/if}
							{#if confirmingId === note.noteId}
								<div class="confirm" role="alert">
									<span>Excluir esta anotação?</span>
									<button class="danger" type="button" onclick={() => remove(note)} disabled={saving}>Excluir</button>
									<button class="text-button" type="button" onclick={() => (confirmingId = null)} disabled={saving}>Cancelar</button>
								</div>
							{:else}
								<div class="note-actions">
									<button class="text-button danger" type="button" onclick={() => (confirmingId = note.noteId)}>Excluir</button>
								</div>
							{/if}
						</div>
					</li>
				{/each}
			</ul>
		</section>
	{/each}
{/if}

<style>
	.notes-header { max-width: 640px; margin-bottom: 22px; }
	h1 { margin: 0 0 6px; }
	.intro { margin: 0; color: var(--muted); }
	.notes-toolbar { display: grid; gap: 10px; padding: 14px 0 16px; border-top: 1px solid var(--line); border-bottom: 1px solid var(--line); }
	.search-field { position: relative; display: flex; align-items: center; max-width: 560px; }
	.search-glyph { position: absolute; left: 12px; width: 17px; height: 17px; color: var(--muted); pointer-events: none; }
	.search-field input { width: 100%; min-height: 40px; padding: 0 12px 0 38px; border: 1px solid var(--input-border); border-radius: 9px; background: var(--input); color: var(--text); font: inherit; }
	.filter-row { display: flex; align-items: end; gap: 10px 14px; flex-wrap: wrap; }
	.filter-row label { display: grid; gap: 4px; color: var(--muted); font-size: 11px; font-weight: 600; letter-spacing: .02em; }
	select, input[type='date'] { min-height: 34px; padding: 0 8px; border: 1px solid var(--input-border); border-radius: 8px; background: var(--input); color: var(--text); font: inherit; font-size: 13px; }
	.search-field input:focus-visible, select:focus-visible, input[type='date']:focus-visible, button:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }
	.view-switch { display: inline-flex; padding: 2px; border: 1px solid var(--line); border-radius: 9px; }
	.view-switch button { min-height: 30px; padding: 0 10px; border: 0; border-radius: 7px; background: transparent; color: var(--muted); font: inherit; font-size: 12px; cursor: pointer; }
	.view-switch button.active { background: var(--accent); color: var(--brand-mark-ink); }
	.clear { align-self: center; }
	.notes-status { display: flex; align-items: baseline; justify-content: space-between; gap: 16px; flex-wrap: wrap; padding: 10px 0; color: var(--muted); font-size: 12px; }
	.truncated { color: var(--error); }
	.note-group { margin-top: 12px; }
	.note-group h2 { display: flex; align-items: baseline; gap: 8px; margin: 0; padding: 12px 0 8px; font-family: inherit; font-size: 13px; font-weight: 600; letter-spacing: .02em; color: var(--heading); position: sticky; top: 0; background: var(--page); z-index: 1; }
	.note-group h2 span { color: var(--muted); font-weight: 400; }
	.note-group ul { list-style: none; margin: 0; padding: 0; }
	.note-row { display: grid; grid-template-columns: 84px minmax(0, 1fr); gap: 12px; padding: 12px 0 14px; border-top: 1px solid var(--line); }
	.note-when { display: grid; justify-items: start; align-content: start; gap: 4px; color: var(--muted); font-size: 11px; font-variant-numeric: tabular-nums; }
	.timestamp { padding: 1px 7px; border: 1px solid var(--line); border-radius: 999px; background: var(--surface); color: var(--accent); font: 600 11px/1.6 ui-monospace, SFMono-Regular, Menlo, monospace; font-variant-numeric: tabular-nums; cursor: pointer; }
	.timestamp:hover { border-color: var(--accent); }
	.note-content { display: grid; gap: 5px; min-width: 0; }
	.note-context { display: flex; align-items: baseline; gap: 6px; flex-wrap: wrap; font-size: 12px; color: var(--muted); }
	.note-context a { color: var(--accent); font-weight: 600; text-decoration: none; }
	.note-context a:hover { text-decoration: underline; }
	.sep { opacity: .5; }
	.edited { padding: 0 6px; border: 1px solid var(--line); border-radius: 999px; font-size: 10px; }
	.note-text { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; line-height: 1.6; font-size: 14px; }
	.note-text.clamped { display: -webkit-box; -webkit-line-clamp: 6; line-clamp: 6; -webkit-box-orient: vertical; overflow: hidden; }
	.more, .note-actions, .confirm { justify-self: start; }
	.note-actions { opacity: 0; transition: opacity 150ms ease; }
	.note-row:hover .note-actions, .note-row:focus-within .note-actions { opacity: 1; }
	@media (hover: none) { .note-actions { opacity: 1; } }
	.note-actions button, .confirm button { padding: 0; border: 0; background: none; font: inherit; font-size: 12px; }
	.danger { color: var(--error); }
	.confirm { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; font-size: 12px; color: var(--muted); }
	.confirm .danger { font-weight: 600; text-decoration: underline; }
	.state, .empty-panel { color: var(--muted); }
	.empty-panel { display: grid; justify-items: start; gap: 6px; padding: 32px 0; }
	.empty-panel h2 { margin: 0; color: var(--heading); font-family: inherit; font-size: 17px; }
	.empty-panel p { margin: 0; max-width: 520px; }
	.primary-link { display: inline-flex; align-items: center; min-height: 36px; margin-top: 6px; padding: 0 14px; border-radius: 8px; background: var(--accent); color: var(--brand-mark-ink); text-decoration: none; font-size: 13px; font-weight: 600; }
	.error { color: var(--error); }
	.error .text-button { color: inherit; text-decoration: underline; }
	@media (max-width: 700px) {
		.note-row { grid-template-columns: 1fr; gap: 4px; }
		.note-when { display: flex; align-items: center; gap: 8px; }
	}
	@media (prefers-reduced-motion: reduce) { .note-actions { transition: none; } }
</style>
