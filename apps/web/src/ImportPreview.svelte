<script lang="ts">
	import ImportSteps from './ImportSteps.svelte';
	import type { DiscoveryNode, ImportPreview as ImportPreviewData } from './transport';

	let { preview, onCancel, onConfirm, busy = false, step = 3 as 3 | 4 | 5, error = '', onRetryRescan = null as null | (() => void) }: {
		preview: ImportPreviewData;
		onCancel: () => void;
		onConfirm: () => void;
		busy?: boolean;
		step?: 3 | 4 | 5;
		error?: string;
		onRetryRescan?: null | (() => void);
	} = $props();

	type Row = { node: DiscoveryNode; depth: number };
	function rows(node: DiscoveryNode, depth = 0): Row[] {
		return [{ node, depth }, ...node.children.flatMap((child) => rows(child, depth + 1))];
	}

	function statusLabel(node: DiscoveryNode): string {
		if (node.status.status === 'found') return node.fileKind ? `encontrado · ${node.fileKind}` : 'encontrado';
		return `${node.status.status} · ${node.status.reason}`;
	}
</script>

<section class="catalog-screen flow-screen" aria-labelledby="import-preview-heading">
	<div class="catalog-lead">
		<p class="eyebrow">Descoberta · prévia somente leitura</p>
		<h2 id="import-preview-heading">Revise a pasta antes de importar</h2>
		<p>A estrutura física não será alterada. Sugestões são explicáveis e não classificações definitivas.</p>
	</div>
	<ImportSteps currentStep={step} />
	<div class="scan-summary" aria-label="Resumo da descoberta">
		<div class="scan-count"><strong>{preview.discovery.summary.found}</strong> encontrados</div>
		<div class="scan-count"><strong>{preview.discovery.summary.ignored}</strong> ignorados</div>
		<div class="scan-count"><strong>{preview.discovery.summary.inaccessible}</strong> inacessíveis</div>
		<div class="scan-count"><strong>{preview.discovery.summary.unsupported}</strong> não suportados</div>
	</div>
	<div class="tree-compare">
		<section class="tree-card" aria-labelledby="preview-tree-heading">
			<h3 id="preview-tree-heading">Estrutura física encontrada</h3>
			<ul class="preview-tree" aria-label="Estrutura física da pasta">
				{#each rows(preview.discovery.root) as row (row.node.path)}
					<li class:preview-muted={row.node.status.status !== 'found'} style={`--preview-depth: ${row.depth}`}>
						<span class="preview-tree-name">{row.node.entryKind === 'directory' ? '▸' : '•'} {row.node.name}</span>
						<small>{statusLabel(row.node)}</small>
					</li>
				{/each}
			</ul>
		</section>
		<section class="tree-card evidence" aria-labelledby="preview-evidence-heading">
			<h3 id="preview-evidence-heading">Sugestões e evidências</h3>
			{#if preview.interpretation.suggestions.length}
				<ul class="preview-suggestions">
					{#each preview.interpretation.suggestions as suggestion}
						<li><strong>{suggestion.proposedRole}</strong> <span>({suggestion.confidence})</span><code>{suggestion.path}</code><p>{suggestion.explanation}</p>{#if suggestion.ambiguous}<small>Revisão necessária: ambíguo.</small>{/if}</li>
					{/each}
				</ul>
			{:else}<p class="empty">Nenhuma sugestão automática foi encontrada.</p>{/if}
		</section>
	</div>
	<div class="import-preview-actions">
		{#if step === 5 && onRetryRescan}
			<p class="rescan-error" role="alert"><strong>Curso registrado; não foi possível atualizar o catálogo.</strong> {error}</p>
			<button type="button" class="primary" onclick={onRetryRescan}>Tentar atualizar catálogo</button>
		{:else}
			<button type="button" class="text-button" onclick={onCancel} disabled={busy}>Cancelar</button>
			<button type="button" class="primary" onclick={onConfirm} disabled={busy} aria-busy={busy}>{busy ? 'Importando…' : 'Confirmar importação'}</button>
		{/if}
	</div>
</section>
<style>
	.preview-tree,.preview-suggestions{list-style:none;margin:0;padding:0}
	.preview-tree{font:12px/1.5 ui-monospace,SFMono-Regular,Consolas,monospace}
	.preview-tree li{display:flex;justify-content:space-between;gap:12px;padding:7px 9px 7px calc(9px + var(--preview-depth)*18px);border-bottom:1px solid var(--line)}
	.preview-tree li:last-child{border-bottom:0}
	.preview-tree-name{overflow-wrap:anywhere}
	.preview-tree small,.preview-muted{color:var(--muted)}
	.preview-muted .preview-tree-name{color:var(--muted)}
	.preview-suggestions{display:grid;gap:9px}
	.preview-suggestions li{padding:10px;border:1px solid var(--line);border-radius:6px;background:var(--surface);font-size:12px}
	.preview-suggestions code{display:block;margin-top:5px;overflow-wrap:anywhere;color:var(--muted)}
	.preview-suggestions p{margin:6px 0 0}
	.preview-suggestions small{display:block;margin-top:6px;color:var(--amber)}
	.import-preview-actions{display:flex;justify-content:flex-end;gap:9px;margin-top:16px;align-items:center;flex-wrap:wrap}
	.rescan-error{margin:0;display:grid;gap:4px;padding:10px 12px;border:1px solid var(--line);border-left:3px solid var(--danger);border-radius:var(--radius-sm);background:var(--surface-subtle);color:var(--muted);font-size:13px;flex:1;min-width:0}
	.rescan-error strong{color:var(--ink)}
	@media(max-width:700px){.import-preview-actions{flex-direction:column-reverse}.import-preview-actions button{width:100%}}
</style>
