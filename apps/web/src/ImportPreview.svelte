<script lang="ts">
	import type { DiscoveryNode, ImportPreview as ImportPreviewData } from './transport';

	let { preview, onCancel, onConfirm, busy = false }: {
		preview: ImportPreviewData;
		onCancel: () => void;
		onConfirm: () => void;
		busy?: boolean;
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

<section class="import-preview" aria-labelledby="import-preview-heading">
	<div class="import-preview-header">
		<div>
			<p class="eyebrow">Prévia somente leitura</p>
			<h2 id="import-preview-heading">Revisar pasta antes de importar</h2>
			<p class="intro">A estrutura física não será alterada. Sugestões são pistas explicáveis, não classificações definitivas.</p>
		</div>
		<div class="preview-summary" aria-label="Resumo da descoberta">
			<strong>{preview.discovery.summary.found}</strong><span>encontrados</span>
			<strong>{preview.discovery.summary.ignored}</strong><span>ignorados</span>
			<strong>{preview.discovery.summary.inaccessible}</strong><span>inacessíveis</span>
			<strong>{preview.discovery.summary.unsupported}</strong><span>não suportados</span>
		</div>
	</div>
	<div class="preview-columns">
		<div>
			<h3>Estrutura encontrada</h3>
			<ul class="preview-tree" aria-label="Estrutura física da pasta">
				{#each rows(preview.discovery.root) as row (row.node.path)}
					<li class:preview-muted={row.node.status.status !== 'found'} style={`--preview-depth: ${row.depth}`}>
						<span class="preview-tree-name">{row.node.entryKind === 'directory' ? '▸' : '•'} {row.node.name}</span>
						<small>{statusLabel(row.node)}</small>
					</li>
				{/each}
			</ul>
		</div>
		<div>
			<h3>Sugestões explicáveis</h3>
			{#if preview.interpretation.suggestions.length}
				<ul class="preview-suggestions">
					{#each preview.interpretation.suggestions as suggestion}
						<li><strong>{suggestion.proposedRole}</strong> <span>({suggestion.confidence})</span><code>{suggestion.path}</code><p>{suggestion.explanation}</p>{#if suggestion.ambiguous}<small>Revisão necessária: ambíguo.</small>{/if}</li>
					{/each}
				</ul>
			{:else}<p class="empty">Nenhuma sugestão automática foi encontrada.</p>{/if}
		</div>
	</div>
	<div class="import-preview-actions">
		<button type="button" class="text-button" onclick={onCancel} disabled={busy}>Voltar</button>
		<button type="button" class="primary" onclick={onConfirm} disabled={busy} aria-busy={busy}>{busy ? 'Importando…' : 'Confirmar importação'}</button>
	</div>
</section>
