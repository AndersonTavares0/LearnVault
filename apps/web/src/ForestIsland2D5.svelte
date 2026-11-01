<script lang="ts">
	import {
		calculateGridDimension,
		calculateIslandGeometry,
		gridToIso,
		layoutTreesOnIsland
	} from './forest-island';
	import ForestTree2D5Svg from './ForestTree2D5Svg.svelte';
	import type { ForestStats, ForestTree } from './forest';

	let {
		trees = [],
		stats,
		selectedTree = null,
		onselect
	}: {
		trees: ForestTree[];
		stats: ForestStats;
		selectedTree?: ForestTree | null;
		onselect?: (tree: ForestTree) => void;
	} = $props();

	const tileWidth = 64;
	const tileHeight = 32;
	const TREE_SIZE = 64;
	// IDs únicos por instância para evitar colisão de gradientes SVG
	const uid = $props.id();
	const cliffLeftId = `lv-cliff-left-${uid}`;
	const cliffRightId = `lv-cliff-right-${uid}`;
	const shadowId = `lv-island-shadow-${uid}`;

	const dimension = $derived(calculateGridDimension(trees.length));
	const geometry = $derived(calculateIslandGeometry(dimension, tileWidth, tileHeight, 52));
	const placedTrees = $derived(layoutTreesOnIsland(trees, dimension, tileWidth, tileHeight));
	const hiddenCount = $derived(Math.max(0, trees.length - placedTrees.length));

	// Mapa de posições ocupadas para decorar tiles vazios com pequenas flores silvestres
	const occupiedSpots = $derived(new Set(placedTrees.map((p) => `${p.row},${p.col}`)));

	// Lista de todos os tiles do grid
	const allTiles = $derived.by(() => {
		const tiles = [];
		for (let r = 0; r < dimension; r++) {
			for (let c = 0; c < dimension; c++) {
				const iso = gridToIso(r, c, tileWidth, tileHeight);
				const isOccupied = occupiedSpots.has(`${r},${c}`);
				const isAlternate = (r + c) % 2 === 1;
				// Determinismo pseudo-aleatório para pequenas flores em solo vazio
				const seed = (r * 7 + c * 13) % 7;
				const hasFlower = !isOccupied && seed === 2;
				const hasClover = !isOccupied && seed === 5;
				tiles.push({
					row: r,
					col: c,
					x: iso.x,
					y: iso.y,
					depth: iso.depth,
					isAlternate,
					isOccupied,
					hasFlower,
					hasClover
				});
			}
		}
		return tiles;
	});

	function handleTreeClick(tree: ForestTree) {
		if (onselect) {
			onselect(tree);
		}
	}

	function handleKeydown(event: KeyboardEvent, tree: ForestTree) {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			handleTreeClick(tree);
		}
	}
</script>

