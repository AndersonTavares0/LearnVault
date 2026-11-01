<script lang="ts">
	let {
		level,
		size = 24
	}: {
		level: number;
		size?: number;
	} = $props();
</script>

{#if level === 1}
	<!-- Nível 1: Broto de Bronze — Início da jornada -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-level-crest lv-level-crest--1"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="#fef3c7" stroke="#b45309" stroke-width="1.5" />
		<path d="M12 18V10" stroke="#78350f" stroke-width="1.8" stroke-linecap="round" />
		<path d="M12 13C8 13 7 9 12 10" fill="#16a34a" stroke="#15803d" stroke-width="1" stroke-linejoin="round" />
		<path d="M12 10C16 10 17 6 12 7" fill="#4ade80" stroke="#15803d" stroke-width="1" stroke-linejoin="round" />
	</svg>
{:else if level === 2}
	<!-- Nível 2: Bússola do Explorador — Orientação no acervo -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-level-crest lv-level-crest--2"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="#fef3c7" stroke="#d97706" stroke-width="1.5" />
		<circle cx="12" cy="12" r="8" stroke="#d97706" stroke-width="0.8" stroke-dasharray="2 2" />
		<polygon points="12,5 15,12 12,11 9,12" fill="#dc2626" />
		<polygon points="12,19 15,12 12,13 9,12" fill="#475569" />
		<circle cx="12" cy="12" r="1.5" fill="#f8fafc" />
	</svg>
{:else if level === 3}
	<!-- Nível 3: Tomo Aberto — Estudante dedicado -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-level-crest lv-level-crest--3"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="#e0f2fe" stroke="#0284c7" stroke-width="1.5" />
		<path d="M6 8C9 7 12 8 12 9V17C12 16 9 15 6 16V8Z" fill="#bae6fd" stroke="#0369a1" stroke-width="1.2" stroke-linejoin="round" />
		<path d="M18 8C15 7 12 8 12 9V17C12 16 15 15 18 16V8Z" fill="#bae6fd" stroke="#0369a1" stroke-width="1.2" stroke-linejoin="round" />
	</svg>
{:else if level === 4}
	<!-- Nível 4: Ampulheta de Esmeralda — Praticante consistente -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-level-crest lv-level-crest--4"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="#dcfce7" stroke="#059669" stroke-width="1.5" />
		<path d="M7 6H17M7 18H17" stroke="#047857" stroke-width="1.5" stroke-linecap="round" />
		<path d="M8 6L12 11L16 6M8 18L12 13L16 18" stroke="#047857" stroke-width="1.2" stroke-linejoin="round" fill="#a7f3d0" />
		<circle cx="12" cy="12" r="1" fill="#059669" />
	</svg>
{:else if level === 5}
	<!-- Nível 5: Joia de Ametista — Artesão do conhecimento -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-level-crest lv-level-crest--5"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="#f3e8ff" stroke="#7c3aed" stroke-width="1.5" />
		<polygon points="12,5 18,9 15,18 9,18 6,9" fill="#c084fc" stroke="#6d28d9" stroke-width="1.2" stroke-linejoin="round" />
		<line x1="6" y1="9" x2="18" y2="9" stroke="#6d28d9" stroke-width="0.8" />
		<line x1="12" y1="5" x2="12" y2="18" stroke="#6d28d9" stroke-width="0.8" />
	</svg>
{:else if level === 6}
	<!-- Nível 6: Coroa de Louros — Mestre da disciplina -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-level-crest lv-level-crest--6"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="#ffe4e6" stroke="#e11d48" stroke-width="1.5" />
		<path d="M6 14C6 17 8 19 12 19C16 19 18 17 18 14" stroke="#be123c" stroke-width="1.5" stroke-linecap="round" />
		<!-- Folhas de louro -->
		<path d="M6 13C5 11 6 9 7 9C8 10 7 12 6 13Z" fill="#e11d48" />
		<path d="M18 13C19 11 18 9 17 9C16 10 17 12 18 13Z" fill="#e11d48" />
		<polygon points="12,7 13.5,10.5 17,11 14.5,13.5 15,17 12,15 9,17 9.5,13.5 7,11 10.5,10.5" fill="#facc15" stroke="#ca8a04" stroke-width="0.8" />
	</svg>
{:else if level === 7}
	<!-- Nível 7: Tocha Dourada — Sábio da biblioteca -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-level-crest lv-level-crest--7"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="#fef9c3" stroke="#ca8a04" stroke-width="1.5" />
		<path d="M10 18L11 13H13L14 18H10Z" fill="#78350f" stroke="#451a03" stroke-width="1" />
		<!-- Chama viva -->
		<path d="M12 5C10 7 9 9 10 11C11 12 13 12 14 11C15 9 14 7 12 5Z" fill="#ef4444" />
		<path d="M12 7C11 8 11 9 11.5 10C12 10.5 12.5 10.5 13 10C13.5 9 13 8 12 7Z" fill="#facc15" />
	</svg>
{:else}
	<!-- Nível 8+: Estrela Celestial — Guardião do conhecimento -->
	<svg
		width={size}
		height={size}
		viewBox="0 0 24 24"
		fill="none"
		xmlns="http://www.w3.org/2000/svg"
		class="lv-level-crest lv-level-crest--8"
		aria-hidden="true"
	>
		<circle cx="12" cy="12" r="10" fill="#e0e7ff" stroke="#6366f1" stroke-width="1.5" />
		<!-- Estrela de 8 pontas radiante -->
		<polygon points="12,4 14,9 19,7 16,12 19,17 14,15 12,20 10,15 5,17 8,12 5,7 10,9" fill="#818cf8" stroke="#4338ca" stroke-width="0.8" />
		<circle cx="12" cy="12" r="2.5" fill="#fde047" stroke="#eab308" stroke-width="0.8" />
	</svg>
{/if}

<style>
	.lv-level-crest {
		display: block;
		flex-shrink: 0;
		filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.08));
	}
</style>
