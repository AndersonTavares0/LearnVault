<script lang="ts">
	import { onMount } from 'svelte';
	import { formatDuration } from './player';
	import { buildNotesEntries, courseOptions, createDefaultNotesFilters, filterNotes, groupNotes, type NotesSort, type NotesView } from './notes-filter';
	import { href } from './router';
	import { transport, type Catalog, type StudyNote } from './transport';

	let { catalog, searchQuery = $bindable('') }: { catalog: Catalog | null; searchQuery?: string } = $props();
	let notes = $state<StudyNote[]>([]);
	let loading = $state(true);
	let error = $state('');
	let truncated = $state(false);
	let filters = $state(createDefaultNotesFilters());
	let sort = $state<NotesSort>('recent');
	let view = $state<NotesView>('course');
	let expanded = $state(new Set<number>());
	let actionMenuId = $state<number | null>(null);
	let confirmingId = $state<number | null>(null);
	let saving = $state(false);
	let now = $state(Math.floor(Date.now() / 1000));

	let entries = $derived(buildNotesEntries(notes, catalog));
	let courses = $derived(courseOptions(entries));
	let queryFilters = $derived({ ...filters, query: searchQuery });
	let filtered = $derived(filterNotes(entries, queryFilters, sort, now));
	let groups = $derived(groupNotes(filtered, view, sort, now));
	let activeFilters = $derived(searchQuery.trim() !== '' || filters.course !== 'all' || filters.period !== 'all');

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
		searchQuery = '';
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
			actionMenuId = null;
		} catch (cause) {
			error = cause instanceof Error ? cause.message : 'Não foi possível excluir a anotação.';
		} finally {
			saving = false;
		}
	}

	function formatDate(seconds: number): string {
		const date = new Date(seconds * 1000);
		const current = new Date(now * 1000);
		const time = date.toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' });
		if (date.toDateString() === current.toDateString()) return `Hoje, ${time}`;
		const yesterday = new Date(current.getFullYear(), current.getMonth(), current.getDate() - 1);
		if (date.toDateString() === yesterday.toDateString()) return `Ontem, ${time}`;
		const day = date.toLocaleDateString('pt-BR', { day: '2-digit', month: 'short', ...(date.getFullYear() === current.getFullYear() ? {} : { year: 'numeric' }) });
		return `${day}, ${time}`;
	}

	function formatFullDate(seconds: number): string {
		return new Date(seconds * 1000).toLocaleString('pt-BR');
	}
</script>