<div class="lv-island-card" role="region" aria-label="Ilha 2.5D do bosque">
	<div class="lv-island-sky">
		<!-- Partículas atmosféricas suaves -->
		<div class="lv-island-orb lv-island-orb--1" aria-hidden="true"></div>
		<div class="lv-island-orb lv-island-orb--2" aria-hidden="true"></div>

		<!-- SVG da Ilha Isométrica 2.5D -->
		<svg
			class="lv-island-svg"
			viewBox={geometry.viewBox}
			fill="none"
			xmlns="http://www.w3.org/2000/svg"
			role="group"
			aria-label="Ilha isométrica com {trees.length} árvores"
		>
			<defs>
				<!-- Gradiente Falésia Esquerda (Sombra/Terra rochosa) -->
				<linearGradient id={cliffLeftId} x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stop-color="var(--lv-iso-cliff-left-top, #4a2e1b)" />
					<stop offset="100%" stop-color="var(--lv-iso-cliff-left-bot, #2e1a0f)" />
				</linearGradient>

				<!-- Gradiente Falésia Direita (Luz/Terra rochosa iluminada) -->
				<linearGradient id={cliffRightId} x1="0" y1="0" x2="0" y2="1">
					<stop offset="0%" stop-color="var(--lv-iso-cliff-right-top, #70472e)" />
					<stop offset="100%" stop-color="var(--lv-iso-cliff-right-bot, #4e301d)" />
				</linearGradient>

				<!-- Gradiente de sombra flutuante no ar -->
				<radialGradient id={shadowId} cx="50%" cy="50%" r="50%">
					<stop offset="0%" stop-color="#000" stop-opacity="0.35" />
					<stop offset="70%" stop-color="#000" stop-opacity="0.12" />
					<stop offset="100%" stop-color="#000" stop-opacity="0" />
				</radialGradient>
			</defs>

			<!-- 1. Sombra projetada no ar sob a ilha flutuante -->
			<ellipse
				cx="0"
				cy={geometry.bottomVertex.y + geometry.cliffHeight + 20}
				rx={dimension * 32}
				ry={dimension * 10}
				fill="url(#{shadowId})"
			/>

			<!-- 2. Paredes de Rocha e Terra (Falésia 2.5D inferior) -->
			<!-- Falésia Esquerda (Face de sombra) -->
			<path d={geometry.leftCliffPath} fill="url(#{cliffLeftId})" />
			<path d={geometry.leftCliffShadePath} fill="#1d1009" opacity="0.25" />

			<!-- Estratos rochosos na face esquerda -->
			<path
				d="M {geometry.leftVertex.x + 20} {geometry.leftVertex.y + 18} L {geometry.bottomVertex.x} {geometry.bottomVertex.y + 18}"
				stroke="#27150c"
				stroke-width="2"
				opacity="0.6"
			/>
			<path
				d="M {geometry.leftVertex.x + 35} {geometry.leftVertex.y + 36} L {geometry.bottomVertex.x} {geometry.bottomVertex.y + 36}"
				stroke="#1f1008"
				stroke-width="2.5"
				opacity="0.7"
			/>

			<!-- Falésia Direita (Face iluminada) -->
			<path d={geometry.rightCliffPath} fill="url(#{cliffRightId})" />
			<path d={geometry.rightCliffShadePath} fill="#8c5839" opacity="0.2" />

			<!-- Estratos rochosos na face direita -->
			<path
				d="M {geometry.bottomVertex.x} {geometry.bottomVertex.y + 20} L {geometry.rightVertex.x - 20} {geometry.rightVertex.y + 20}"
				stroke="#53331f"
				stroke-width="2"
				opacity="0.5"
			/>
			<path
				d="M {geometry.bottomVertex.x} {geometry.bottomVertex.y + 38} L {geometry.rightVertex.x - 30} {geometry.rightVertex.y + 38}"
				stroke="#422716"
				stroke-width="2.5"
				opacity="0.6"
			/>

			<!-- Borda inferior esculpida da ilha -->
			<path
				d="M {geometry.leftVertex.x} {geometry.leftVertex.y + geometry.cliffHeight} L {geometry.bottomVertex.x} {geometry.bottomVertex.y + geometry.cliffHeight} L {geometry.rightVertex.x} {geometry.rightVertex.y + geometry.cliffHeight}"
				stroke="#1a0c06"
				stroke-width="2"
				fill="none"
				opacity="0.7"
			/>

			<!-- Franja orgânica de grama pendente nas bordas da falésia -->
			{#each Array(dimension) as _, i}
				<!-- Grama pendente na borda esquerda -->
				{@const segStartX = geometry.leftVertex.x + (i * tileWidth) / 2}
				{@const segStartY = geometry.leftVertex.y + (i * tileHeight) / 2}
				{@const segMidX = segStartX + tileWidth / 4}
				{@const segMidY = segStartY + tileHeight / 4 + 4}
				{@const segEndX = segStartX + tileWidth / 2}
				{@const segEndY = segStartY + tileHeight / 2}
				<path
					d="M {segStartX} {segStartY} Q {segMidX} {segMidY} {segEndX} {segEndY}"
					fill="none"
					stroke="var(--lv-iso-grass-edge, #5da026)"
					stroke-width="4"
					stroke-linecap="round"
				/>

				<!-- Grama pendente na borda direita -->
				{@const rSegStartX = geometry.bottomVertex.x + (i * tileWidth) / 2}
				{@const rSegStartY = geometry.bottomVertex.y - (i * tileHeight) / 2}
				{@const rSegMidX = rSegStartX + tileWidth / 4}
				{@const rSegMidY = rSegStartY - tileHeight / 4 + 4}
				{@const rSegEndX = rSegStartX + tileWidth / 2}
				{@const rSegEndY = rSegStartY - tileHeight / 2}
				<path
					d="M {rSegStartX} {rSegStartY} Q {rSegMidX} {rSegMidY} {rSegEndX} {rSegEndY}"
					fill="none"
					stroke="var(--lv-iso-grass-edge, #5da026)"
					stroke-width="4"
					stroke-linecap="round"
				/>
			{/each}

			<!-- 3. Superfície Verde Isométrica (Tabuleiro de Grama) -->
			<g class="lv-island-surface">
				{#each allTiles as tile}
					<!-- Losango do tile isométrico -->
					<path
						d="M {tile.x} {tile.y - tileHeight / 2} L {tile.x + tileWidth / 2} {tile.y} L {tile.x} {tile.y + tileHeight / 2} L {tile.x - tileWidth / 2} {tile.y} Z"
						fill={tile.isAlternate ? 'var(--lv-iso-tile-b, #7bb934)' : 'var(--lv-iso-tile-a, #88c93b)'}
						stroke="var(--lv-iso-tile-border, #9bd74e)"
						stroke-width="0.8"
						class="lv-island-tile"
					/>

					<!-- Detalhes de flores silvestres em tiles vazios (como no Forest) -->
					{#if tile.hasFlower}
						<!-- Pequena flor silvestre branca/amarela -->
						<circle cx={tile.x - 4} cy={tile.y} r="2" fill="#ffffff" opacity="0.9" />
						<circle cx={tile.x - 4} cy={tile.y} r="1" fill="#facc15" />
						<circle cx={tile.x + 6} cy={tile.y + 2} r="1.8" fill="#fef08a" opacity="0.85" />
					{:else if tile.hasClover}
						<!-- Trevinho verde escuro decorativo -->
						<circle cx={tile.x + 3} cy={tile.y - 1} r="1.5" fill="#3f721a" opacity="0.8" />
						<circle cx={tile.x + 6} cy={tile.y - 1} r="1.5" fill="#3f721a" opacity="0.8" />
					{/if}
				{/each}
			</g>

			<!-- 4. Árvores e Elementos Botânicos Ordenados por Profundidade (Algoritmo do Pintor) -->
			<g class="lv-island-trees">
				{#each placedTrees as placed (placed.tree.sessionId)}
					{@const isSelected = selectedTree?.sessionId === placed.tree.sessionId}
					<!-- Grupo de Árvore Isométrica interativo -->
					<g
						class="lv-island-tree-group"
						class:is-selected={isSelected}
						class:is-withered={!placed.tree.completed}
						role="button"
						tabindex="0"
						aria-label="{placed.tree.stageName}, {placed.tree.minutes} minutos, {placed.tree.completed ? 'sadia' : 'seca'}"
						aria-pressed={isSelected}
						onclick={() => handleTreeClick(placed.tree)}
						onkeydown={(e) => handleKeydown(e, placed.tree)}
					>
						<!-- Halo de seleção brilhante sob o solo quando clicada -->
						{#if isSelected}
							<ellipse
								cx={placed.isoX}
								cy={placed.isoY}
								rx={tileWidth * 0.42}
								ry={tileHeight * 0.42}
								fill="var(--lv-iso-select-halo, rgba(250, 204, 21, 0.4))"
								stroke="var(--lv-iso-select-border, #facc15)"
								stroke-width="2.5"
								class="lv-island-halo"
							/>
						{/if}

						<!-- Área de foco/clique no tile (anel visível via teclado) -->
						<path
							class="lv-island-tree-hit"
							d="M {placed.isoX} {placed.isoY - tileHeight / 2} L {placed.isoX + tileWidth / 2} {placed.isoY} L {placed.isoX} {placed.isoY + tileHeight / 2} L {placed.isoX - tileWidth / 2} {placed.isoY} Z"
							fill="transparent"
						/>

						<!-- Vetor 2.5D alinhado ao ponto de apoio do tronco (32, 74 no viewBox 64x84) -->
						<g transform="translate({placed.isoX - TREE_SIZE / 2}, {placed.isoY - (74 * TREE_SIZE * 1.3) / 84})">
							<g class="lv-island-tree-lift">
								<ForestTree2D5Svg stage={placed.tree.stage} size={TREE_SIZE} />
							</g>
						</g>
					</g>
				{/each}
			</g>
		</svg>
		{#if hiddenCount > 0}
			<p class="lv-island-overflow">+{hiddenCount} árvores além da ilha — veja todas na Grade</p>
		{/if}
	</div>

	<!-- 5. Barra Inferior de Métricas (Fiel ao App Forest) -->
	<footer class="lv-island-footer" aria-label="Resumo do bosque">
		<div class="lv-island-footer__time">
			<span class="lv-island-footer__time-value">{stats.totalMinutes}</span>
			<span class="lv-island-footer__time-unit">mins</span>
		</div>

		<div class="lv-island-footer__badges">
			<!-- Contador de Árvores Sadias com Emblema de Folha Verde -->
			<div class="lv-island-badge lv-island-badge--healthy" title="{stats.healthyTrees} árvores sadias florescidas">
				<span class="lv-island-badge__count">{stats.healthyTrees}</span>
				<span class="lv-island-badge__icon" aria-hidden="true">
					<svg viewBox="0 0 24 24" width="22" height="22" fill="none">
						<circle cx="12" cy="12" r="10.5" fill="#2d6a3f" stroke="#52b788" stroke-width="1.5" />
						<path
							d="M12 5C8.5 7.5 7 11.5 8 15.5C9.5 17 11.5 17.5 14 17C16.5 15.5 17.5 11.5 16 7.5C14 6 12.8 5.2 12 5Z"
							fill="#95d5b2"
						/>
						<path d="M12 7.5V17" stroke="#1b4332" stroke-width="1.2" stroke-linecap="round" />
					</svg>
				</span>
			</div>

			<!-- Contador de Árvores Secas com Emblema de Folha Seca -->
			<div class="lv-island-badge lv-island-badge--withered" title="{stats.witheredTrees} árvores secas">
				<span class="lv-island-badge__count">{stats.witheredTrees}</span>
				<span class="lv-island-badge__icon" aria-hidden="true">
					<svg viewBox="0 0 24 24" width="22" height="22" fill="none">
						<circle cx="12" cy="12" r="10.5" fill="#78350f" stroke="#b45309" stroke-width="1.5" />
						<path
							d="M12 5C8.5 7.5 7 11.5 8 15.5C9.5 17 11.5 17.5 14 17C16.5 15.5 17.5 11.5 16 7.5C14 6 12.8 5.2 12 5Z"
							fill="#fde68a"
						/>
						<path d="M12 7.5V17" stroke="#92400e" stroke-width="1.2" stroke-linecap="round" />
					</svg>
				</span>
			</div>
		</div>
	</footer>
</div>

<style>
	:global(:root) {
		/* Cores Tema Claro (Morning Forest) */
		--lv-iso-card-bg: linear-gradient(180deg, #eaf5ec 0%, #d3e8d8 100%);
		--lv-iso-footer-bg: rgba(255, 255, 255, 0.65);
		--lv-iso-footer-ink: #14301f;
		--lv-iso-footer-muted: #3d5a48;
		--lv-iso-footer-line: rgba(20, 48, 31, 0.12);
		--lv-iso-tile-a: #89cb39;
		--lv-iso-tile-b: #7cbe2d;
		--lv-iso-tile-border: #99d84c;
		--lv-iso-grass-edge: #5fa428;
		--lv-iso-cliff-left-top: #4a2e1b;
		--lv-iso-cliff-left-bot: #2c1a0e;
		--lv-iso-cliff-right-top: #73482f;
		--lv-iso-cliff-right-bot: #4f301d;
		--lv-iso-select-halo: rgba(250, 204, 21, 0.45);
		--lv-iso-select-border: #facc15;
	}

	:global(:root[data-theme='dark']) {
		/* Cores Tema Escuro (Twilight Sanctuary) */
		--lv-iso-card-bg: linear-gradient(180deg, #183325 0%, #0e1e16 100%);
		--lv-iso-footer-bg: rgba(0, 0, 0, 0.25);
		--lv-iso-footer-ink: #ffffff;
		--lv-iso-footer-muted: rgba(255, 255, 255, 0.8);
		--lv-iso-footer-line: rgba(255, 255, 255, 0.08);
		--lv-iso-tile-a: #7fbf35;
		--lv-iso-tile-b: #72b02b;
		--lv-iso-tile-border: #8fcb47;
		--lv-iso-grass-edge: #56962a;
	}

	.lv-island-card {
		display: flex;
		flex-direction: column;
		background: var(--lv-iso-card-bg);
		border-radius: 1.25rem;
		box-shadow:
			0 12px 32px -4px rgba(0, 0, 0, 0.35),
			inset 0 1px 0 rgba(255, 255, 255, 0.1);
		overflow: hidden;
		position: relative;
		border: 1px solid rgba(255, 255, 255, 0.08);
	}

	.lv-island-sky {
		position: relative;
		width: 100%;
		padding: 1.5rem 1rem 0.5rem;
		display: flex;
		justify-content: center;
		align-items: center;
		min-height: 380px;
	}

	.lv-island-orb {
		position: absolute;
		border-radius: 50%;
		filter: blur(50px);
		pointer-events: none;
	}

	.lv-island-orb--1 {
		top: 10%;
		left: 15%;
		width: 160px;
		height: 160px;
		background: rgba(74, 222, 128, 0.08);
	}

	.lv-island-orb--2 {
		bottom: 20%;
		right: 12%;
		width: 180px;
		height: 180px;
		background: rgba(45, 106, 79, 0.12);
	}

	.lv-island-svg {
		width: 100%;
		max-width: 720px;
		height: auto;
		max-height: 520px;
		overflow: visible;
		display: block;
	}

	.lv-island-tree-group:focus-visible .lv-island-tree-hit {
		fill: rgba(250, 204, 21, 0.35);
		stroke: #14301f;
		stroke-width: 3;
		paint-order: stroke;
	}

	.lv-island-tree-lift {
		transform-box: fill-box;
		transform-origin: bottom center;
		transition: transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
	}

	.lv-island-tree-group:hover .lv-island-tree-lift,
	.lv-island-tree-group:focus-visible .lv-island-tree-lift {
		transform: translateY(-4px) scale(1.06);
	}

	.lv-island-halo {
		transform-box: fill-box;
		transform-origin: center;
	}

	.lv-island-overflow {
		position: absolute;
		bottom: 0.5rem;
		left: 50%;
		transform: translateX(-50%);
		margin: 0;
		padding: 0.25rem 0.75rem;
		border-radius: 9999px;
		font-size: 0.75rem;
		font-weight: 600;
		background: var(--lv-iso-footer-bg);
		color: var(--lv-iso-footer-ink);
	}

	@media (prefers-reduced-motion: reduce) {
		.lv-island-tree-lift {
			transition: none;
		}
		.lv-island-tree-group:hover .lv-island-halo,
		.lv-island-tree-group:focus-visible .lv-island-halo {
			animation: none;
		}
	}

	.lv-island-tile {
		transition: fill 0.2s ease;
	}

	.lv-island-tree-group {
		cursor: pointer;
		outline: none;
	}

	.lv-island-tree-group:focus-visible .lv-island-halo,
	.lv-island-tree-group:hover .lv-island-halo {
		animation: lv-pulse-halo 1.5s infinite alternate ease-in-out;
	}

	@keyframes lv-pulse-halo {
		0% {
			transform: scale(0.97);
			opacity: 0.7;
		}
		100% {
			transform: scale(1.05);
			opacity: 1;
		}
	}

	/* Footer Bar do Estilo Forest App */
	.lv-island-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 1rem 1.5rem 1.25rem;
		background: var(--lv-iso-footer-bg);
		backdrop-filter: blur(8px);
		border-top: 1px solid var(--lv-iso-footer-line);
	}

	.lv-island-footer__time {
		display: flex;
		align-items: baseline;
		gap: 0.35rem;
		color: var(--lv-iso-footer-ink);
	}

	.lv-island-footer__time-value {
		font-size: 1.5rem;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
		letter-spacing: -0.02em;
	}

	.lv-island-footer__time-unit {
		font-size: 0.95rem;
		font-weight: 500;
		color: var(--lv-iso-footer-muted);
	}

	.lv-island-footer__badges {
		display: flex;
		align-items: center;
		gap: 1.25rem;
	}

	.lv-island-badge {
		display: inline-flex;
		align-items: center;
		gap: 0.45rem;
	}

	.lv-island-badge__count {
		font-size: 1.25rem;
		font-weight: 700;
		font-variant-numeric: tabular-nums;
		color: var(--lv-iso-footer-ink);
	}

	.lv-island-badge__icon {
		display: flex;
		align-items: center;
		justify-content: center;
		filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.3));
	}

	@media (max-width: 640px) {
		.lv-island-sky {
			min-height: 320px;
			padding: 1rem 0.5rem 0;
		}

		.lv-island-footer {
			padding: 0.85rem 1rem 1rem;
		}

		.lv-island-footer__time-value {
			font-size: 1.25rem;
		}

		.lv-island-badge__count {
			font-size: 1.1rem;
		}
	}
</style>
