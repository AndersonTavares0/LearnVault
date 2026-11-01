<script lang="ts">
	import type { BotanicalSpeciesId } from './forest-catalog';
	import type { TreeStage } from './forest';

	let {
		stage,
		size = 64
	}: {
		stage: TreeStage | BotanicalSpeciesId;
		size?: number;
	} = $props();
</script>

{#if stage === 'pine'}
	<!-- Pinheiro Nobre 2.5D (Facetas poliédricas com luz e sombra) -->
	<svg
		width={size}
		height={size * 1.3}
		viewBox="0 0 64 84"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-tree-2d5 lv-tree-2d5--pine"
	>
		<!-- Sombra isométrica no solo -->
		<ellipse cx="32" cy="74" rx="18" ry="8" fill="#142918" opacity="0.3" />
		<!-- Tronco 2.5D -->
		<path d="M29 60H32V75H29Z" fill="#854d0e" />
		<path d="M32 60H35V75H32Z" fill="#54300a" />
		<!-- Camada 3 (inferior) -->
		<path d="M12 60L32 38V54L12 60Z" fill="#4ade80" />
		<path d="M32 38L52 60L32 54V38Z" fill="#15803d" />
		<path d="M12 60L32 54L52 60L32 63Z" fill="#14532d" opacity="0.5" />
		<!-- Camada 2 (média) -->
		<path d="M16 46L32 26V40L16 46Z" fill="#4ade80" />
		<path d="M32 26L48 46L32 40V26Z" fill="#16a34a" />
		<path d="M16 46L32 40L48 46L32 49Z" fill="#14532d" opacity="0.5" />
		<!-- Camada 1 (topo) -->
		<path d="M20 32L32 12V26L20 32Z" fill="#86efac" />
		<path d="M32 12L44 32L32 26V12Z" fill="#22c55e" />
		<path d="M20 32L32 26L44 32L32 34Z" fill="#15803d" opacity="0.4" />
	</svg>
{:else if stage === 'oak'}
	<!-- Carvalho Frondoso 2.5D (Tronco esculpido, copa volumétrica e flores) -->
	<svg
		width={size}
		height={size * 1.3}
		viewBox="0 0 64 84"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-tree-2d5 lv-tree-2d5--oak"
	>
		<!-- Sombra isométrica no solo -->
		<ellipse cx="32" cy="74" rx="20" ry="9" fill="#142918" opacity="0.3" />
		<!-- Tronco 2.5D com galhos abertos -->
		<path d="M30 52L29 74H33L33 52Z" fill="#78350f" />
		<path d="M33 52L33 74H36L35 52Z" fill="#451a03" />
		<path d="M30 54L22 44L24 42L31 50Z" fill="#78350f" />
		<path d="M34 52L42 43L44 45L35 55Z" fill="#451a03" />
		<!-- Copas volumétricas em 2.5D -->
		<!-- Fundo/Sombra -->
		<circle cx="22" cy="38" r="14" fill="#15803d" />
		<circle cx="42" cy="38" r="14" fill="#14532d" />
		<circle cx="32" cy="24" r="16" fill="#166534" />
		<!-- Frente/Luz -->
		<circle cx="24" cy="34" r="12" fill="#22c55e" />
		<circle cx="40" cy="34" r="12" fill="#15803d" />
		<circle cx="32" cy="22" r="13" fill="#4ade80" />
		<circle cx="30" cy="18" r="9" fill="#86efac" />
		<!-- Flores e Frutos (estilo clássico Forest) -->
		<circle cx="20" cy="30" r="2.5" fill="#ffffff" />
		<circle cx="20" cy="30" r="1" fill="#facc15" />
		<circle cx="28" cy="22" r="2.5" fill="#f43f5e" />
		<circle cx="38" cy="20" r="2.5" fill="#ffffff" />
		<circle cx="38" cy="20" r="1" fill="#facc15" />
		<circle cx="44" cy="32" r="2.5" fill="#f43f5e" />
		<circle cx="34" cy="34" r="2" fill="#ffffff" />
		<circle cx="26" cy="40" r="2" fill="#f43f5e" />
	</svg>
{:else if stage === 'sakura'}
	<!-- Cerejeira em Flor (Sakura) 2.5D -->
	<svg
		width={size}
		height={size * 1.3}
		viewBox="0 0 64 84"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-tree-2d5 lv-tree-2d5--sakura"
	>
		<!-- Sombra isométrica no solo -->
		<ellipse cx="32" cy="74" rx="20" ry="9" fill="#142918" opacity="0.3" />
		<!-- Tronco 2.5D -->
		<path d="M30 52L29 74H33L33 52Z" fill="#78350f" />
		<path d="M33 52L33 74H36L35 52Z" fill="#451a03" />
		<!-- Copas rosadas em volume 2.5D -->
		<circle cx="22" cy="38" r="14" fill="#db2777" />
		<circle cx="42" cy="38" r="14" fill="#9d174d" />
		<circle cx="32" cy="24" r="16" fill="#be185d" />
		<circle cx="24" cy="34" r="12" fill="#f472b6" />
		<circle cx="40" cy="34" r="12" fill="#db2777" />
		<circle cx="32" cy="22" r="13" fill="#fbcfe8" />
		<circle cx="30" cy="18" r="9" fill="#fdf2f8" />
		<!-- Flores sakura brancas e pétalas flutuantes -->
		<circle cx="18" cy="32" r="2.5" fill="#ffffff" />
		<circle cx="28" cy="24" r="2.5" fill="#fce7f3" />
		<circle cx="38" cy="22" r="2.5" fill="#ffffff" />
		<circle cx="44" cy="34" r="2.5" fill="#fce7f3" />
		<circle cx="32" cy="32" r="2" fill="#ffffff" />
		<circle cx="14" cy="54" r="1.5" fill="#f472b6" />
		<circle cx="48" cy="58" r="1.5" fill="#fbcfe8" />
	</svg>
{:else if stage === 'willow'}
	<!-- Salgueiro Chorão 2.5D -->
	<svg
		width={size}
		height={size * 1.3}
		viewBox="0 0 64 84"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-tree-2d5 lv-tree-2d5--willow"
	>
		<!-- Sombra isométrica no solo -->
		<ellipse cx="32" cy="74" rx="22" ry="9" fill="#142918" opacity="0.3" />
		<!-- Tronco 2.5D -->
		<path d="M30 46V74H33V46Z" fill="#54300a" />
		<path d="M33 46V74H36V46Z" fill="#3b1d03" />
		<!-- Copa de topo -->
		<ellipse cx="32" cy="26" rx="18" ry="12" fill="#047857" />
		<ellipse cx="30" cy="24" rx="14" ry="9" fill="#10b981" />
		<!-- Cascatas de folhagem pendentes 2.5D -->
		<path d="M14 26C12 42 12 58 17 66" stroke="#34d399" stroke-width="3" stroke-linecap="round" />
		<path d="M22 24C19 40 18 60 23 68" stroke="#10b981" stroke-width="3.5" stroke-linecap="round" />
		<path d="M30 22C29 38 28 62 31 70" stroke="#059669" stroke-width="3.5" stroke-linecap="round" />
		<path d="M36 22C37 38 38 62 35 70" stroke="#047857" stroke-width="3.5" stroke-linecap="round" />
		<path d="M44 24C47 40 48 60 43 68" stroke="#065f46" stroke-width="3.5" stroke-linecap="round" />
		<path d="M52 26C54 42 54 58 49 66" stroke="#047857" stroke-width="3" stroke-linecap="round" />
	</svg>
{:else if stage === 'ginkgo'}
	<!-- Ginkgo Dourado 2.5D -->
	<svg
		width={size}
		height={size * 1.3}
		viewBox="0 0 64 84"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-tree-2d5 lv-tree-2d5--ginkgo"
	>
		<!-- Sombra isométrica no solo -->
		<ellipse cx="32" cy="74" rx="20" ry="9" fill="#142918" opacity="0.3" />
		<!-- Tronco 2.5D -->
		<path d="M30 52L29 74H33L33 52Z" fill="#78350f" />
		<path d="M33 52L33 74H36L35 52Z" fill="#451a03" />
		<!-- Copas douradas volumétricas -->
		<circle cx="22" cy="38" r="14" fill="#b45309" />
		<circle cx="42" cy="38" r="14" fill="#78350f" />
		<circle cx="32" cy="24" r="16" fill="#d97706" />
		<circle cx="24" cy="34" r="12" fill="#f59e0b" />
		<circle cx="40" cy="34" r="12" fill="#d97706" />
		<circle cx="32" cy="22" r="13" fill="#fde047" />
		<circle cx="30" cy="18" r="9" fill="#fef08a" />
		<!-- Folhas douradas brilhantes -->
		<circle cx="20" cy="30" r="2.5" fill="#fef08a" />
		<circle cx="38" cy="20" r="2.5" fill="#fef08a" />
		<circle cx="44" cy="32" r="2.5" fill="#fde047" />
	</svg>
{:else if stage === 'ancient'}
	<!-- Árvore Ancestral / Celestial 2.5D (Titã místico com estrelas) -->
	<svg
		width={size}
		height={size * 1.3}
		viewBox="0 0 64 84"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-tree-2d5 lv-tree-2d5--ancient"
	>
		<!-- Sombra isométrica larga -->
		<ellipse cx="32" cy="74" rx="24" ry="10" fill="#142918" opacity="0.35" />
		<!-- Raízes e Tronco largo 2.5D -->
		<path d="M22 74C26 70 28 56 28 46H34C34 56 36 70 40 74Z" fill="#54300a" />
		<path d="M32 46V74H35C35 62 38 68 41 74H43C39 68 36 56 36 46Z" fill="#291404" />
		<!-- Copas exuberantes místicas -->
		<ellipse cx="32" cy="28" rx="24" ry="18" fill="#14532d" />
		<circle cx="18" cy="30" r="13" fill="#15803d" />
		<circle cx="46" cy="30" r="13" fill="#052e16" />
		<circle cx="32" cy="18" r="15" fill="#16a34a" />
		<circle cx="26" cy="22" r="11" fill="#4ade80" />
		<circle cx="38" cy="22" r="11" fill="#22c55e" />
		<circle cx="32" cy="14" r="10" fill="#86efac" />
		<!-- Estrelas celestiais flutuando ao redor -->
		<polygon points="32,2 34,7 39,7 35,10 37,15 32,12 27,15 29,10 25,7 30,7" fill="#facc15" />
		<polygon points="12,18 13,21 16,21 14,23 15,26 12,24 9,26 10,23 8,21 11,21" fill="#fde047" />
		<polygon points="52,20 53,23 56,23 54,25 55,28 52,26 49,28 50,25 48,23 51,23" fill="#fde047" />
	</svg>
{:else if stage === 'sapling'}
	<!-- Brotinho Silvestre 2.5D (Muda jovem em montículo de terra) -->
	<svg
		width={size}
		height={size * 1.3}
		viewBox="0 0 64 84"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-tree-2d5 lv-tree-2d5--sapling"
	>
		<!-- Sombra isométrica no solo -->
		<ellipse cx="32" cy="74" rx="14" ry="6" fill="#142918" opacity="0.3" />
		<!-- Montículo de terra fértil -->
		<ellipse cx="32" cy="71" rx="10" ry="4" fill="#78350f" />
		<ellipse cx="32" cy="70" rx="8" ry="3" fill="#92400e" />
		<!-- Haste jovem 2.5D -->
		<path d="M31 52Q30 64 31 71H33Q33 64 32 52Z" fill="#15803d" />
		<!-- Folha Esquerda (Luz) -->
		<path d="M31 60C20 58 16 46 29 48Z" fill="#86efac" stroke="#4ade80" stroke-width="1.2" />
		<!-- Folha Direita (Sombra) -->
		<path d="M32 54C42 52 46 42 33 44Z" fill="#22c55e" stroke="#15803d" stroke-width="1.2" />
		<!-- Broto topo -->
		<circle cx="31.5" cy="48" r="3.5" fill="#4ade80" />
	</svg>
{:else}
	<!-- Árvore Seca 2.5D (Galhos retorcidos desfolhados com contraste volumétrico) -->
	<svg
		width={size}
		height={size * 1.3}
		viewBox="0 0 64 84"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-tree-2d5 lv-tree-2d5--withered"
	>
		<!-- Sombra seca cinzenta no solo -->
		<ellipse cx="32" cy="74" rx="15" ry="6" fill="#1e293b" opacity="0.25" />
		<!-- Tronco seco com lado de luz e sombra -->
		<path d="M30 46L30 74H33L32 46Z" fill="#64748b" />
		<path d="M32 46L33 74H35L34 46Z" fill="#334155" />
		<!-- Galhos secos retorcidos -->
		<path d="M31 52L21 40L23 38L32 48Z" fill="#64748b" />
		<path d="M21 40L14 34L15 32L23 38Z" fill="#94a3b8" />
		<path d="M33 48L43 38L45 40L34 52Z" fill="#475569" />
		<path d="M43 38L50 30L52 32L45 40Z" fill="#334155" />
		<path d="M31 46L31 30L33 30L33 46Z" fill="#64748b" />
		<path d="M31 34L26 26L28 24L33 32Z" fill="#94a3b8" />
		<path d="M33 32L39 26L41 28L33 34Z" fill="#475569" />
	</svg>
{/if}

<style>
	.lv-tree-2d5 {
		display: block;
		filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.12));
	}
</style>
