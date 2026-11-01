<script lang="ts">
	import type { BotanicalSpeciesId } from './forest-catalog';
	import type { TreeStage } from './forest';

	let { stage, size = 56 }: { stage: TreeStage | BotanicalSpeciesId; size?: number } = $props();
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
		<ellipse cx="32" cy="56" rx="14" ry="4" fill="currentColor" opacity="0.15" />
		<path d="M32 54C32 40 31 34 32 24" stroke="#5d4037" stroke-width="3" stroke-linecap="round" />
		<path d="M32 38C22 38 20 28 32 30" fill="#66bb6a" stroke="#388e3c" stroke-width="1.5" stroke-linejoin="round" />
		<path d="M32 32C42 32 44 22 32 24" fill="#81c784" stroke="#388e3c" stroke-width="1.5" stroke-linejoin="round" />
		<path d="M32 24C30 14 34 14 32 24" fill="#4caf50" stroke="#2e7d32" stroke-width="1.5" stroke-linejoin="round" />
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
		<ellipse cx="32" cy="57" rx="18" ry="4" fill="currentColor" opacity="0.18" />
		<path
			d="M27 56L29 40C29 36 26 33 24 30M35 40L37 56M35 40C35 36 38 33 40 30M32 40V32"
			stroke="#5d4037"
			stroke-width="3.5"
			stroke-linecap="round"
			stroke-linejoin="round"
		/>
		<circle cx="23" cy="28" r="11" fill="#2e7d32" />
		<circle cx="41" cy="28" r="11" fill="#2e7d32" />
		<circle cx="32" cy="19" r="13" fill="#388e3c" />
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
		<ellipse cx="32" cy="57" rx="16" ry="3.5" fill="currentColor" opacity="0.18" />
		<path d="M32 56V42" stroke="#4e342e" stroke-width="4" stroke-linecap="round" />
		<path d="M17 44L32 29L47 44C43 45 21 45 17 44Z" fill="#1b5e20" stroke="#0e3d13" stroke-width="1.2" stroke-linejoin="round" />
		<path d="M21 34L32 20L43 34C39 35 25 35 21 34Z" fill="#2e7d32" stroke="#1b5e20" stroke-width="1.2" stroke-linejoin="round" />
		<path d="M25 24L32 10L39 24C36 25 28 25 25 24Z" fill="#388e3c" stroke="#2e7d32" stroke-width="1.2" stroke-linejoin="round" />
	</svg>
{:else if stage === 'sakura'}
	<!-- Cerejeira em Flor (Sakura): copas rosadas e pétalas delicadas -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-forest-tree lv-forest-tree--sakura"
	>
		<ellipse cx="32" cy="57" rx="17" ry="4" fill="currentColor" opacity="0.18" />
		<path
			d="M28 56L30 42C30 38 27 34 25 30M34 42L36 56M34 42C34 37 38 34 40 30M32 42V32"
			stroke="#5d4037"
			stroke-width="3"
			stroke-linecap="round"
		/>
		<!-- Nuvens floridas rosadas -->
		<circle cx="23" cy="27" r="11" fill="#db2777" />
		<circle cx="41" cy="27" r="11" fill="#db2777" />
		<circle cx="32" cy="18" r="12" fill="#ec4899" />
		<circle cx="25" cy="24" r="9" fill="#f472b6" />
		<circle cx="39" cy="24" r="9" fill="#f472b6" />
		<circle cx="32" cy="17" r="10" fill="#fbcfe8" />
		<!-- Pétalas caindo suavemente -->
		<circle cx="16" cy="38" r="1.5" fill="#f472b6" />
		<circle cx="46" cy="40" r="1.5" fill="#fbcfe8" />
		<circle cx="48" cy="48" r="1.2" fill="#ec4899" />
	</svg>
{:else if stage === 'willow'}
	<!-- Salgueiro Chorão: galhos arqueados e ramagens esmeralda pendentes -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-forest-tree lv-forest-tree--willow"
	>
		<ellipse cx="32" cy="57" rx="19" ry="4" fill="currentColor" opacity="0.18" />
		<path d="M30 56V38C30 32 28 26 32 20M34 56V40C34 34 38 28 32 20" stroke="#4e342e" stroke-width="3.5" stroke-linecap="round" />
		<ellipse cx="32" cy="20" rx="16" ry="10" fill="#047857" />
		<!-- Cascatas de folhagem pendentes -->
		<path d="M18 22C16 32 16 44 19 48" stroke="#10b981" stroke-width="2.2" stroke-linecap="round" />
		<path d="M24 20C22 30 22 46 25 50" stroke="#059669" stroke-width="2.5" stroke-linecap="round" />
		<path d="M32 16C31 28 31 46 33 50" stroke="#34d399" stroke-width="2.5" stroke-linecap="round" />
		<path d="M40 20C42 30 42 46 39 50" stroke="#059669" stroke-width="2.5" stroke-linecap="round" />
		<path d="M46 22C48 32 48 44 45 48" stroke="#10b981" stroke-width="2.2" stroke-linecap="round" />
	</svg>
{:else if stage === 'ginkgo'}
	<!-- Ginkgo Dourado: folhas em leque e aura dourada iluminada -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 64 64"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		aria-hidden="true"
		class="lv-forest-tree lv-forest-tree--ginkgo"
	>
		<ellipse cx="32" cy="57" rx="18" ry="4" fill="currentColor" opacity="0.18" />
		<path
			d="M28 56L30 40C30 36 27 32 24 28M34 40L36 56M34 40C34 36 38 32 40 28M32 40V30"
			stroke="#5d4037"
			stroke-width="3"
			stroke-linecap="round"
		/>
		<!-- Folhas douradas de ginkgo -->
		<circle cx="23" cy="27" r="11" fill="#d97706" />
		<circle cx="41" cy="27" r="11" fill="#d97706" />
		<circle cx="32" cy="18" r="13" fill="#eab308" />
		<circle cx="26" cy="24" r="9" fill="#facc15" />
		<circle cx="38" cy="24" r="9" fill="#facc15" />
		<circle cx="32" cy="17" r="10" fill="#fef08a" />
		<!-- Centelhas de folha dourada -->
		<polygon points="17,35 19,38 15,38" fill="#eab308" />
		<polygon points="47,36 49,39 45,39" fill="#facc15" />
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
		<ellipse cx="32" cy="58" rx="22" ry="4" fill="currentColor" opacity="0.22" />
		<path d="M20 57C26 55 28 44 28 36M44 57C38 55 36 44 36 36M32 54V34" stroke="#3e2723" stroke-width="4.5" stroke-linecap="round" />
		<ellipse cx="32" cy="24" rx="24" ry="15" fill="#1b5e20" />
		<circle cx="18" cy="24" r="10" fill="#2e7d32" />
		<circle cx="46" cy="24" r="10" fill="#2e7d32" />
		<circle cx="32" cy="17" r="13" fill="#388e3c" />
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
		<ellipse cx="32" cy="57" rx="14" ry="3" fill="#1c130d" opacity="0.25" />
		<!-- Tronco seco e retorcido em tons terrosos escuros -->
		<path
			d="M29 57 Q27 48 30 40 Q32 33 29 26 L33 26 Q35 33 33 40 Q31 48 34 57 Z"
			fill="#3a2012"
		/>
		<path
			d="M32 57 Q31 48 33 40 Q35 33 33 26 L35 26 Q37 34 35 40 Q33 48 36 57 Z"
			fill="#52311b"
		/>
		<!-- Galhos secos quebrados sem folhas -->
		<path d="M30 42 L22 35 L20 33 L23 32 L31 38 Z" fill="#3a2012" />
		<path d="M32 40 Q38 38 42 41 L43 43 Q38 40 33 43 Z" fill="#31190c" />
		<path d="M30 32 Q35 27 42 28 L43 26 Q34 25 29 30 Z" fill="#52311b" />
		<path d="M30 29 Q24 25 22 18 L24 17.5 Q26 23 31 26 Z" fill="#422515" />
	</svg>
{/if}
