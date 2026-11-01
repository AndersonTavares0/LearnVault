<script lang="ts">
	import type { TreeStage } from './forest';

	let { stage, size = 56 }: { stage: TreeStage; size?: number } = $props();
</script>

{#if stage === 'sapling'}
	<!-- Brotinho / Muda: haste jovem com folhas suaves -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-forest-tree lv-forest-tree--sapling"
	>
		<!-- Chão -->
		<ellipse cx="32" cy="56" rx="14" ry="4" fill="currentColor" opacity="0.15" />
		<!-- Haste / Caule -->
		<path d="M32 54C32 40 31 34 32 24" stroke="#5d4037" stroke-width="3" stroke-linecap="round" />
		<!-- Folha Esquerda -->
		<path
			d="M32 38C22 38 20 28 32 30"
			fill="#66bb6a"
			stroke="#388e3c"
			stroke-width="1.5"
			stroke-linejoin="round"
		/>
		<!-- Folha Direita -->
		<path
			d="M32 32C42 32 44 22 32 24"
			fill="#81c784"
			stroke="#388e3c"
			stroke-width="1.5"
			stroke-linejoin="round"
		/>
		<!-- Folha Topo / Broto -->
		<path
			d="M32 24C30 14 34 14 32 24"
			fill="#4caf50"
			stroke="#2e7d32"
			stroke-width="1.5"
			stroke-linejoin="round"
		/>
	</svg>
{:else if stage === 'oak'}
	<!-- Carvalho Frondoso: tronco resistente e copa copada circular -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-forest-tree lv-forest-tree--oak"
	>
		<!-- Sombra do solo -->
		<ellipse cx="32" cy="57" rx="18" ry="4" fill="currentColor" opacity="0.18" />
		<!-- Tronco -->
		<path
			d="M27 56L29 40C29 36 26 33 24 30M35 40L37 56M35 40C35 36 38 33 40 30M32 40V32"
			stroke="#5d4037"
			stroke-width="3.5"
			stroke-linecap="round"
			stroke-linejoin="round"
		/>
		<!-- Copa (camada de trás/sombra) -->
		<circle cx="23" cy="28" r="11" fill="#2e7d32" />
		<circle cx="41" cy="28" r="11" fill="#2e7d32" />
		<circle cx="32" cy="19" r="13" fill="#388e3c" />
		<!-- Copa (camada frontal iluminada) -->
		<circle cx="26" cy="25" r="9" fill="#4caf50" />
		<circle cx="38" cy="25" r="9" fill="#4caf50" />
		<circle cx="32" cy="18" r="10" fill="#66bb6a" />
	</svg>
{:else if stage === 'pine'}
	<!-- Pinheiro Nobre: tronco reto e camadas de conífera -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-forest-tree lv-forest-tree--pine"
	>
		<!-- Sombra do solo -->
		<ellipse cx="32" cy="57" rx="16" ry="3.5" fill="currentColor" opacity="0.18" />
		<!-- Tronco -->
		<path d="M32 56V42" stroke="#4e342e" stroke-width="4" stroke-linecap="round" />
		<!-- Nível inferior da conífera -->
		<path
			d="M17 44L32 29L47 44C43 45 21 45 17 44Z"
			fill="#1b5e20"
			stroke="#0e3d13"
			stroke-width="1.2"
			stroke-linejoin="round"
		/>
		<!-- Nível médio -->
		<path
			d="M21 34L32 20L43 34C39 35 25 35 21 34Z"
			fill="#2e7d32"
			stroke="#1b5e20"
			stroke-width="1.2"
			stroke-linejoin="round"
		/>
		<!-- Nível topo -->
		<path
			d="M25 24L32 10L39 24C36 25 28 25 25 24Z"
			fill="#388e3c"
			stroke="#2e7d32"
			stroke-width="1.2"
			stroke-linejoin="round"
		/>
	</svg>
{:else if stage === 'ancient'}
	<!-- Árvore Ancestral: tronco vigoroso com raízes expostas e copa majestosa -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-forest-tree lv-forest-tree--ancient"
	>
		<!-- Sombra do solo -->
		<ellipse cx="32" cy="58" rx="22" ry="4" fill="currentColor" opacity="0.22" />
		<!-- Raízes e Tronco -->
		<path
			d="M20 57C26 55 28 44 28 36M44 57C38 55 36 44 36 36M32 54V34"
			stroke="#3e2723"
			stroke-width="4.5"
			stroke-linecap="round"
		/>
		<!-- Copa de fundo -->
		<ellipse cx="32" cy="24" rx="24" ry="15" fill="#1b5e20" />
		<!-- Nuvens de folhagem intermédia -->
		<circle cx="18" cy="24" r="10" fill="#2e7d32" />
		<circle cx="46" cy="24" r="10" fill="#2e7d32" />
		<circle cx="32" cy="17" r="13" fill="#388e3c" />
		<!-- Realces da copa -->
		<circle cx="25" cy="20" r="8" fill="#43a047" />
		<circle cx="39" cy="20" r="8" fill="#43a047" />
		<circle cx="32" cy="14" r="8" fill="#66bb6a" />
	</svg>
{:else}
	<!-- Árvore Seca / Murcha: tronco e galhos desfolhados sem vida -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-forest-tree lv-forest-tree--withered"
	>
		<!-- Sombra do solo tênue -->
		<ellipse cx="32" cy="57" rx="14" ry="3" fill="currentColor" opacity="0.12" />
		<!-- Tronco seco e retorcido -->
		<path
			d="M32 56L31 38C30 32 34 26 30 18M31 38L23 28M25 28L18 24M31 32L39 26M37 26L44 20M30 22L36 16"
			stroke="#78909c"
			stroke-width="2.5"
			stroke-linecap="round"
			stroke-linejoin="round"
		/>
	</svg>
{/if}
