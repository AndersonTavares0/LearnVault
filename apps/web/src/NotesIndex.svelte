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
	let isNarrow = $state(false);
	let matchQuery: MediaQueryList | null = null;

	onMount(() => {
		void load();
		// Breakpoint compartilhado (700px): sem leitura de layout por render; o listener
		// é limpo no unmount.
		matchQuery = window.matchMedia('(max-width: 700px)');
		isNarrow = matchQuery.matches;
		const onChange = (event: MediaQueryListEvent) => (isNarrow = event.matches);
		matchQuery.addEventListener('change', onChange);
		return () => matchQuery?.removeEventListener('change', onChange);
	});

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
	<details class="filter-disclosure" open={!isNarrow}>
		<summary>Filtrar anotações</summary>
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
	</details>
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
	.notes-index { max-width: 1240px; margin: 0 auto; padding: clamp(36px, 5vw, 64px) clamp(24px, 4vw, 56px) 64px; }
	.notes-header { margin: 0 0 34px; }
	.notes-index .title { font-size: 32px; line-height: 38px; margin: 6px 0 8px; letter-spacing: -0.025em; color: var(--ink); }
	.notes-index .sub { margin: 0; color: var(--muted); font-size: 15px; }
	.notes-layout { display: grid; grid-template-columns: 250px minmax(0, 1fr); gap: 32px; margin-top: 0; align-items: start; }
	.filter-disclosure summary { display: none; }
	.filter { height: max-content; padding: 16px; border: 1px solid var(--line); border-radius: var(--radius-md); background: var(--surface); }
	.filter h3 { margin: 0 0 12px; font-size: 14px; color: var(--ink); }
	.filter label { display: block; margin: 13px 0 5px; color: var(--muted); font-size: 14px; line-height: 20px; }
	.filter .field { width: 100%; min-height: 44px; border: 1px solid var(--line-strong); border-radius: var(--radius-sm); background: var(--input); color: var(--ink); font-size: 16px; line-height: 24px; }
	.note-list { display: grid; gap: 10px; }
	.note { display: grid; grid-template-columns: 72px minmax(0, 1fr) auto; gap: 14px; border: 1px solid var(--line); border-radius: var(--radius-md); padding: 14px; align-items: start; background: var(--surface); }
	.timestamp { color: var(--accent); font-weight: 700; font-variant-numeric: tabular-nums; text-decoration: none; }
	.timestamp:hover { text-decoration: underline; }
	.note-when { display: grid; justify-items: start; align-content: start; gap: 4px; color: var(--muted); font-size: 13px; line-height: 20px; }
	.note-copy { min-width: 0; }
	.note-copy h3 { margin: 0 0 4px; font-size: 16px; line-height: 24px; overflow-wrap: anywhere; }
	.note-copy h3 a { color: var(--ink); text-decoration: none; }
	.note-copy h3 a:hover { text-decoration: underline; }
	.note-copy .note-text { margin: 0; color: var(--ink); font-size: 16px; line-height: 24px; white-space: pre-wrap; overflow-wrap: anywhere; }
	.note-text.clamped { display: -webkit-box; -webkit-line-clamp: 6; line-clamp: 6; -webkit-box-orient: vertical; overflow: hidden; }
	.note-copy > time { display: block; margin-top: 6px; color: var(--muted); font-size: 13px; line-height: 20px; }
	.note-actions { display: flex; align-items: flex-start; gap: 6px; color: var(--muted); }
	.note-actions button { padding: 6px 9px; border: 1px solid var(--line-strong); border-radius: var(--radius-sm); background: var(--surface); color: var(--ink); font-size: 14px; min-height: 34px; }
	.note-actions .note-menu-toggle { border: 0; background: transparent; color: var(--muted); font-size: 19px; line-height: 1; }
	.note-actions .danger, .edited { color: var(--danger); }
	.edited { font-size: 13px; }
	.more { width: fit-content; padding: 0; border: 0; background: transparent; color: var(--accent); font-size: 14px; }
	.clear-filters { margin-top: 14px; }
	.truncated { margin: 0 0 12px; padding: 11px 14px; border: 1px solid var(--line); border-left: 3px solid var(--accent); border-radius: var(--radius-sm); background: var(--surface-subtle); color: var(--muted); font-size: 14px; }
	.empty-panel { padding: 20px; border: 1px dashed var(--line); border-radius: var(--radius-md); background: var(--surface); }
	.empty-panel h2 { margin: 0 0 6px; font-size: 16px; }
	.empty-panel p { margin: 0 0 12px; color: var(--muted); }
	.empty-panel .btn, .empty-panel .text-button { justify-self: start; }
	.confirm-label { color: var(--muted); font-size: 14px; }
	.notes-results { min-width: 0; }
	.notes-toolbar { align-items: center; min-height: 44px; margin-bottom: 13px; padding-bottom: 13px; border-bottom: 1px solid var(--line); }
	.notes-toolbar strong { color: var(--ink); font-size: 14px; }
	.notes-toolbar .btn { white-space: nowrap; }
	@media (max-width: 950px) { .notes-layout { grid-template-columns: 240px minmax(0, 1fr); gap: 24px; } }
	@media (max-width: 700px) {
		.notes-index { max-width: none; padding: 28px 16px 44px; }
		.notes-index .title { font-size: 28px; line-height: 34px; }
		.notes-layout { display: block; }
		.filter-disclosure summary { display: flex; align-items: center; min-height: 44px; padding: 8px 12px; border: 1px solid var(--line-strong); border-radius: var(--radius-sm); background: var(--surface); color: var(--ink); font-size: 14px; font-weight: 650; margin-bottom: 12px; }
		.filter-disclosure[open] summary { margin-bottom: 8px; }
		.filter { margin-bottom: 15px; }
		.notes-toolbar { align-items: flex-start; flex-direction: column; }
		.note { grid-template-columns: minmax(0, 1fr); gap: 10px; padding: 16px; }
		.note-actions { grid-column: 1; justify-content: flex-start; }
		.empty-panel { min-height: 180px; padding: 24px 20px; }
	}
</style>