<section class="notes-index" aria-labelledby="notes-index-heading">
	<header class="notes-header">
		<div>
			<div class="eyebrow">Seu material de estudo</div>
			<h1 class="title" id="notes-index-heading">Anotações</h1>
			<p class="sub">Notas privadas, organizadas por curso e ligadas ao momento da aula.</p>
		</div>
	</header>
	<div class="notes-layout">
		<aside class="filter" aria-label="Refinar anotações">
			<h3>Refinar notas</h3>
			<label for="notes-course-filter">Curso</label>
			<select id="notes-course-filter" class="field" bind:value={filters.course}>
				<option value="all">Todos os cursos</option>
				{#each courses as course (course.id)}<option value={course.id}>{course.title} ({course.count})</option>{/each}
			</select>
			<label for="notes-period-filter">Período</label>
			<select id="notes-period-filter" class="field" bind:value={filters.period}>
				<option value="all">Qualquer data</option>
				<option value="today">Hoje</option>
				<option value="7">Últimos 7 dias</option>
				<option value="30">Últimos 30 dias</option>
				<option value="custom">Escolher datas</option>
			</select>
			{#if filters.period === 'custom'}
				<label for="notes-from">De</label>
				<input id="notes-from" class="field" type="date" bind:value={filters.from} max={filters.to || undefined} />
				<label for="notes-to">Até</label>
				<input id="notes-to" class="field" type="date" bind:value={filters.to} min={filters.from || undefined} />
			{/if}
			<label for="notes-sort">Ordenar por</label>
			<select id="notes-sort" class="field" bind:value={sort}>
				<option value="recent">Mais recentes</option>
				<option value="oldest">Mais antigas</option>
			</select>
			{#if activeFilters}<button class="text-button clear-filters" type="button" onclick={clearFilters}>Limpar filtros</button>{/if}
		</aside>
		<div class="notes-results">
			{#if error}<p class="error" role="alert">{error}</p>{/if}
			<div class="row notes-toolbar">
				<strong aria-live="polite">{loading || error ? '' : `${filtered.length}${activeFilters ? ` de ${entries.length}` : ''} ${filtered.length === 1 ? 'anotação' : 'anotações'}`}</strong>
				<button class="btn" type="button" aria-pressed={view === 'course'} onclick={() => view = view === 'course' ? 'date' : 'course'}>{view === 'course' ? 'Agrupar por curso' : 'Agrupar por data'} <span aria-hidden="true">⌄</span></button>
			</div>
			{#if truncated}<p class="truncated" role="status">Mostrando as 2.000 anotações mais recentes; refine os filtros ou exclua notas antigas para ver as demais.</p>{/if}
			{#if loading}<p class="empty" role="status">Carregando anotações…</p>
			{:else if error}<div class="empty-panel" role="status"><button class="text-button" type="button" onclick={load}>Tentar de novo</button></div>
			{:else if entries.length === 0}
				<div class="empty-panel" role="status"><h2>Nenhuma anotação ainda</h2><p>Abra uma aula e escreva a primeira anotação. Ela aparecerá aqui, ligada ao curso e ao momento do vídeo.</p><a class="btn primary" href={href('home')}>Ir para os cursos</a></div>
			{:else if filtered.length === 0}
				<div class="empty-panel" role="status"><h2>Nenhuma anotação corresponde aos filtros</h2><p>Tente outro curso, amplie o período ou apague a busca.</p><button class="text-button" type="button" onclick={clearFilters}>Limpar filtros</button></div>
			{:else}
				<div class="note-list">
					{#each groups as group (group.key)}
						{#each group.entries as entry (entry.note.noteId)}
							{@const note = entry.note}
							<article class="note">
								<div class="note-when">
									{#if note.positionSeconds !== null}
										{#if entry.lessonPath}<a class="timestamp" href={href('player', entry.courseId, entry.lessonPath)} aria-label="Abrir a aula em {formatDuration(note.positionSeconds)}">{formatDuration(note.positionSeconds)}</a>{:else}<span class="timestamp">{formatDuration(note.positionSeconds)}</span>{/if}
									{/if}
								</div>
								<div class="note-copy">
									<h3>{#if entry.lessonPath}<a href={href('player', entry.courseId, entry.lessonPath)}>{entry.courseTitle} / {entry.lessonTitle}</a>{:else}{entry.courseTitle} / {entry.lessonTitle}{/if}</h3>
									<p class="note-text" class:clamped={!expanded.has(note.noteId) && note.text.length > 420}>{note.text}</p>
									<time datetime={new Date(note.updatedAt * 1000).toISOString()} title={formatFullDate(note.updatedAt)}>{formatDate(note.updatedAt)}</time>
									{#if note.text.length > 420}<button class="text-button more" type="button" onclick={() => toggleExpanded(note.noteId)} aria-expanded={expanded.has(note.noteId)}>{expanded.has(note.noteId) ? 'Mostrar menos' : 'Mostrar tudo'}</button>{/if}
									{#if note.updatedAt !== note.createdAt}<span class="edited">editada</span>{/if}
								</div>
								<div class="note-actions">
									{#if confirmingId === note.noteId}
										<span class="confirm-label">Excluir esta anotação?</span>
										<button class="danger" type="button" onclick={() => remove(note)} disabled={saving}>{saving ? 'Excluindo…' : 'Excluir'}</button>
										<button type="button" onclick={() => confirmingId = null} disabled={saving}>Cancelar</button>
									{:else}
										<button class="note-menu-toggle" type="button" aria-label="Ações da anotação" aria-expanded={actionMenuId === note.noteId} onclick={() => actionMenuId = actionMenuId === note.noteId ? null : note.noteId}>···</button>
										{#if actionMenuId === note.noteId}<button type="button" onclick={() => { confirmingId = note.noteId; actionMenuId = null; }}>Excluir</button>{/if}
									{/if}
								</div>
							</article>
						{/each}
					{/each}
				</div>
			{/if}
		</div>
	</div>
</section>

<style>
	.notes-index { max-width: 1500px; margin: 0 auto; padding: 0 24px; }
	.notes-index .title { font-size: 30px; line-height: 1.12; margin: 4px 0 5px; letter-spacing: -0.04em; }
	.notes-header { margin: 4px 2px 0; }
	.notes-index .sub { margin: 0; color: var(--muted); }
	.notes-layout { display: grid; grid-template-columns: 250px minmax(0, 1fr); gap: 26px; margin-top: 27px; }
	.filter { height: max-content; padding: 16px; border: 1px solid var(--line); border-radius: 8px; background: var(--surface); }
	.filter h3 { margin: 0 0 12px; font-size: 14px; }
	.filter label { display: block; margin: 13px 0 5px; color: var(--muted); font-size: 12px; }
	.filter .field { width: 100%; }
	.note-list { display: grid; gap: 10px; }
	.note { display: grid; grid-template-columns: 72px minmax(0, 1fr) auto; align-items: start; gap: 14px; padding: 14px; border: 1px solid var(--line); border-radius: 7px; background: var(--surface); }
	.timestamp { color: var(--accent); font-weight: 750; font-variant-numeric: tabular-nums; text-decoration: none; }
	.timestamp:hover { text-decoration: underline; }
	.note-when { display: grid; justify-items: start; align-content: start; gap: 4px; color: var(--muted); font-size: 11px; }
	.note-copy>time { display: block; margin-top: 6px; color: var(--muted); font-size: 11px; }
	.note-copy { min-width: 0; }
	.note-copy h3 { margin: 0 0 4px; font-size: 13px; overflow-wrap: anywhere; }
	.note-copy h3 a { color: var(--ink); text-decoration: none; }
	.note-copy h3 a:hover { text-decoration: underline; }
	.note-copy .note-text { margin: 0; color: var(--text); font-size: 13px; white-space: pre-wrap; overflow-wrap: anywhere; }
	.note-text.clamped { display: -webkit-box; -webkit-line-clamp: 6; line-clamp: 6; -webkit-box-orient: vertical; overflow: hidden; }
	.note-actions { display: flex; align-items: flex-start; gap: 6px; color: var(--muted); }
	.note-actions button { padding: 5px 9px; border: 1px solid var(--line); border-radius: 5px; background: var(--surface); color: var(--ink); font-size: 12px; }
	.note-actions .note-menu-toggle { border: 0; background: transparent; color: var(--muted); font-size: 19px; line-height: 1; }
	.note-actions .danger, .edited { color: var(--error); }
	.edited { font-size: 11px; }
	.more { width: fit-content; padding: 0; border: 0; background: transparent; color: var(--accent); font-size: 12px; }
	.clear-filters { margin-top: 14px; }
	.truncated { margin: 0 0 12px; color: var(--amber); font-size: 12px; }
	.empty-panel { padding: 20px; border: 1px dashed var(--line); border-radius: 8px; background: var(--surface); }
	.empty-panel h2 { margin: 0 0 6px; font-size: 16px; }
	.empty-panel p { margin: 0 0 12px; color: var(--muted); }
	.confirm-label { color: var(--muted); font-size: 12px; }
	@media (max-width: 950px) { .notes-index { padding: 0 22px; } }
	@media (max-width: 700px) {
		.notes-index { max-width: none; padding: 0; }
		.notes-index .title { font-size: 25px; }
		.notes-layout { display: block; }
		.filter { margin-bottom: 15px; }
		.notes-toolbar { align-items: flex-start; flex-direction: column; }
		.note { grid-template-columns: 58px minmax(0, 1fr); gap: 9px; }
		.note-actions { grid-column: 2; }
	}
	.notes-toolbar { margin-bottom: 13px; }
	.notes-results { min-width: 0; }
	.notes-toolbar .btn { white-space: nowrap; }
:global(.notes-layout) {
display: grid;
grid-template-columns: 250px minmax(0,1fr);
gap: 26px;
margin-top: 27px;
}
:global(.filter) {
background: white;
border: 1px solid var(--line);
border-radius: 8px;
padding: 16px;
height: max-content;
}
:global(.filter h3) {
font-size: 14px;
margin: 0 0 12px;
}
:global(.filter label) {
display: block;
color: var(--muted);
font-size: 12px;
margin: 13px 0 5px;
}
:global(.filter .field) {
width: 100%;
}
:global(.note-list) {
display: grid;
gap: 10px;
}
:global(.note) {
display: grid;
grid-template-columns: 72px 1fr auto;
gap: 14px;
background: white;
border: 1px solid var(--line);
border-radius: 7px;
padding: 14px;
align-items: start;
}
:global(.timestamp) {
color: var(--accent);
font-weight: 750;
font-variant-numeric: tabular-nums;
}
:global(.note h3) {
font-size: 13px;
margin: 0 0 4px;
}
:global(.note p) {
font-size: 13px;
color: var(--muted);
margin: 0;
}
:global(.note .small) {
margin-top: 8px;
}
:global(.note-actions) {
color: var(--muted);
}
@media (max-width:700px) {
:global(.notes-layout) {
display: block;
}
}
@media (max-width:700px) {
:global(.filter) {
margin-bottom: 15px;
}
}
@media (max-width:700px) {
:global(.note) {
grid-template-columns: 58px 1fr;
gap: 9px;
}
}
@media (max-width:700px) {
:global(.note-actions) {
grid-column: 2;
}
}
:global(:root[data-theme=dark] .filter) {
background: var(--surface);
border-color: var(--line);
color: var(--text);
}
:global(:root[data-theme=dark] .note) {
background: var(--surface);
border-color: var(--line);
color: var(--text);
}
:global(#app .shell .notes-index) {
width: 100%;
}
:global(.layout > main:has(.notes-index)) {
padding: 0;
}
:global(#app .shell .notes-index) {
max-width: 1240px;
margin: 0 auto;
padding: clamp(36px, 5vw, 64px) clamp(24px, 4vw, 56px) 64px;
}
:global(.notes-index .title) {
margin: 6px 0 8px;
}
:global(.notes-index .notes-header) {
margin: 0 0 34px;
}
:global(.notes-index .notes-layout) {
grid-template-columns: 270px minmax(0, 820px);
justify-content: space-between;
gap: clamp(28px, 5vw, 64px);
margin-top: 0;
}
:global(.notes-index .filter) {
position: sticky;
top: 24px;
padding: 20px;
border: 1px solid var(--line);
border-radius: var(--radius-md);
background: var(--surface);
box-shadow: var(--shadow-sm);
}
:global(.notes-index .filter h3) {
margin-bottom: 18px;
font-size: 16px;
}
:global(.notes-index .filter label) {
margin: 16px 0 7px;
color: var(--ink);
font-size: 13px;
font-weight: 650;
}
:global(.notes-index .filter .field) {
min-height: 42px;
}
:global(.notes-index .notes-results) {
width: 100%;
max-width: 820px;
}
:global(.notes-index .notes-toolbar) {
min-height: 44px;
align-items: center;
margin-bottom: 14px;
padding-bottom: 14px;
border-bottom: 1px solid var(--line);
}
:global(.notes-index .notes-toolbar strong) {
color: var(--ink);
font-size: 14px;
}
:global(.notes-index .note-list) {
gap: 12px;
}
:global(.notes-index .note) {
grid-template-columns: 82px minmax(0, 1fr) auto;
gap: 18px;
padding: 18px;
border: 1px solid var(--line);
border-radius: var(--radius-md);
background: var(--surface);
box-shadow: var(--shadow-sm);
}
:global(.notes-index .note-when) {
padding-top: 2px;
}
:global(.notes-index .timestamp) {
display: inline-flex;
min-height: 30px;
align-items: center;
padding: 0 9px;
border-radius: 999px;
background: var(--surface-accent);
color: var(--accent-strong);
font-size: 12px;
}
:global(.notes-index .note-copy h3) {
margin-bottom: 8px;
font-size: 15px;
}
:global(.notes-index .note-copy .note-text) {
color: var(--ink);
font-size: 14px;
line-height: 1.6;
}
:global(.notes-index .note-copy > time) {
font-size: 12px;
}
:global(.notes-index .edited) {
font-size: 12px;
}
:global(.notes-index .note-actions button) {
min-height: 34px;
border-radius: var(--radius-sm);
}
:global(.notes-index .empty-panel) {
display: grid;
max-width: 760px;
min-height: 210px;
align-content: center;
padding: 32px;
border: 1px solid var(--line);
border-left: 4px solid var(--accent);
border-radius: var(--radius-md);
background: var(--surface);
box-shadow: var(--shadow-sm);
}
:global(.notes-index .empty-panel h2) {
margin-bottom: 8px;
font-size: 20px;
}
:global(.notes-index .empty-panel p) {
max-width: 58ch;
font-size: 14px;
line-height: 1.6;
}
:global(.truncated) {
padding: 11px 14px;
border: 1px solid var(--line);
border-left: 3px solid var(--accent);
border-radius: var(--radius-sm);
background: var(--surface-subtle);
color: var(--muted);
}
@media (max-width: 950px) {
:global(.notes-index .notes-layout) {
grid-template-columns: 240px minmax(0, 1fr);
gap: 24px;
}
}
@media (max-width: 700px) {
:global(#app .shell .notes-index) {
padding: 28px 16px 44px;
}
}
@media (max-width: 700px) {
:global(.notes-index .notes-header) {
margin-bottom: 26px;
}
}
@media (max-width: 700px) {
:global(.notes-index .notes-layout) {
display: block;
}
}
@media (max-width: 700px) {
:global(.notes-index .filter) {
position: static;
margin-bottom: 18px;
}
}
@media (max-width: 700px) {
:global(.notes-index .note) {
grid-template-columns: 1fr;
gap: 10px;
padding: 16px;
}
}
@media (max-width: 700px) {
:global(.notes-index .note-actions) {
justify-content: flex-start;
}
}
@media (max-width: 700px) {
:global(.notes-index .empty-panel) {
min-height: 180px;
padding: 24px 20px;
}
}
:global(.notes-index .empty-panel .btn) {
justify-self: start;
}
:global(.notes-index .empty-panel .text-button) {
justify-self: start;
}
</style>
