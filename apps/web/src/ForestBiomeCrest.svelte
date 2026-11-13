<script lang="ts">
	let {
		level,
		size = 40
	}: {
		level: number;
		size?: number;
	} = $props();
</script>

{#if level === 0}
	<!-- Nível 0: Clareira Inicial — Semente brotando em solo fértil -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 48 48"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-biome-crest lv-biome-crest--0"
		aria-hidden="true"
	>
		<circle cx="24" cy="24" r="22" fill="color-mix(in srgb, #b45309 10%, transparent)" stroke="#b45309" stroke-width="1.5" />
		<!-- Solo suave -->
		<ellipse cx="24" cy="38" rx="14" ry="4" fill="#78350f" opacity="0.3" />
		<!-- Broto jovem -->
		<path d="M24 38V24" stroke="#5d4037" stroke-width="2.5" stroke-linecap="round" />
		<path d="M24 28C17 28 16 20 24 22" fill="#65a30d" stroke="#4d7c0f" stroke-width="1.2" stroke-linejoin="round" />
		<path d="M24 24C31 24 32 16 24 18" fill="#84cc16" stroke="#4d7c0f" stroke-width="1.2" stroke-linejoin="round" />
		<!-- Raios de sol matinal -->
		<path d="M24 8V12M14 12L17 15M34 12L31 15" stroke="#f59e0b" stroke-width="1.5" stroke-linecap="round" opacity="0.7" />
	</svg>
{:else if level === 1}
	<!-- Nível 1: Bosque Jovem — Duas mudas crescendo juntas em harmonia -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 48 48"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-biome-crest lv-biome-crest--1"
		aria-hidden="true"
	>
		<circle cx="24" cy="24" r="22" fill="color-mix(in srgb, #16a34a 12%, transparent)" stroke="#16a34a" stroke-width="1.5" />
		<ellipse cx="24" cy="38" rx="16" ry="4" fill="#15803d" opacity="0.25" />
		<!-- Muda principal -->
		<path d="M20 38V20" stroke="#4e342e" stroke-width="2.5" stroke-linecap="round" />
		<circle cx="20" cy="18" r="7" fill="#22c55e" stroke="#15803d" stroke-width="1.2" />
		<!-- Muda secundária -->
		<path d="M30 38V25" stroke="#5d4037" stroke-width="2" stroke-linecap="round" />
		<circle cx="30" cy="24" r="5.5" fill="#4ade80" stroke="#16a34a" stroke-width="1.2" />
		<!-- Brilho suave -->
		<circle cx="15" cy="12" r="1.5" fill="#facc15" />
		<circle cx="35" cy="14" r="1.5" fill="#facc15" />
	</svg>
{:else if level === 2}
	<!-- Nível 2: Bosque Frondoso — Carvalho copado com ramificações -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 48 48"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-biome-crest lv-biome-crest--2"
		aria-hidden="true"
	>
		<circle cx="24" cy="24" r="22" fill="color-mix(in srgb, #15803d 14%, transparent)" stroke="#15803d" stroke-width="1.5" />
		<ellipse cx="24" cy="39" rx="16" ry="4" fill="#166534" opacity="0.3" />
		<!-- Tronco forte -->
		<path d="M20 38L22 28C22 24 19 22 17 20M28 38L26 28C26 24 29 22 31 20M24 28V22" stroke="#4e342e" stroke-width="3" stroke-linecap="round" />
		<!-- Copa densa em camadas -->
		<circle cx="18" cy="19" r="8" fill="#15803d" />
		<circle cx="30" cy="19" r="8" fill="#15803d" />
		<circle cx="24" cy="13" r="9" fill="#16a34a" />
		<circle cx="24" cy="14" r="7" fill="#22c55e" opacity="0.9" />
	</svg>
{:else if level === 3}
	<!-- Nível 3: Floresta Verdejante — Coníferas e copas entrelaçadas sob céu claro -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 48 48"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-biome-crest lv-biome-crest--3"
		aria-hidden="true"
	>
		<circle cx="24" cy="24" r="22" fill="color-mix(in srgb, #047857 16%, transparent)" stroke="#047857" stroke-width="1.5" />
		<ellipse cx="24" cy="40" rx="18" ry="4" fill="#064e3b" opacity="0.3" />
		<!-- Pinheiro central imponente -->
		<path d="M24 40V26" stroke="#3e2723" stroke-width="3" stroke-linecap="round" />
		<path d="M14 31L24 19L34 31C30 32 18 32 14 31Z" fill="#065f46" stroke="#064e3b" stroke-width="1" />
		<path d="M17 23L24 13L31 23C28 24 20 24 17 23Z" fill="#047857" stroke="#065f46" stroke-width="1" />
		<path d="M20 16L24 9L28 16C26 17 22 17 20 16Z" fill="#10b981" />
		<!-- Bosque ao fundo -->
		<circle cx="13" cy="28" r="6" fill="#15803d" opacity="0.8" />
		<circle cx="35" cy="28" r="6" fill="#15803d" opacity="0.8" />
	</svg>
{:else if level === 4}
	<!-- Nível 4: Reserva Natural — Montanhas, pinheiros ancestrais e curso d'água -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 48 48"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-biome-crest lv-biome-crest--4"
		aria-hidden="true"
	>
		<circle cx="24" cy="24" r="22" fill="color-mix(in srgb, #0284c7 15%, transparent)" stroke="#0284c7" stroke-width="1.5" />
		<!-- Picos de montanhas ao fundo -->
		<path d="M8 36L18 20L28 36H8Z" fill="#475569" opacity="0.4" />
		<path d="M22 36L32 17L42 36H22Z" fill="#334155" opacity="0.5" />
		<!-- Neve no pico -->
		<path d="M32 17L35 22L32 23L29 22L32 17Z" fill="#f8fafc" opacity="0.9" />
		<!-- Floresta na base -->
		<path d="M24 38L24 28" stroke="#3e2723" stroke-width="2.5" />
		<path d="M18 33L24 23L30 33C27 34 21 34 18 33Z" fill="#065f46" />
		<path d="M20 27L24 20L28 27C26 28 22 28 20 27Z" fill="#059669" />
		<!-- Curso d'água cristalino -->
		<path d="M20 38C22 41 26 41 28 44" stroke="#38bdf8" stroke-width="2" stroke-linecap="round" />
	</svg>
{:else}
	<!-- Nível 5: Santuário Ancestral — A Árvore Cósmica com raízes de ouro e constelações -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 48 48"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-biome-crest lv-biome-crest--5"
		aria-hidden="true"
	>
		<!-- Halo celestial dourado -->
		<circle cx="24" cy="24" r="22" fill="color-mix(in srgb, #eab308 18%, transparent)" stroke="#eab308" stroke-width="1.8" />
		<!-- Estrelas / Pontos de constelação -->
		<circle cx="11" cy="14" r="1.5" fill="#fef08a" />
		<circle cx="37" cy="14" r="1.5" fill="#fef08a" />
		<circle cx="24" cy="6" r="2" fill="#fde047" />
		<!-- Solo sagrado -->
		<ellipse cx="24" cy="40" rx="18" ry="4" fill="#713f12" opacity="0.4" />
		<!-- Raízes robustas e tronco dourado ancestral -->
		<path d="M17 40C20 38 22 30 22 24M31 40C28 38 26 30 26 24M24 38V22" stroke="#854d0e" stroke-width="3" stroke-linecap="round" />
		<!-- Copa mística radiante -->
		<circle cx="24" cy="17" r="12" fill="#065f46" />
		<circle cx="17" cy="18" r="8" fill="#047857" />
		<circle cx="31" cy="18" r="8" fill="#047857" />
		<circle cx="24" cy="13" r="9" fill="#10b981" />
		<circle cx="24" cy="13" r="5" fill="#fef08a" opacity="0.85" />
	</svg>
{/if}

<style>
	.lv-biome-crest {
		display: block;
		flex-shrink: 0;
		filter: drop-shadow(0 2px 4px rgba(0, 0, 0, 0.08));
		transition: transform 0.2s ease;
	}

	.lv-biome-crest:hover {
		transform: scale(1.06);
	}
</style>
