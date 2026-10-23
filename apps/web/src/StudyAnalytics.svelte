<script lang="ts">
	import { onMount } from 'svelte';
	import { transport, type StudyAnalytics } from './transport';
	import { knowledgeTree } from './knowledge-tree';

	let stats: StudyAnalytics | null = $state(null);
	let error = $state('');
	onMount(() => { void transport.studyAnalytics().then((value) => stats = value).catch(() => error = 'Não foi possível carregar as estatísticas.'); });
	const duration = (seconds: number) => `${Math.floor(seconds / 3600)} h ${Math.floor((seconds % 3600) / 60)} min`;
</script>


<section class="analytics-page" aria-labelledby="analytics-title">
	<p class="page-eyebrow">Seu ritmo de estudo</p><h1 id="analytics-title">Estatísticas</h1><p class="intro">Acompanhe o que já foi visto e o tempo dedicado à sua biblioteca.</p>
	{#if error}<p role="alert">{error}</p>
	{:else if !stats}<p aria-live="polite">Carregando estatísticas…</p>
	{:else}{@const tree = knowledgeTree(stats.completedCount, stats.lastActivityAt)}
	<dl><div><dt>Aulas acompanhadas</dt><dd>{stats.trackedCount}</dd></div><div><dt>Aulas concluídas</dt><dd>{stats.completedCount}</dd></div><div><dt>Tempo registrado</dt><dd>{duration(stats.totalPositionSeconds)}</dd></div></dl>
	<section class="tree-card" aria-labelledby="tree-title">
		<div class="tree-stage"><img src={tree.art} alt={tree.alt} width="512" height="512" /></div>
		<div>
			<h2 id="tree-title">Árvore do conhecimento</h2>
			<p class="tree-stage-label">{tree.stageLabel}</p>
			<dl class="tree-facts">
				<div><dt>Próximo marco</dt><dd>{tree.milestone}</dd></div>
				<div><dt>Faltam</dt><dd>{tree.remaining}</dd></div>
				<div><dt>Vitalidade</dt><dd>{tree.vitality} · {tree.vitalityHint}</dd></div>
			</dl>
		</div>
	</section>{/if}
</section>

<style>
	.tree-card { display:grid; grid-template-columns:200px minmax(0,1fr); gap:24px; align-items:center; margin-top:22px; padding:22px; border:1px solid var(--library-line); border-radius:14px; background:var(--library-surface); }
	.tree-stage { display:grid; place-items:center; }
	.tree-stage img { width:100%; height:auto; }
	.tree-card h2 { margin:0; color:var(--library-ink); font-size:18px; letter-spacing:-.02em; }
	.tree-stage-label { margin:4px 0 14px; color:var(--library-muted); font-size:13px; }
	.tree-card .tree-facts { display:grid; grid-template-columns:1fr; gap:10px; }
	.tree-facts dt { color:var(--library-muted); font-size:12px; }
	.tree-facts dd { margin:2px 0 0; color:var(--library-ink); font-size:15px; font-weight:600; letter-spacing:0; }
	@media (max-width:760px) { .tree-card { grid-template-columns:1fr; } .tree-stage img { max-width:170px; } }
</style>
